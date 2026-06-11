use crate::bridge::runtime_bootstrap;
use crate::commonjs::CommonJsModules;
use anp::authentication::AuthMode;
use anp_adapter::{
    bearer_token_expiry_ms, sign_challenge_proof, ChallengeLoginRequest, ChallengeLoginResponse,
    ChallengeProofPayload, DidChallenge as AdapterDidChallenge, DidCredentialConfig,
    FileDidCredentialProvider, IdentitySession,
};
use dock_core::error::{DockCoreError, ErrorCode};
use dock_core::host::ApiExecutor;
use dock_core::orchestrator::ApiCallContext;
use mcp_schema::AtomicApiResult;
use rquickjs::function::Func;
use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Function, Object, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use skill_loader::LoadedSkill;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApiVmConfig {
    pub timeout: Duration,
    pub memory_limit_bytes: usize,
    pub max_stack_size_bytes: usize,
}

impl Default for ApiVmConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            memory_limit_bytes: 16 * 1024 * 1024,
            max_stack_size_bytes: 512 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HostDidAuthConfig {
    pub did_document_path: PathBuf,
    pub private_key_path: PathBuf,
    pub check_private_key_permissions: bool,
    token_cache: Arc<Mutex<BTreeMap<AuthSessionKey, CachedAuthSession>>>,
}

