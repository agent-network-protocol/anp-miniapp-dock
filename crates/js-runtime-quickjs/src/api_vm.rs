use crate::bridge::runtime_bootstrap;
use crate::commonjs::CommonJsModules;
use anp::authentication::AuthMode;
use anp_adapter::{
    decode_capability_token_scopes, sign_challenge_proof, ChallengeLoginRequest,
    ChallengeLoginResponse, ChallengeProofPayload, DidAuthReceipt, DidAuthSession,
    DidAuthSessionError, DidAuthSessionKey, DidAuthSessionManager,
    DidChallenge as AdapterDidChallenge, DidCredentialConfig, FileDidCredentialProvider,
    IdentitySession,
};
use dock_core::error::{DockCoreError, ErrorCode};
use dock_core::host::ApiExecutor;
use dock_core::observability::{
    MetricsSink, NoopMetricsSink, ObservabilityMetric, TraceContext, TraceSpan, TraceSpanKind,
};
use dock_core::orchestrator::ApiCallContext;
use mcp_schema::AtomicApiResult;
use rquickjs::function::Func;
use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Function, Object, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use skill_loader::{resolve_component_path, LoadedSkill};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use thiserror::Error;
use wx_compat::{
    high_risk_api_spec, high_risk_consent_required_json, high_risk_error_json,
    high_risk_success_json, unsupported_api, AppBaseInfo, CapabilityProfile, DeviceInfo,
    HighRiskApiRequest, HighRiskHostProvider, InMemoryScopedStorage, RequestBroker, ScopedStorage,
    StorageError, StorageScope, UnavailableHighRiskHostProvider, WxMethod, WxRequest,
    WxRequestError, WxResponse,
};

#[derive(Debug, Clone)]
pub struct ApiVmConfig {
    pub timeout: Duration,
    pub memory_limit_bytes: usize,
    pub max_stack_size_bytes: usize,
    pub max_pending_jobs: usize,
    pub max_console_entries: usize,
    pub max_console_message_chars: usize,
    pub max_result_json_bytes: usize,
}

impl Default for ApiVmConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            memory_limit_bytes: 16 * 1024 * 1024,
            max_stack_size_bytes: 512 * 1024,
            max_pending_jobs: 1024,
            max_console_entries: 64,
            max_console_message_chars: 2048,
            max_result_json_bytes: 256 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HostDidAuthConfig {
    pub did_document_path: PathBuf,
    pub private_key_path: PathBuf,
    pub check_private_key_permissions: bool,
    session_manager: DidAuthSessionManager,
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
            session_manager: DidAuthSessionManager::new(),
        }
    }

    pub fn without_private_key_permission_check(mut self) -> Self {
        self.check_private_key_permissions = false;
        self
    }

    pub fn with_session_manager(mut self, session_manager: DidAuthSessionManager) -> Self {
        self.session_manager = session_manager;
        self
    }

    pub fn session_manager(&self) -> &DidAuthSessionManager {
        &self.session_manager
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
    #[serde(skip)]
    pub trace: Option<TraceContext>,
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
            trace: None,
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
            trace: context.trace.clone(),
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

    #[error("API `{0}` result exceeded size limit of {1} bytes")]
    ResultTooLarge(String, usize),

    #[error("QuickJS pending job limit exceeded")]
    PendingJobLimitExceeded,

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
            | Self::ResultTooLarge(_, _)
            | Self::PendingJobLimitExceeded
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
    storage: InMemoryScopedStorage,
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
            storage: InMemoryScopedStorage::new(),
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
        self.call_with_host_did_auth_and_metrics(call, host_did_auth, NoopMetricsSink)
    }

    fn call_with_host_did_auth_and_metrics<M>(
        &self,
        call: ApiCall,
        host_did_auth: Option<HostDidAuthConfig>,
        metrics: M,
    ) -> Result<AtomicApiResult, ApiVmError>
    where
        M: MetricsSink + 'static,
    {
        if !self
            .registered_apis
            .iter()
            .any(|registered| registered.name == call.api_name)
        {
            return Err(ApiVmError::MissingApi(call.api_name));
        }

        execute_api_call(
            &self.skill,
            &self.modules,
            &self.config,
            call,
            host_did_auth,
            self.storage.clone(),
            metrics,
        )
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
    metrics: NoopMetricsSink,
}

