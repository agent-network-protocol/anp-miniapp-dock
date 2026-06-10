use crate::bridge::runtime_bootstrap;
use crate::commonjs::CommonJsModules;
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
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::{Duration, Instant};
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
        if !self
            .registered_apis
            .iter()
            .any(|registered| registered.name == call.api_name)
        {
            return Err(ApiVmError::MissingApi(call.api_name));
        }

        execute_api_call(&self.modules, &self.config, call)
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
}

impl QuickJsApiExecutor {
    pub fn new(vm: ApiVm) -> Self {
        Self { vm }
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
        self.vm.call(ApiCall::from(context)).map_err(Into::into)
    }
}

fn evaluate_registration(
    modules: &CommonJsModules,
    config: &ApiVmConfig,
) -> Result<(Vec<RegisteredApi>, ExecutionTrace), ApiVmError> {
    with_runtime(modules, config, |ctx| {
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
) -> Result<AtomicApiResult, ApiVmError> {
    let api_name = call.api_name.clone();
    let (result, _trace) = with_runtime(modules, config, |ctx| {
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
        install_host_bridge(ctx.clone(), modules_json, console.clone())?;
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
) -> Result<(), ApiVmError> {
    let dock = Object::new(ctx.clone()).map_err(to_quickjs_error)?;
    let modules_json_fn = {
        let modules_json = modules_json.clone();
        Func::from(move || modules_json.clone())
    };
    dock.set("modulesJson", modules_json_fn)
        .map_err(to_quickjs_error)?;

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