impl HostDidAuthConfig {
    pub fn new(
        did_document_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            did_document_path: did_document_path.into(),
            private_key_path: private_key_path.into(),
            check_private_key_permissions: true,
            token_cache: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn without_private_key_permission_check(mut self) -> Self {
        self.check_private_key_permissions = false;
        self
    }

    fn credential_config(&self) -> DidCredentialConfig {
        let mut config = DidCredentialConfig::new(
            self.did_document_path.clone(),
            self.private_key_path.clone(),
        );
        config.check_private_key_permissions = self.check_private_key_permissions;
        config
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AuthSessionKey {
    base_url: String,
    merchant_did: String,
    user_did: String,
    agent_did: Option<String>,
    skill_id: String,
    session_id: String,
}

#[derive(Debug, Clone)]
struct CachedAuthSession {
    token: String,
    expires_at_ms: Option<u64>,
    scopes: Vec<String>,
}

impl CachedAuthSession {
    fn is_expired(&self) -> bool {
        self.expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms().saturating_add(5_000))
    }
}

fn now_ms() -> u64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredApi {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsoleLevel {
    Log,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub console: Vec<ConsoleEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiCall {
    pub skill_id: String,
    pub session_id: String,
    pub api_name: String,
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_did: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_did: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_did: Option<String>,
}

impl ApiCall {
    pub fn new(
        skill_id: impl Into<String>,
        session_id: impl Into<String>,
        api_name: impl Into<String>,
        arguments: Value,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            session_id: session_id.into(),
            api_name: api_name.into(),
            arguments,
            user_did: None,
            agent_did: None,
            merchant_did: None,
        }
    }

    fn to_context_value(&self) -> Value {
        json!({
            "name": self.api_name,
            "skillId": self.skill_id,
            "sessionId": self.session_id,
            "arguments": self.arguments,
            "userDid": self.user_did,
            "agentDid": self.agent_did,
            "merchantDid": self.merchant_did,
        })
    }

    fn argument_string(&self, name: &str) -> Option<String> {
        self.arguments
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }
}

impl From<&ApiCallContext> for ApiCall {
    fn from(context: &ApiCallContext) -> Self {
        Self {
            skill_id: context.skill_id.clone(),
            session_id: context.session_id.clone(),
            api_name: context.api_name.clone(),
            arguments: context.arguments.clone(),
            user_did: context.user_did.clone(),
            agent_did: context.agent_did.clone(),
            merchant_did: context.merchant_did.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ApiVmError {
    #[error("quickjs error: {0}")]
    QuickJs(String),

    #[error("unsafe require: {0}")]
    UnsafeRequire(String),

    #[error("missing API registration: {0}")]
    MissingApi(String),

    #[error("duplicate API registration reported by VM: {0}")]
    DuplicateApi(String),

    #[error("API `{0}` is not declared in mcp.json")]
    UndeclaredApi(String),

    #[error("API `{0}` was declared but not registered by index.js")]
    ManifestApiNotRegistered(String),

    #[error("API `{0}` returned invalid JSON: {1}")]
    InvalidJson(String, String),

    #[error("API `{0}` returned invalid AtomicApiResult: {1}")]
    InvalidResult(String, String),

    #[error("API `{0}` timed out after {1:?}")]
    Timeout(String, Duration),
}

impl ApiVmError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Timeout(_, _) => ErrorCode::Timeout,
            Self::MissingApi(_)
            | Self::DuplicateApi(_)
            | Self::UndeclaredApi(_)
            | Self::ManifestApiNotRegistered(_)
            | Self::InvalidJson(_, _)
            | Self::InvalidResult(_, _)
            | Self::UnsafeRequire(_) => ErrorCode::ValidationFailed,
            Self::QuickJs(_) => ErrorCode::VmFailed,
        }
    }
}

impl From<ApiVmError> for DockCoreError {
    fn from(error: ApiVmError) -> Self {
        DockCoreError::core(error.code(), error.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ApiVm {
    skill: LoadedSkill,
    modules: CommonJsModules,
    config: ApiVmConfig,
    registered_apis: Vec<RegisteredApi>,
    trace: ExecutionTrace,
}

impl ApiVm {
    pub fn load_skill(skill: LoadedSkill) -> Result<Self, ApiVmError> {
        Self::load_skill_with_config(skill, ApiVmConfig::default())
    }

    pub fn load_skill_with_config(
        skill: LoadedSkill,
        config: ApiVmConfig,
    ) -> Result<Self, ApiVmError> {
        let modules = CommonJsModules::from_skill(&skill)?;
        let (registered_apis, trace) = evaluate_registration(&modules, &config)?;
        validate_registration(&skill, &registered_apis)?;

        Ok(Self {
            skill,
            modules,
            config,
            registered_apis,
            trace,
        })
    }

    pub fn registered_apis(&self) -> &[RegisteredApi] {
        &self.registered_apis
    }

    pub fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }

    pub fn call(&self, call: ApiCall) -> Result<AtomicApiResult, ApiVmError> {
        self.call_with_host_did_auth(call, None)
    }

    pub fn call_with_host_did_auth(
        &self,
        call: ApiCall,
        host_did_auth: Option<HostDidAuthConfig>,
    ) -> Result<AtomicApiResult, ApiVmError> {
        if !self
            .registered_apis
            .iter()
            .any(|registered| registered.name == call.api_name)
        {
            return Err(ApiVmError::MissingApi(call.api_name));
        }

        execute_api_call(&self.modules, &self.config, call, host_did_auth)
    }

    pub fn executor(self) -> QuickJsApiExecutor {
        QuickJsApiExecutor::new(self)
    }

    pub fn skill(&self) -> &LoadedSkill {
        &self.skill
    }
}

#[derive(Debug, Clone)]
pub struct QuickJsApiExecutor {
    vm: ApiVm,
    host_did_auth: Option<HostDidAuthConfig>,
}

impl QuickJsApiExecutor {
    pub fn new(vm: ApiVm) -> Self {
        Self {
            vm,
            host_did_auth: None,
        }
    }

    pub fn with_host_did_auth(mut self, host_did_auth: HostDidAuthConfig) -> Self {
        self.host_did_auth = Some(host_did_auth);
        self
    }

    pub fn vm(&self) -> &ApiVm {
        &self.vm
    }
}

impl ApiExecutor for QuickJsApiExecutor {
    fn execute(
        &self,
        context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        self.vm
            .call_with_host_did_auth(ApiCall::from(context), self.host_did_auth.clone())
            .map_err(Into::into)
    }
}

fn evaluate_registration(
    modules: &CommonJsModules,
    config: &ApiVmConfig,
) -> Result<(Vec<RegisteredApi>, ExecutionTrace), ApiVmError> {
    with_runtime(modules, config, HostBridgeRuntime::registration(), |ctx| {
        let load_entry: Function = ctx
            .globals()
            .get("__dockLoadEntry")
            .map_err(to_quickjs_error)?;
        load_entry
            .call::<_, ()>(())
            .catch(&ctx)
            .map_err(caught_error)?;
        drain_jobs(&ctx);

        let registered_names: Function = ctx
            .globals()
            .get("__dockRegisteredApiNames")
            .map_err(to_quickjs_error)?;
        let names_json = ctx
            .json_stringify(
                registered_names
                    .call::<_, rquickjs::Value>(())
                    .catch(&ctx)
                    .map_err(caught_error)?,
            )
            .catch(&ctx)
            .map_err(caught_error)?
            .ok_or_else(|| {
                ApiVmError::QuickJs("failed to serialize registered API names".to_owned())
            })?
            .to_string()
            .map_err(to_quickjs_error)?;
        let names: Vec<String> = serde_json::from_str(&names_json).map_err(|error| {
            ApiVmError::InvalidJson("__registeredApis".to_owned(), error.to_string())
        })?;

        let mut seen = BTreeSet::new();
        let mut apis = Vec::with_capacity(names.len());
        for name in names {
            if !seen.insert(name.clone()) {
                return Err(ApiVmError::DuplicateApi(name));
            }
            apis.push(RegisteredApi { name });
        }
        Ok(apis)
    })
}

fn execute_api_call(
    modules: &CommonJsModules,
    config: &ApiVmConfig,
    call: ApiCall,
    host_did_auth: Option<HostDidAuthConfig>,
) -> Result<AtomicApiResult, ApiVmError> {
    let api_name = call.api_name.clone();
    let bridge = HostBridgeRuntime::for_call(call.clone(), host_did_auth);
    let (result, _trace) = with_runtime(modules, config, bridge, |ctx| {
        let load_entry: Function = ctx
            .globals()
            .get("__dockLoadEntry")
            .map_err(to_quickjs_error)?;
        load_entry
            .call::<_, ()>(())
            .catch(&ctx)
            .map_err(caught_error)?;
        drain_jobs(&ctx);

        let context_json = serde_json::to_string(&call.to_context_value())
            .map_err(|error| ApiVmError::InvalidJson(api_name.clone(), error.to_string()))?;
        let call_api: Function = ctx
            .globals()
            .get("__dockCallApi")
            .map_err(to_quickjs_error)?;
        let result: rquickjs::promise::MaybePromise = call_api
            .call((api_name.as_str(), context_json))
            .catch(&ctx)
            .map_err(|error| map_caught_or_timeout(error, &api_name, config.timeout))?;
        let result_json = result
            .finish::<String>()
            .catch(&ctx)
            .map_err(|error| map_caught_or_timeout(error, &api_name, config.timeout))?;

        serde_json::from_str::<AtomicApiResult>(&result_json).map_err(|error| {
            ApiVmError::InvalidResult(api_name.clone(), format!("{error}; payload={result_json}"))
        })
    })?;
    Ok(result)
}

fn with_runtime<R>(
    modules: &CommonJsModules,
    config: &ApiVmConfig,
    bridge: HostBridgeRuntime,
    callback: impl for<'js> FnOnce(Ctx<'js>) -> Result<R, ApiVmError>,
) -> Result<(R, ExecutionTrace), ApiVmError> {
    let runtime = Runtime::new().map_err(to_quickjs_error)?;
    runtime.set_memory_limit(config.memory_limit_bytes);
    runtime.set_max_stack_size(config.max_stack_size_bytes);

    let start = Instant::now();
    let timeout = config.timeout;
    runtime.set_interrupt_handler(Some(Box::new(move || start.elapsed() >= timeout)));

    let context = Context::builder()
        .with::<rquickjs::context::intrinsic::Eval>()
        .with::<rquickjs::context::intrinsic::Promise>()
        .with::<rquickjs::context::intrinsic::Json>()
        .build(&runtime)
        .map_err(to_quickjs_error)?;
    let console = Rc::new(RefCell::new(Vec::new()));
    let modules_json = serde_json::to_string(&modules.to_json_value())
        .map_err(|error| ApiVmError::InvalidJson("__modules".to_owned(), error.to_string()))?;

    let result = context.with(|ctx| {
        install_host_bridge(ctx.clone(), modules_json, console.clone(), bridge)?;
        ctx.eval::<(), _>(runtime_bootstrap())
            .catch(&ctx)
            .map_err(caught_error)?;

        callback(ctx)
    });

    let trace = ExecutionTrace {
        console: Rc::try_unwrap(console).unwrap_or_default().into_inner(),
    };

    runtime.set_interrupt_handler(None);

    result.map(|value| (value, trace))
}

fn install_host_bridge<'js>(
    ctx: Ctx<'js>,
    modules_json: String,
    console: Rc<RefCell<Vec<ConsoleEntry>>>,
    bridge: HostBridgeRuntime,
) -> Result<(), ApiVmError> {
    let dock = Object::new(ctx.clone()).map_err(to_quickjs_error)?;
    let modules_json_fn = {
        let modules_json = modules_json.clone();
        Func::from(move || modules_json.clone())
    };
    dock.set("modulesJson", modules_json_fn)
        .map_err(to_quickjs_error)?;

    let login_bridge = bridge.clone();
    let login_fn = Func::from(move || login_bridge.login_json());
    dock.set("login", login_fn).map_err(to_quickjs_error)?;

    let request_bridge = bridge.clone();
    let request_fn =
        Func::from(move |options_json: String| request_bridge.request_json(options_json));
    dock.set("request", request_fn).map_err(to_quickjs_error)?;

    let log_fn = Func::from(move |level: String, args: Vec<String>| {
        let level = match level.as_str() {
            "warn" => ConsoleLevel::Warn,
            "error" => ConsoleLevel::Error,
            _ => ConsoleLevel::Log,
        };
        console.borrow_mut().push(ConsoleEntry {
            level,
            message: args.join(" "),
        });
    });
    dock.set("log", log_fn).map_err(to_quickjs_error)?;
    ctx.globals()
        .set("__dock", dock)
        .map_err(to_quickjs_error)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostRequestOptions {
    url: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default, alias = "headers")]
    header: BTreeMap<String, String>,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Clone)]
struct HostBridgeRuntime {
    call: Option<ApiCall>,
    host_did_auth: Option<HostDidAuthConfig>,
}

impl HostBridgeRuntime {
    fn registration() -> Self {
        Self {
            call: None,
            host_did_auth: None,
        }
    }