impl QuickJsApiExecutor {
    pub fn new(vm: ApiVm) -> Self {
        Self {
            vm,
            host_did_auth: None,
            metrics: NoopMetricsSink,
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

impl QuickJsApiExecutor {
    pub fn with_metrics<M>(self, metrics: M) -> QuickJsApiExecutorWithMetrics<M>
    where
        M: MetricsSink + 'static,
    {
        QuickJsApiExecutorWithMetrics {
            vm: self.vm,
            host_did_auth: self.host_did_auth,
            metrics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuickJsApiExecutorWithMetrics<M = NoopMetricsSink> {
    vm: ApiVm,
    host_did_auth: Option<HostDidAuthConfig>,
    metrics: M,
}

impl<M> QuickJsApiExecutorWithMetrics<M>
where
    M: MetricsSink + 'static,
{
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
        execute_with_metrics(
            &self.vm,
            context,
            self.host_did_auth.clone(),
            self.metrics.clone(),
        )
    }
}

impl<M> ApiExecutor for QuickJsApiExecutorWithMetrics<M>
where
    M: MetricsSink + 'static,
{
    fn execute(
        &self,
        context: &ApiCallContext,
        _component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError> {
        execute_with_metrics(
            &self.vm,
            context,
            self.host_did_auth.clone(),
            self.metrics.clone(),
        )
    }
}

fn execute_with_metrics<M>(
    vm: &ApiVm,
    context: &ApiCallContext,
    host_did_auth: Option<HostDidAuthConfig>,
    metrics: M,
) -> Result<AtomicApiResult, DockCoreError>
where
    M: MetricsSink + 'static,
{
    let started = Instant::now();
    let trace = context.trace.clone().unwrap_or_else(TraceContext::root);
    let span = trace.child();
    let mut call = ApiCall::from(context);
    call.trace = Some(span.clone());
    let result = vm.call_with_host_did_auth_and_metrics(call, host_did_auth, metrics.clone());
    let latency = elapsed_ms(started);
    let outcome = result
        .as_ref()
        .map(|result| if result.is_error { "error" } else { "ok" })
        .unwrap_or("vm_failed");
    metrics.record_metric(
        ObservabilityMetric::histogram_ms("dock.api_vm_execution_ms", latency)
            .with_label("api", context.api_name.clone())
            .with_label("outcome", outcome)
            .with_trace_id(span.trace_id.clone()),
    );
    metrics.record_span(
        TraceSpan::new(span, TraceSpanKind::WxApiCall, "quickjs.apiVm.execute")
            .with_attribute("api", context.api_name.clone())
            .with_outcome(outcome)
            .with_latency_ms(latency),
    );
    result.map_err(Into::into)
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
        drain_jobs(&ctx, config.max_pending_jobs)?;

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

fn execute_api_call<M>(
    skill: &LoadedSkill,
    modules: &CommonJsModules,
    config: &ApiVmConfig,
    call: ApiCall,
    host_did_auth: Option<HostDidAuthConfig>,
    storage: InMemoryScopedStorage,
    metrics: M,
) -> Result<AtomicApiResult, ApiVmError>
where
    M: MetricsSink + 'static,
{
    let api_name = call.api_name.clone();
    let bridge =
        HostBridgeRuntime::for_call(skill.clone(), call.clone(), host_did_auth, storage, metrics);
    let runtime_bridge = bridge.clone();
    let (result, _trace) = with_runtime(modules, config, runtime_bridge, |ctx| {
        let load_entry: Function = ctx
            .globals()
            .get("__dockLoadEntry")
            .map_err(to_quickjs_error)?;
        load_entry
            .call::<_, ()>(())
            .catch(&ctx)
            .map_err(caught_error)?;
        drain_jobs(&ctx, config.max_pending_jobs)?;

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
        if result_json.len() > config.max_result_json_bytes {
            return Err(ApiVmError::ResultTooLarge(
                api_name.clone(),
                config.max_result_json_bytes,
            ));
        }

        let mut result = serde_json::from_str::<AtomicApiResult>(&result_json).map_err(|_| {
            ApiVmError::InvalidResult(api_name.clone(), "schema validation failed".to_owned())
        })?;
        bridge.attach_model_context_meta(&mut result);
        Ok(result)
    })?;
    Ok(result)
}

fn with_runtime<R, M>(
    modules: &CommonJsModules,
    config: &ApiVmConfig,
    bridge: HostBridgeRuntime<M>,
    callback: impl for<'js> FnOnce(Ctx<'js>) -> Result<R, ApiVmError>,
) -> Result<(R, ExecutionTrace), ApiVmError>
where
    M: MetricsSink + 'static,
{
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
        .with::<rquickjs::context::intrinsic::Proxy>()
        .build(&runtime)
        .map_err(to_quickjs_error)?;
    let console = Rc::new(RefCell::new(Vec::new()));
    let modules_json = serde_json::to_string(&modules.to_json_value())
        .map_err(|error| ApiVmError::InvalidJson("__modules".to_owned(), error.to_string()))?;

    let result = context.with(|ctx| {
        install_host_bridge(ctx.clone(), modules_json, console.clone(), bridge, config)?;
        ctx.eval::<(), _>(runtime_bootstrap())
            .catch(&ctx)
            .map_err(caught_error)?;

        callback(ctx)
    });

    let trace = ExecutionTrace {
        console: console.borrow().clone(),
    };

    runtime.set_interrupt_handler(None);

    result.map(|value| (value, trace))
}

fn install_host_bridge<'js, M>(
    ctx: Ctx<'js>,
    modules_json: String,
    console: Rc<RefCell<Vec<ConsoleEntry>>>,
    bridge: HostBridgeRuntime<M>,
    config: &ApiVmConfig,
) -> Result<(), ApiVmError>
where
    M: MetricsSink + 'static + 'js,
{
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

    let check_session_bridge = bridge.clone();
    let check_session_fn = Func::from(move || check_session_bridge.check_session_json());
    dock.set("checkSession", check_session_fn)
        .map_err(to_quickjs_error)?;

    let request_bridge = bridge.clone();
    let request_fn =
        Func::from(move |options_json: String| request_bridge.request_json(options_json));
    dock.set("request", request_fn).map_err(to_quickjs_error)?;

    let get_storage_bridge = bridge.clone();
    let get_storage_fn =
        Func::from(move |options_json: String| get_storage_bridge.get_storage_json(options_json));
    dock.set("getStorage", get_storage_fn)
        .map_err(to_quickjs_error)?;

    let set_storage_bridge = bridge.clone();
    let set_storage_fn =
        Func::from(move |options_json: String| set_storage_bridge.set_storage_json(options_json));
    dock.set("setStorage", set_storage_fn)
        .map_err(to_quickjs_error)?;

    let remove_storage_bridge = bridge.clone();
    let remove_storage_fn = Func::from(move |options_json: String| {
        remove_storage_bridge.remove_storage_json(options_json)
    });
    dock.set("removeStorage", remove_storage_fn)
        .map_err(to_quickjs_error)?;

    let clear_storage_bridge = bridge.clone();
    let clear_storage_fn = Func::from(move || clear_storage_bridge.clear_storage_json());
    dock.set("clearStorage", clear_storage_fn)
        .map_err(to_quickjs_error)?;

    let device_info_bridge = bridge.clone();
    let device_info_fn = Func::from(move || device_info_bridge.device_info_json());
    dock.set("getDeviceInfo", device_info_fn)
        .map_err(to_quickjs_error)?;

    let app_base_info_bridge = bridge.clone();
    let app_base_info_fn = Func::from(move || app_base_info_bridge.app_base_info_json());
    dock.set("getAppBaseInfo", app_base_info_fn)
        .map_err(to_quickjs_error)?;

    let high_risk_bridge = bridge.clone();
    let high_risk_fn = Func::from(move |api_name: String, options_json: String| {
        high_risk_bridge.high_risk_api_json(api_name, options_json)
    });
    dock.set("highRiskApi", high_risk_fn)
        .map_err(to_quickjs_error)?;

    let unsupported_api_fn =
        Func::from(move |api_name: String| Value::Object(unsupported_api(&api_name)).to_string());
    dock.set("unsupportedApi", unsupported_api_fn)
        .map_err(to_quickjs_error)?;

    let session_bridge = bridge.clone();
    let get_session_id_fn = Func::from(move || session_bridge.session_id());
    dock.set("modelContextGetSessionId", get_session_id_fn)
        .map_err(to_quickjs_error)?;

    let expire_bridge = bridge.clone();
    let expire_all_cards_fn =
        Func::from(move |options_json: String| expire_bridge.expire_all_cards_json(options_json));
    dock.set("modelContextExpireAllCards", expire_all_cards_fn)
        .map_err(to_quickjs_error)?;

    let max_console_entries = config.max_console_entries;
    let max_console_message_chars = config.max_console_message_chars;
    let log_fn = Func::from(move |level: String, args: Vec<String>| {
        let level = match level.as_str() {
            "warn" => ConsoleLevel::Warn,
            "error" => ConsoleLevel::Error,
            _ => ConsoleLevel::Log,
        };
        let mut entries = console.borrow_mut();
        if max_console_entries == 0 {
            return;
        }
        if entries.len() >= max_console_entries {
            if let Some(last) = entries.last_mut() {
                last.level = ConsoleLevel::Warn;
                last.message = "[console output truncated]".to_owned();
            }
            return;
        }
        let message = truncate_chars(&args.join(" "), max_console_message_chars);
        entries.push(ConsoleEntry { level, message });
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageOptions {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpireAllCardsOptions {
    #[serde(default)]
    component_paths: Vec<String>,
    #[serde(default, rename = "match")]
    match_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelContextCardEvent {
    #[serde(rename = "type")]
    event_type: &'static str,
    component_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_policy: Option<String>,
}

#[derive(Debug, Clone)]
struct HostBridgeRuntime<M = NoopMetricsSink> {
    skill: Option<LoadedSkill>,
    call: Option<ApiCall>,
    host_did_auth: Option<HostDidAuthConfig>,
    storage: InMemoryScopedStorage,
    card_events: Rc<RefCell<Vec<ModelContextCardEvent>>>,
    metrics: M,
}

impl HostBridgeRuntime<NoopMetricsSink> {
    fn registration() -> Self {
        Self {
            skill: None,
            call: None,
            host_did_auth: None,
            storage: InMemoryScopedStorage::new(),
            card_events: Rc::new(RefCell::new(Vec::new())),
            metrics: NoopMetricsSink,
        }
    }
}

impl<M> HostBridgeRuntime<M>
where
    M: MetricsSink + 'static,
{
    fn for_call(
        skill: LoadedSkill,
        call: ApiCall,
        host_did_auth: Option<HostDidAuthConfig>,
        storage: InMemoryScopedStorage,
        metrics: M,
    ) -> Self {
        Self {
            skill: Some(skill),
            call: Some(call),
            host_did_auth,
            storage,
            card_events: Rc::new(RefCell::new(Vec::new())),
            metrics,
        }
    }

    fn session_id(&self) -> String {
        self.call
            .as_ref()
            .map(|call| call.session_id.clone())
            .unwrap_or_default()
    }

    fn expire_all_cards_json(&self, options_json: String) -> String {
        match self.expire_all_cards(&options_json) {
            Ok(expired_count) => json!({
                "errMsg": "modelContext.expireAllCards:ok",
                "expiredCount": expired_count,
            })
            .to_string(),
            Err(message) => json!({
                "errMsg": format!("modelContext.expireAllCards:fail {message}"),
                "code": "invalid_options",
                "reason": message,
                "suggestion": "Declare expirable components and pass safe relative componentPaths from mcp.json."
            })
            .to_string(),
        }
    }

    fn expire_all_cards(&self, options_json: &str) -> Result<usize, String> {
        let skill = self
            .skill
            .as_ref()
            .ok_or_else(|| "modelContext is unavailable during API registration".to_owned())?;
        let options: ExpireAllCardsOptions = serde_json::from_str(options_json).map_err(|_| {
            "options must be an object with componentPaths array and match policy".to_owned()
        })?;
        if let Some(match_policy) = options.match_policy.as_deref() {
            if !matches!(match_policy, "latest" | "session" | "all") {
                return Err("match must be one of latest, session, or all".to_owned());
            }
        }

        let mut component_paths = if options.component_paths.is_empty() {
            skill
                .manifest
                .components
                .iter()
                .filter(|component| component.expirable == Some(true))
                .map(|component| component.path.clone())
                .collect::<Vec<_>>()
        } else {
            options.component_paths
        };
        component_paths.sort();
        component_paths.dedup();

        if component_paths.is_empty() {
            return Err("no expirable componentPaths were declared or provided".to_owned());
        }

        for component_path in &component_paths {
            resolve_component_path(&skill.root, component_path).map_err(|_| {
                "componentPaths contains a path outside the Skill package".to_owned()
            })?;
            let Some(component) = skill
                .manifest
                .components
                .iter()
                .find(|component| component.path == *component_path)
            else {
                return Err("componentPaths contains an undeclared component".to_owned());
            };
            if component.expirable != Some(true) {
                return Err(
                    "componentPaths contains a component without expirable: true".to_owned(),
                );
            }
        }

        let expired_count = component_paths.len();
        self.card_events.borrow_mut().push(ModelContextCardEvent {
            event_type: "expireAllCards",
            component_paths,
            match_policy: options.match_policy,
        });
        Ok(expired_count)
    }

    fn attach_model_context_meta(&self, result: &mut AtomicApiResult) {
        let card_events = self.card_events.borrow();
        if card_events.is_empty() {
            return;
        }
        let Ok(card_events) = serde_json::to_value(&*card_events) else {
            return;
        };
        let meta = result.meta.get_or_insert_with(Default::default);
        let model_context = meta
            .entry("modelContext".to_owned())
            .or_insert_with(|| json!({}));
        if let Some(object) = model_context.as_object_mut() {
            object.insert("cardEvents".to_owned(), card_events);
        } else {
            meta.insert(
                "modelContext".to_owned(),
                json!({ "cardEvents": card_events }),
            );
        }
    }

    fn get_storage_json(&self, options_json: String) -> String {
        match self.storage_key_from_options("getStorage", &options_json) {
            Ok((scope, key)) => match self.storage.get_storage(&scope, &key) {
                Ok(Some(data)) => json!({
                    "errMsg": "getStorage:ok",
                    "data": data,
                })
                .to_string(),
                Ok(None) => storage_failure_json(
                    "getStorage",
                    "invalid_options",
                    "storage key was not found in this scope",
                    "Initialize the key with wx.setStorage before reading it.",
                ),
                Err(error) => storage_error_json("getStorage", error),
            },
            Err(failure) => failure.to_json().to_string(),
        }
    }

    fn set_storage_json(&self, options_json: String) -> String {
        match self.storage_key_and_data_from_options("setStorage", &options_json) {
            Ok((scope, key, data)) => match self.storage.set_storage(&scope, key, data) {
                Ok(()) => json!({ "errMsg": "setStorage:ok" }).to_string(),
                Err(error) => storage_error_json("setStorage", error),
            },
            Err(failure) => failure.to_json().to_string(),
        }
    }

    fn remove_storage_json(&self, options_json: String) -> String {
        match self.storage_key_from_options("removeStorage", &options_json) {
            Ok((scope, key)) => match self.storage.remove_storage(&scope, &key) {
                Ok(_) => json!({ "errMsg": "removeStorage:ok" }).to_string(),
                Err(error) => storage_error_json("removeStorage", error),
            },
            Err(failure) => failure.to_json().to_string(),
        }
    }

    fn clear_storage_json(&self) -> String {
        match self.storage_scope("clearStorage") {
            Ok(scope) => match self.storage.clear_storage(&scope) {
                Ok(()) => json!({ "errMsg": "clearStorage:ok" }).to_string(),
                Err(error) => storage_error_json("clearStorage", error),
            },
            Err(failure) => failure.to_json().to_string(),
        }
    }

    fn storage_key_from_options(
        &self,
        api_name: &'static str,
        options_json: &str,
    ) -> Result<(StorageScope, String), StorageFailure> {
        let options = storage_options(api_name, options_json)?;
        let key = options
            .key
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| {
                StorageFailure::invalid_options(
                    api_name,
                    "storage key must be a non-empty string",
                    "Pass options.key for async storage APIs.",
                )
            })?;
        let scope = self.storage_scope(api_name)?;
        Ok((scope, key))
    }

    fn storage_key_and_data_from_options(
        &self,
        api_name: &'static str,
        options_json: &str,
    ) -> Result<(StorageScope, String, Value), StorageFailure> {
        let options = storage_options(api_name, options_json)?;
        let key = options
            .key
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| {
                StorageFailure::invalid_options(
                    api_name,
                    "storage key must be a non-empty string",
                    "Pass options.key for async storage APIs.",
                )
            })?;
        let data = options.data.ok_or_else(|| {
            StorageFailure::invalid_options(
                api_name,
                "storage data must be provided",
                "Pass options.data for setStorage.",
            )
        })?;
        let scope = self.storage_scope(api_name)?;
        Ok((scope, key, data))
    }

    fn storage_scope(&self, api_name: &'static str) -> Result<StorageScope, StorageFailure> {
        let Some(call) = &self.call else {
            return Err(StorageFailure::provider_unavailable(
                api_name,
                "storage is unavailable during API registration",
                "Call storage APIs from a registered API handler.",
            ));
        };
        let Some(user_did) = call
            .user_did
            .clone()
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(StorageFailure::provider_unavailable(
                api_name,
                "storage scope requires userDid",
                "Provide userDid in the ApiCallContext before using wx storage.",
            ));
        };
        let Some(merchant_did) = call
            .merchant_did
            .clone()
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(StorageFailure::provider_unavailable(
                api_name,
                "storage scope requires merchantDid",
                "Provide merchantDid in the ApiCallContext before using wx storage.",
            ));
        };
        Ok(StorageScope::new(
            user_did,
            merchant_did,
            call.skill_id.clone(),
        ))
    }

    fn device_info_json(&self) -> String {
        serde_json::to_string(&DeviceInfo::default()).expect("device info must serialize")
    }

    fn app_base_info_json(&self) -> String {
        serde_json::to_string(&AppBaseInfo::default()).expect("app base info must serialize")
    }

    fn high_risk_api_json(&self, api_name: String, options_json: String) -> String {
        let Some(spec) = high_risk_api_spec(&api_name) else {
            return serde_json::Value::Object(unsupported_api(&api_name)).to_string();
        };
        let options: Value = match serde_json::from_str(&options_json) {
            Ok(options) => options,
            Err(_) => {
                return high_risk_error_json(spec.name, wx_compat::HighRiskApiError::InvalidOptions)
                    .to_string()
            }
        };
        if high_risk_options_contain_local_file_path(&options) {
            return high_risk_error_json(spec.name, wx_compat::HighRiskApiError::InvalidOptions)
                .to_string();
        }
        if options
            .get("__dockConsentRequired")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return high_risk_consent_required_json(spec.name).to_string();
        }
        let request = HighRiskApiRequest::new(spec.name, options);
        let provider = UnavailableHighRiskHostProvider;
        match provider.call_high_risk_api(spec, &request) {
            Ok(success) => high_risk_success_json(spec.name, success).to_string(),
            Err(error) => high_risk_error_json(spec.name, error).to_string(),
        }
    }

    fn login_json(&self) -> String {
        let started = Instant::now();
        match self.ensure_login(None) {
            Ok(Some((key, session))) => {
                let receipt = DidAuthReceipt::from_session(&key, &session);
                self.record_token_metric("wx.login", "ok", started);
                json!({
                    "code": receipt.code,
                    "errMsg": "login:ok",
                    "didAuth": {
                        "status": "ok",
                        "tokenReceived": receipt.token_received,
                        "tokenVisibleToSkill": receipt.token_visible_to_skill,
                        "userDid": receipt.user_did,
                        "agentDid": receipt.agent_did,
                        "merchantDid": receipt.merchant_did,
                        "scopes": receipt.scopes
                    }
                })
                .to_string()
            }
            Ok(None) => {
                self.record_token_metric("wx.login", "mock", started);
                json!({
                    "code": "dock-login-code-localhost",
                    "errMsg": "login:ok",
                    "didAuth": {
                        "status": "mock",
                        "tokenReceived": false,
                        "tokenVisibleToSkill": false
                    }
                })
                .to_string()
            }
            Err(message) => {
                self.record_token_metric("wx.login", "auth_failed", started);
                json!({
                    "errMsg": format!("login:fail {message}")
                })
                .to_string()
            }
        }
    }

    fn check_session_json(&self) -> String {
        let started = Instant::now();
        match self.check_session() {
            Ok(true) => {
                self.record_token_metric("wx.checkSession", "ok", started);
                json!({
                    "errMsg": "checkSession:ok"
                })
                .to_string()
            }
            Ok(false) => {
                self.record_token_metric("wx.checkSession", "auth_failed", started);
                json!({
                    "errMsg": "checkSession:fail auth_failed",
                    "code": "auth_failed",
                    "reason": "DID auth session is not configured for this call"
                })
                .to_string()
            }
            Err(message) => {
                self.record_token_metric("wx.checkSession", "auth_failed", started);
                json!({
                    "errMsg": "checkSession:fail auth_failed",
                    "code": "auth_failed",
                    "reason": message
                })
                .to_string()
            }
        }
    }

    fn request_json(&self, options_json: String) -> String {
        let started = Instant::now();
        match self.host_request(&options_json) {
            Ok(value) => {
                let status = value
                    .get("statusCode")
                    .and_then(Value::as_u64)
                    .map(request_status_label)
                    .unwrap_or("unknown");
                self.record_request_metric(status, started);
                value.to_string()
            }
            Err(error) => {
                self.record_request_metric(error.code, started);
                error.to_json().to_string()
            }
        }
    }

    fn record_token_metric(&self, api_name: &str, outcome: &str, started: Instant) {
        let latency = elapsed_ms(started);
        let trace = self.trace_child();
        self.metrics.record_metric(
            ObservabilityMetric::counter("dock.token_refresh.total", 1)
                .with_label("api", api_name)
                .with_label("outcome", outcome)
                .with_trace_id(trace.trace_id.clone()),
        );
        self.metrics.record_metric(
            ObservabilityMetric::histogram_ms("dock.token_refresh_latency_ms", latency)
                .with_label("api", api_name)
                .with_label("outcome", outcome)
                .with_trace_id(trace.trace_id.clone()),
        );
        self.metrics.record_span(
            TraceSpan::new(trace, TraceSpanKind::Token, "quickjs.wxToken")
                .with_attribute("api", api_name)
                .with_outcome(outcome)
                .with_latency_ms(latency),
        );
    }

    fn record_request_metric(&self, outcome: &str, started: Instant) {
        let latency = elapsed_ms(started);
        let trace = self.trace_child();
        self.metrics.record_metric(
            ObservabilityMetric::counter("dock.request.total", 1)
                .with_label("api", "wx.request")
                .with_label("outcome", outcome)
                .with_trace_id(trace.trace_id.clone()),
        );
        self.metrics.record_metric(
            ObservabilityMetric::histogram_ms("dock.request_latency_ms", latency)
                .with_label("api", "wx.request")
                .with_label("outcome", outcome)
                .with_trace_id(trace.trace_id.clone()),
        );
        self.metrics.record_span(
            TraceSpan::new(trace, TraceSpanKind::Request, "quickjs.wx.request")
                .with_attribute("api", "wx.request")
                .with_outcome(outcome)
                .with_latency_ms(latency),
        );
    }

    fn trace_child(&self) -> TraceContext {
        self.call
            .as_ref()
            .and_then(|call| call.trace.as_ref())
            .map(TraceContext::child)
            .unwrap_or_else(TraceContext::root)
    }

    fn host_request(&self, options_json: &str) -> Result<Value, HostRequestFailure> {
        let options: HostRequestOptions = serde_json::from_str(options_json).map_err(|_| {
            HostRequestFailure::invalid_options("request options must be valid JSON")
        })?;
        let data = options.data.unwrap_or(Value::Null);
        let method = options
            .method
            .as_deref()
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let parsed =
            ParsedHttpUrl::parse(&options.url).map_err(HostRequestFailure::invalid_options)?;
        if !parsed.is_loopback() {
            return Err(HostRequestFailure::network_denied(
                "wx.request demo bridge only allows localhost URLs",
            ));
        }
        if parsed.scheme != "http" {
            return Err(HostRequestFailure::invalid_options(
                "wx.request demo bridge only supports http:// localhost URLs",
            ));
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

        if let Some(name) = host_owned_header(&options.header) {
            return Err(HostRequestFailure::permission_denied(format!(
                "JS-provided {name} header is not allowed; host attaches auth material"
            )));
        }

        let session = self
            .ensure_login(Some(parsed.origin()))
            .map_err(HostRequestFailure::auth_failed)?;
        let bearer = session
            .as_ref()
            .map(|(_, session)| session.bearer_token().to_owned());
        let method = wx_method(&method).map_err(HostRequestFailure::invalid_options)?;

        let broker = LocalDidRequestBroker { bearer };
        let response = broker
            .request(
                &CapabilityProfile::atomic_api(),
                WxRequest {
                    url: request_url,
                    method,
                    headers: options.header,
                    data: if body.is_empty() {
                        None
                    } else {
                        Some(Value::String(body))
                    },
                },
            )
            .map_err(HostRequestFailure::from_request_error)?;

        Ok(json!({
            "statusCode": response.status_code,
            "header": response.headers,
            "data": response.data,
            "errMsg": "request:ok"
        }))
    }

    fn ensure_login(
        &self,
        request_origin: Option<String>,
    ) -> Result<Option<(DidAuthSessionKey, DidAuthSession)>, String> {
        let Some(auth_config) = &self.host_did_auth else {
            return Ok(None);
        };
        let Some(key) = self.session_key(request_origin)? else {
            return Ok(None);
        };
        auth_config
            .session_manager
            .ensure_session(key.clone(), |key| {
                self.perform_did_login(auth_config, key)
                    .map_err(DidAuthSessionError::LoginFailed)
            })
            .map(|session| Some((key, session)))
            .map_err(safe_did_session_error)
    }

    fn check_session(&self) -> Result<bool, String> {
        let Some(auth_config) = &self.host_did_auth else {
            return Ok(false);
        };
        let Some(key) = self.session_key(None)? else {
            return Ok(false);
        };
        auth_config
            .session_manager
            .check_session(&key)
            .map(|_| true)
            .map_err(safe_did_session_error)
    }

    fn session_key(
        &self,
        request_origin: Option<String>,
    ) -> Result<Option<DidAuthSessionKey>, String> {
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
        Ok(Some(DidAuthSessionKey::new(
            base_url,
            merchant_did,
            user_did,
            call.agent_did.clone(),
            call.skill_id.clone(),
            call.session_id.clone(),
        )))
    }

    fn perform_did_login(
        &self,
        auth_config: &HostDidAuthConfig,
        key: &DidAuthSessionKey,
    ) -> Result<DidAuthSession, String> {
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
        let scopes = decode_capability_token_scopes(&login.capability_token).unwrap_or_else(|| {
            vec![
                "coffee:drinks:read".to_owned(),
                "coffee:order:confirm".to_owned(),
                "coffee:order:pay".to_owned(),
                "coffee:order:read".to_owned(),
            ]
        });
        Ok(DidAuthSession::new(
            login.capability_token,
            login.expires_at_ms,
            scopes,
        ))
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostRequestFailure {
    code: &'static str,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageFailure {
    api_name: &'static str,
    code: &'static str,
    reason: &'static str,
    suggestion: &'static str,
}

impl StorageFailure {
    fn invalid_options(
        api_name: &'static str,
        reason: &'static str,
        suggestion: &'static str,
    ) -> Self {
        Self {
            api_name,
            code: "invalid_options",
            reason,
            suggestion,
        }
    }

    fn provider_unavailable(
        api_name: &'static str,
        reason: &'static str,
        suggestion: &'static str,
    ) -> Self {
        Self {
            api_name,
            code: "provider_unavailable",
            reason,
            suggestion,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "errMsg": format!("{}:fail {}", self.api_name, self.code),
            "code": self.code,
            "reason": self.reason,
            "suggestion": self.suggestion,
        })
    }
}

fn storage_options(
    api_name: &'static str,
    options_json: &str,
) -> Result<StorageOptions, StorageFailure> {
    serde_json::from_str(options_json).map_err(|_| {
        StorageFailure::invalid_options(
            api_name,
            "storage options must be a JSON object",
            "Pass JSON-safe storage options without functions, symbols, or cyclic values.",
        )
    })
}

fn storage_error_json(api_name: &'static str, error: StorageError) -> String {
    let (code, reason, suggestion) = match error {
        StorageError::EmptyKey => (
            "invalid_options",
            "storage key must be a non-empty string",
            "Pass a non-empty key string.",
        ),
        StorageError::KeyContainsNul => (
            "invalid_options",
            "storage key contains an unsupported character",
            "Use storage keys without NUL bytes.",
        ),
        StorageError::KeyTooLarge => (
            "invalid_options",
            "storage key is too large",
            "Use a shorter storage key.",
        ),
        StorageError::SensitiveKey => (
            "permission_denied",
            "storage key is reserved for sensitive data",
            "Do not store tokens, authorization headers, private keys, phone numbers, addresses, or file contents in wx storage.",
        ),
        StorageError::ValueTooLarge => (
            "invalid_options",
            "storage value is too large",
            "Store a smaller JSON-safe value or keep large data in a Host-managed backend.",
        ),
        StorageError::ValueNotJsonSafe => (
            "invalid_options",
            "storage value is not JSON-safe",
            "Store only JSON-safe values.",
        ),
        StorageError::InvalidScope => (
            "provider_unavailable",
            "storage scope is unavailable",
            "Provide userDid, merchantDid, skillId, and namespace before using wx storage.",
        ),
        StorageError::QuotaExceeded => (
            "quota_exceeded",
            "storage quota exceeded",
            "Remove unused keys before writing more data.",
        ),
        StorageError::LockPoisoned | StorageError::BackendUnavailable => (
            "provider_unavailable",
            "storage backend is unavailable",
            "Retry later or use Host-managed state.",
        ),
        StorageError::BackendCorrupt => (
            "provider_unavailable",
            "storage backend data is unavailable",
            "Ask the Host to repair or reset scoped storage for this Skill.",
        ),
    };
    json!({
        "errMsg": format!("{api_name}:fail {code}"),
        "code": code,
        "reason": reason,
        "suggestion": suggestion,
    })
    .to_string()
}

fn storage_failure_json(
    api_name: &'static str,
    code: &'static str,
    reason: &'static str,
    suggestion: &'static str,
) -> String {
    json!({
        "errMsg": format!("{api_name}:fail {code}"),
        "code": code,
        "reason": reason,
        "suggestion": suggestion,
    })
    .to_string()
}

fn high_risk_options_contain_local_file_path(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let normalized = key.to_ascii_lowercase();
            (normalized.contains("path") || normalized.contains("file"))
                && value.as_str().is_some_and(looks_like_local_file_path)
                || high_risk_options_contain_local_file_path(value)
        }),
        Value::Array(items) => items.iter().any(high_risk_options_contain_local_file_path),
        Value::String(text) => looks_like_local_file_path(text),
        _ => false,
    }
}

fn looks_like_local_file_path(text: &str) -> bool {
    text.starts_with('/')
        || text.starts_with("\\\\")
        || text.contains(":\\")
        || text.starts_with("file:")
}

impl HostRequestFailure {
    fn invalid_options(reason: impl Into<String>) -> Self {
        Self {
            code: "invalid_options",
            reason: reason.into(),
        }
    }

    fn permission_denied(reason: impl Into<String>) -> Self {
        Self {
            code: "permission_denied",
            reason: reason.into(),
        }
    }

    fn network_denied(reason: impl Into<String>) -> Self {
        Self {
            code: "network_denied",
            reason: reason.into(),
        }
    }

    fn auth_failed(reason: impl Into<String>) -> Self {
        Self {
            code: "auth_failed",
            reason: reason.into(),
        }
    }

    fn transport_failed(reason: impl Into<String>) -> Self {
        Self {
            code: "transport_failed",
            reason: reason.into(),
        }
    }

    fn from_request_error(error: WxRequestError) -> Self {
        match error {
            WxRequestError::Denied(reason) => Self::network_denied(reason.redacted_for_display()),
            WxRequestError::Transport(reason) => {
                Self::transport_failed(reason.redacted_for_display())
            }
            WxRequestError::Unsupported(reason) => {
                Self::transport_failed(reason.redacted_for_display())
            }
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "errMsg": format!("request:fail {}", self.code),
            "code": self.code,
            "reason": self.reason,
        })
    }
}

#[derive(Debug, Clone)]
struct LocalDidRequestBroker {
    bearer: Option<String>,
}

impl RequestBroker for LocalDidRequestBroker {
    fn request(
        &self,
        profile: &CapabilityProfile,
        request: WxRequest,
    ) -> Result<WxResponse, WxRequestError> {
        profile
            .ensure(wx_compat::Capability::Request)
            .map_err(|denial| match denial {
                wx_compat::PermissionDecision::Deny { reason, .. }
                | wx_compat::PermissionDecision::Prompt { reason, .. } => {
                    WxRequestError::Denied(reason)
                }
                wx_compat::PermissionDecision::MockAllowed { reason, .. } => {
                    WxRequestError::Unsupported(format!(
                        "mock request permission cannot use local DID transport: {reason}"
                    ))
                }
                wx_compat::PermissionDecision::Allow => {
                    WxRequestError::Denied("request capability is unavailable".to_owned())
                }
            })?;

        let body = request
            .data
            .as_ref()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let (status_code, headers, response_body) = http_request_url(
            &request.url,
            wx_method_name(request.method),
            self.bearer.as_deref(),
            Some(&body),
            request.headers,
        )
        .map_err(|message| WxRequestError::Transport(message.redacted_for_display()))?;
        let data =
            serde_json::from_str::<Value>(&response_body).unwrap_or(Value::String(response_body));

        Ok(WxResponse {
            status_code,
            headers: redact_response_headers(headers),
            data,
        })
    }
}

fn wx_method(method: &str) -> Result<WxMethod, String> {
    match method {
        "GET" => Ok(WxMethod::Get),
        "POST" => Ok(WxMethod::Post),
        "PUT" => Ok(WxMethod::Put),
        "DELETE" => Ok(WxMethod::Delete),
        "PATCH" => Ok(WxMethod::Patch),
        _ => Err(format!("unsupported request method: {method}")),
    }
}

fn wx_method_name(method: WxMethod) -> &'static str {
    match method {
        WxMethod::Get => "GET",
        WxMethod::Post => "POST",
        WxMethod::Put => "PUT",
        WxMethod::Delete => "DELETE",
        WxMethod::Patch => "PATCH",
    }
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

fn safe_did_session_error(error: DidAuthSessionError) -> String {
    match error {
        DidAuthSessionError::Unavailable => "DID auth session cache is unavailable".to_owned(),
        DidAuthSessionError::Missing => "DID auth session is missing".to_owned(),
        DidAuthSessionError::Expired => "DID auth session is expired".to_owned(),
        DidAuthSessionError::LoginFailed(message) => message.redacted_for_display(),
    }
}

fn host_owned_header(headers: &BTreeMap<String, String>) -> Option<&str> {
    headers
        .keys()
        .find(|name| {
            name.eq_ignore_ascii_case("authorization")
                || name.eq_ignore_ascii_case("signature")
                || name.eq_ignore_ascii_case("signature-input")
                || name.eq_ignore_ascii_case("cookie")
        })
        .map(String::as_str)
}

fn redact_response_headers(headers: BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .into_iter()
        .filter(|(name, _)| {
            !name.eq_ignore_ascii_case("authorization")
                && !name.eq_ignore_ascii_case("authentication-info")
                && !name.eq_ignore_ascii_case("set-cookie")
                && !name.eq_ignore_ascii_case("signature")
                && !name.eq_ignore_ascii_case("signature-input")
                && !name.eq_ignore_ascii_case("cookie")
                && !name.to_ascii_lowercase().contains("token")
        })
        .collect()
}

fn request_status_label(status: u64) -> &'static str {
    match status {
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "other",
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    let millis = started.elapsed().as_millis();
    millis.min(u128::from(u64::MAX)) as u64
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
    scheme: String,
    host: String,
    port: u16,
    path_with_query: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| "URL scheme is required".to_owned())?;
        if scheme != "http" && scheme != "https" {
            return Err("only http:// or https:// URLs are supported".to_owned());
        }
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
            scheme: scheme.to_owned(),
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
        format!("{}://{}", self.scheme, self.host_header())
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

fn drain_jobs(ctx: &Ctx<'_>, max_pending_jobs: usize) -> Result<(), ApiVmError> {
    let mut drained = 0;
    while ctx.execute_pending_job() {
        drained += 1;
        if drained > max_pending_jobs {
            return Err(ApiVmError::PendingJobLimitExceeded);
        }
    }
    Ok(())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    const MARKER: &str = "...[TRUNCATED]";
    let marker_chars = MARKER.chars().count();
    if max_chars <= marker_chars {
        return MARKER.chars().take(max_chars).collect();
    }
    let prefix_len = max_chars - marker_chars;
    let mut truncated = value.chars().take(prefix_len).collect::<String>();
    truncated.push_str(MARKER);
    truncated
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