    fn for_call(call: ApiCall, host_did_auth: Option<HostDidAuthConfig>) -> Self {
        Self {
            call: Some(call),
            host_did_auth,
        }
    }

    fn login_json(&self) -> String {
        match self.ensure_login(None) {
            Ok(Some(session)) => json!({
                "code": "dock-login-code-localhost",
                "errMsg": "login:ok",
                "didAuth": {
                    "status": "ok",
                    "tokenReceived": true,
                    "tokenVisibleToSkill": false,
                    "userDid": self.call.as_ref().and_then(|call| call.user_did.clone()),
                    "agentDid": self.call.as_ref().and_then(|call| call.agent_did.clone()),
                    "merchantDid": self.call.as_ref().and_then(|call| call.merchant_did.clone()),
                    "scopes": session.scopes
                }
            })
            .to_string(),
            Ok(None) => json!({
                "code": "dock-login-code-localhost",
                "errMsg": "login:ok",
                "didAuth": {
                    "status": "mock",
                    "tokenReceived": false,
                    "tokenVisibleToSkill": false
                }
            })
            .to_string(),
            Err(message) => json!({
                "errMsg": format!("login:fail {message}")
            })
            .to_string(),
        }
    }

    fn request_json(&self, options_json: String) -> String {
        match self.host_request(&options_json) {
            Ok(value) => value.to_string(),
            Err(message) => json!({
                "errMsg": format!("request:fail {message}")
            })
            .to_string(),
        }
    }

    fn host_request(&self, options_json: &str) -> Result<Value, String> {
        let options: HostRequestOptions =
            serde_json::from_str(options_json).map_err(|error| error.to_string())?;
        let data = options.data.unwrap_or(Value::Null);
        let method = options
            .method
            .as_deref()
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let parsed = ParsedHttpUrl::parse(&options.url)?;
        if !parsed.is_loopback() {
            return Err("wx.request demo bridge only allows localhost URLs".to_owned());
        }

        let mut request_url = options.url.clone();
        let body = if method == "GET" {
            let mut path = parsed.path_with_query.clone();
            append_query_data(&mut path, data);
            request_url = format!("{}{}", parsed.origin(), path);
            String::new()
        } else if data.is_null() {
            String::new()
        } else {
            data.to_string()
        };

        let session = self.ensure_login(Some(parsed.origin()))?;
        let bearer = session.as_ref().map(|session| session.token.as_str());
        let headers = options
            .header
            .into_iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
            .collect::<BTreeMap<_, _>>();

        let (status, headers, response_body) =
            http_request_url(&request_url, &method, bearer, Some(&body), headers)?;
        let data = serde_json::from_str::<Value>(&response_body)
            .unwrap_or_else(|_| Value::String(response_body.to_owned()));

        Ok(json!({
            "statusCode": status,
            "header": headers,
            "data": data,
            "errMsg": "request:ok"
        }))
    }

    fn ensure_login(
        &self,
        request_origin: Option<String>,
    ) -> Result<Option<CachedAuthSession>, String> {
        let Some(auth_config) = &self.host_did_auth else {
            return Ok(None);
        };
        let Some(call) = &self.call else {
            return Ok(None);
        };
        let base_url = request_origin
            .or_else(|| call.argument_string("remoteBaseUrl"))
            .or_else(|| call.argument_string("serverUrl"))
            .map(|url| url.trim_end_matches('/').to_owned());
        let Some(base_url) = base_url.filter(|url| !url.is_empty()) else {
            return Ok(None);
        };
        let user_did = call
            .user_did
            .clone()
            .ok_or_else(|| "DID login requires userDid in ApiCallContext".to_owned())?;
        let merchant_did = call
            .merchant_did
            .clone()
            .unwrap_or_else(|| "did:wba:coffee-merchant.example".to_owned());
        let key = AuthSessionKey {
            base_url: base_url.clone(),
            merchant_did,
            user_did,
            agent_did: call.agent_did.clone(),
            skill_id: call.skill_id.clone(),
            session_id: call.session_id.clone(),
        };
        if let Some(cached) = auth_config
            .token_cache
            .lock()
            .map_err(|_| "DID token cache is unavailable".to_owned())?
            .get(&key)
            .filter(|session| !session.is_expired())
            .cloned()
        {
            return Ok(Some(cached));
        }

        let login = self.perform_did_login(auth_config, &key)?;
        auth_config
            .token_cache
            .lock()
            .map_err(|_| "DID token cache is unavailable".to_owned())?
            .insert(key, login.clone());
        Ok(Some(login))
    }

    fn perform_did_login(
        &self,
        auth_config: &HostDidAuthConfig,
        key: &AuthSessionKey,
    ) -> Result<CachedAuthSession, String> {
        let challenge_value = post_json_url(
            &format!("{}/agents/coffee/auth/challenge", key.base_url),
            None,
            json!({
                "sessionId": key.session_id,
                "skillId": key.skill_id,
                "userDid": key.user_did,
                "agentDid": key.agent_did
            }),
        )?;
        let challenge: HostDidChallenge = serde_json::from_value(challenge_value)
            .map_err(|error| format!("invalid DID challenge response: {error}"))?;
        let session = IdentitySession::new(
            key.user_did.clone(),
            key.agent_did.clone(),
            challenge.merchant_did.clone(),
            key.skill_id.clone(),
            key.session_id.clone(),
        );
        let payload = ChallengeProofPayload::from_challenge(
            &AdapterDidChallenge {
                challenge_id: challenge.challenge_id.clone(),
                merchant_did: challenge.merchant_did.clone(),
                nonce: challenge.nonce.clone(),
                expires_at_ms: challenge.expires_at_ms,
            },
            &session,
            challenge.audience.clone(),
            challenge.issued_at_ms,
        );
        let provider = FileDidCredentialProvider::from_config(auth_config.credential_config())
            .map_err(|error| format!("DID credential unavailable: {error}"))?;
        let proof = sign_challenge_proof(&payload, &provider, &session, AuthMode::HttpSignatures)
            .map_err(|error| format!("DID challenge proof failed: {error}"))?;
        let login_request = ChallengeLoginRequest {
            session_id: key.session_id.clone(),
            skill_id: key.skill_id.clone(),
            user_did: key.user_did.clone(),
            agent_did: key.agent_did.clone(),
            merchant_did: challenge.merchant_did,
            challenge_id: challenge.challenge_id,
            signed_challenge: serde_json::to_value(proof)
                .map_err(|error| format!("DID proof serialization failed: {error}"))?,
        };
        let login_value = post_json_url(
            &format!("{}/agents/coffee/auth/login", key.base_url),
            None,
            serde_json::to_value(login_request)
                .map_err(|error| format!("DID login request serialization failed: {error}"))?,
        )?;
        let login: ChallengeLoginResponse = serde_json::from_value(login_value)
            .map_err(|error| format!("invalid DID login response: {error}"))?;
        if login.capability_token.trim().is_empty() {
            return Err("DID login did not return a capability token".to_owned());
        }
        let scopes = capability_token_scopes(&login.capability_token).unwrap_or_else(|| {
            vec![
                "coffee:drinks:read".to_owned(),
                "coffee:order:confirm".to_owned(),
                "coffee:order:pay".to_owned(),
                "coffee:order:read".to_owned(),
            ]
        });
        Ok(CachedAuthSession {
            token: login.capability_token,
            expires_at_ms: login.expires_at_ms,
            scopes,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostDidChallenge {
    challenge_id: String,
    merchant_did: String,
    nonce: String,
    issued_at_ms: u64,
    expires_at_ms: Option<u64>,
    audience: String,
}

fn post_json_url(url: &str, bearer: Option<&str>, body: Value) -> Result<Value, String> {
    let body = body.to_string();
    let (status, _, response_body) =
        http_request_url(url, "POST", bearer, Some(&body), BTreeMap::new())?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "POST {url} returned {status}: {}",
            response_body.redacted_for_display().prefix_text(300)
        ));
    }
    serde_json::from_str(&response_body).map_err(|error| error.to_string())
}

fn capability_token_scopes(token: &str) -> Option<Vec<String>> {
    let _ = bearer_token_expiry_ms(token)?;
    let payload = token.split('.').nth(1)?;
    let decoded = base64_url_decode(payload).ok()?;
    let value = serde_json::from_slice::<Value>(&decoded).ok()?;
    value
        .get("scopes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut output = Vec::new();
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = TABLE
            .bytes()
            .position(|candidate| candidate == byte)
            .ok_or_else(|| "invalid base64url character".to_owned())? as u32;
        bits = (bits << 6) | value;
        bit_count += 6;
        while bit_count >= 8 {
            bit_count -= 8;
            output.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Ok(output)
}

fn http_request_url(
    url: &str,
    method: &str,
    bearer: Option<&str>,
    body: Option<&str>,
    headers: BTreeMap<String, String>,
) -> Result<(u16, BTreeMap<String, String>, String), String> {
    let parsed = ParsedHttpUrl::parse(url)?;
    if !parsed.is_loopback() {
        return Err("wx.request demo bridge only allows localhost URLs".to_owned());
    }
    let body = body.unwrap_or_default();
    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;

    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        parsed.path_with_query,
        parsed.host_header(),
        body.len()
    );
    if let Some(token) = bearer {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;

    let response = read_http_response(&mut stream)?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "HTTP response missing header separator".to_owned())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| "HTTP response missing status code".to_owned())?;
    let headers = head
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    Ok((status, headers, body.to_owned()))
}

trait RedactedText {
    fn prefix_text(&self, max_length: usize) -> String;
    fn redacted_for_display(&self) -> String;
}

impl RedactedText for str {
    fn prefix_text(&self, max_length: usize) -> String {
        if self.chars().count() <= max_length {
            return self.to_owned();
        }
        self.chars().take(max_length).collect::<String>() + "…"
    }

    fn redacted_for_display(&self) -> String {
        let mut text = self.to_owned();
        for marker in [
            "Authorization",
            "Signature",
            "capabilityToken",
            "accessToken",
            "token",
            "private",
            "secret",
        ] {
            if text
                .to_ascii_lowercase()
                .contains(&marker.to_ascii_lowercase())
            {
                text = "[REDACTED]".to_owned();
                break;
            }
        }
        text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHttpUrl {
    host: String,
    port: u16,
    path_with_query: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "only http:// localhost URLs are supported".to_owned())?;
        let (authority, path) = rest
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((rest, "/".to_owned()));
        let (host, port) = authority
            .rsplit_once(':')
            .map(|(host, port)| {
                let port = port
                    .parse::<u16>()
                    .map_err(|error| format!("invalid URL port: {error}"))?;
                Ok::<_, String>((host.to_owned(), port))
            })
            .transpose()?
            .unwrap_or_else(|| (authority.to_owned(), 80));
        if host.is_empty() {
            return Err("URL host is required".to_owned());
        }
        Ok(Self {
            host,
            port,
            path_with_query: path,
        })
    }

    fn is_loopback(&self) -> bool {
        self.host == "localhost" || self.host == "127.0.0.1"
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.host_header())
    }
}

fn append_query_data(path: &mut String, data: Value) {
    let Value::Object(map) = data else {
        return;
    };
    for (key, value) in map {
        if path.contains('?') {
            path.push('&');
        } else {
            path.push('?');
        }
        path.push_str(&url_encode(&key));
        path.push('=');
        path.push_str(&url_encode(&value_to_query_string(value)));
    }
}

fn read_http_response(stream: &mut TcpStream) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed before response headers".to_owned());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
    let body_start = header_end + 4;
    let content_length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    if let Some(content_length) = content_length {
        while bytes.len().saturating_sub(body_start) < content_length {
            let read = stream
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("connection closed before full response body".to_owned());
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    } else {
        loop {
            let read = stream
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn value_to_query_string(value: Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value,
        other => other.to_string(),
    }
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn validate_registration(
    skill: &LoadedSkill,
    registered_apis: &[RegisteredApi],
) -> Result<(), ApiVmError> {
    let declared: BTreeSet<_> = skill
        .manifest
        .apis
        .iter()
        .map(|api| api.name.as_str())
        .collect();
    let registered: BTreeSet<_> = registered_apis
        .iter()
        .map(|api| api.name.as_str())
        .collect();

    for name in &registered {
        if !declared.contains(name) {
            return Err(ApiVmError::UndeclaredApi((*name).to_owned()));
        }
    }

    for name in &declared {
        if !registered.contains(name) {
            return Err(ApiVmError::ManifestApiNotRegistered((*name).to_owned()));
        }
    }

    Ok(())
}

fn drain_jobs(ctx: &Ctx<'_>) {
    while ctx.execute_pending_job() {}
}

fn map_caught_or_timeout(error: CaughtError<'_>, api_name: &str, timeout: Duration) -> ApiVmError {
    if caught_message(&error).as_deref() == Some("interrupted") {
        return ApiVmError::Timeout(api_name.to_owned(), timeout);
    }

    if matches!(error, CaughtError::Error(rquickjs::Error::Exception)) {
        ApiVmError::Timeout(api_name.to_owned(), timeout)
    } else {
        caught_error(error)
    }
}

fn to_quickjs_error(error: rquickjs::Error) -> ApiVmError {
    ApiVmError::QuickJs(error.to_string())
}

fn caught_error(error: CaughtError<'_>) -> ApiVmError {
    match error {
        CaughtError::Exception(exception) => {
            ApiVmError::QuickJs(exception.message().unwrap_or_else(|| exception.to_string()))
        }
        CaughtError::Value(value) => ApiVmError::QuickJs(format!("{value:?}")),
        CaughtError::Error(error) => to_quickjs_error(error),
    }
}

fn caught_message(error: &CaughtError<'_>) -> Option<String> {
    match error {
        CaughtError::Exception(exception) => exception.message(),
        CaughtError::Value(value) => Some(format!("{value:?}")),
        CaughtError::Error(error) => Some(error.to_string()),
    }
}
