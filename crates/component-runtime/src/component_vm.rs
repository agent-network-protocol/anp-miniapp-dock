use crate::compiler::{
    compile_component_to_render_ir, ComponentCompileError, ComponentRenderOutput,
};
use crate::events::ComponentEvent;
use crate::loader::ComponentPackage;
use rquickjs::function::IntoArgs;
use rquickjs::promise::MaybePromise;
use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Function, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const COMPONENT_BOOTSTRAP: &str = r#"
(() => {
'use strict';

let __dockDefinition = null;
let __dockInstance = null;
let __dockExpired = false;
let __dockActions = [];
let __dockTrace = [];
const __dockFunctionConstructor = Function;
const __dockAsyncFunctionPrototype = Object.getPrototypeOf(async function() {});
const __dockGeneratorFunctionPrototype = Object.getPrototypeOf(function* () {});
const __dockAsyncGeneratorFunctionPrototype = Object.getPrototypeOf(async function* () {});

function __dockClone(value) {
  if (value === undefined || value === null) {
    return value;
  }
  return JSON.parse(JSON.stringify(value));
}

function __dockSafeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function __dockSetByPath(target, path, value) {
  const parts = String(path).split('.').filter(Boolean);
  if (parts.length === 0) {
    return;
  }
  let current = target;
  for (let index = 0; index < parts.length - 1; index += 1) {
    const part = parts[index];
    if (!current[part] || typeof current[part] !== 'object' || Array.isArray(current[part])) {
      current[part] = {};
    }
    current = current[part];
  }
  current[parts[parts.length - 1]] = value;
}

function __dockApplySetData(target, partial) {
  const updates = __dockSafeObject(partial);
  for (const key of Object.keys(updates)) {
    if (key.includes('.')) {
      __dockSetByPath(target, key, __dockClone(updates[key]));
    } else {
      target[key] = __dockClone(updates[key]);
    }
  }
}

function __dockNormalizeType(type) {
  if (typeof type !== 'string') {
    return '';
  }
  return type.toLowerCase();
}

function __dockPushAction(action) {
  __dockActions.push(action);
  if (action.type === 'sendFollowUpMessage' && Array.isArray(action.content)) {
    for (const block of action.content) {
      if (block && block.type === 'api/call' && block.data && typeof block.data.name === 'string') {
        __dockActions.push({
          type: 'api/call',
          name: block.data.name,
          arguments: __dockSafeObject(block.data.arguments)
        });
      }
    }
  }
}

function __dockRegisterHandler(handlers, type, callback) {
  const normalized = __dockNormalizeType(type);
  if (typeof callback !== 'function') {
    throw new Error('notification handler must be a function');
  }
  if (!handlers[normalized]) {
    handlers[normalized] = [];
  }
  handlers[normalized].push(callback);
}

function __dockCreateModelContext(handlers) {
  return Object.freeze({
    on(type, callback) {
      __dockRegisterHandler(handlers, type, callback);
    },
    sendFollowUpMessage(message) {
      const payload = __dockSafeObject(message);
      __dockPushAction({
        type: 'sendFollowUpMessage',
        content: Array.isArray(payload.content) ? __dockClone(payload.content) : []
      });
      return { errMsg: 'sendFollowUpMessage:ok' };
    }
  });
}

function __dockCreateViewContext(handlers) {
  return Object.freeze({
    on(type, callback) {
      __dockRegisterHandler(handlers, type, callback);
    },
    getDimensions() {
      return Object.freeze({ minHeight: 120, maxHeight: 480, width: 360 });
    },
    expirePreviousCards(options) {
      const payload = __dockSafeObject(options);
      __dockPushAction({
        type: 'expirePreviousCards',
        componentPaths: Array.isArray(payload.componentPaths) ? __dockClone(payload.componentPaths) : [],
        match: typeof payload.match === 'string' ? payload.match : null
      });
      return { errMsg: 'expirePreviousCards:ok' };
    },
    openDetailPage(options) {
      const payload = __dockSafeObject(options);
      __dockPushAction({
        type: 'openDetailPage',
        url: typeof payload.url === 'string' ? payload.url : ''
      });
      return { errMsg: 'openDetailPage:ok' };
    },
    setRelatedPage(options) {
      const payload = __dockSafeObject(options);
      __dockPushAction({
        type: 'setRelatedPage',
        path: typeof payload.path === 'string' ? payload.path : null,
        query: payload.query === undefined ? null : __dockClone(payload.query)
      });
      return { errMsg: 'setRelatedPage:ok' };
    }
  });
}

function __dockBuildInstance(seedData, properties) {
  const defaults = __dockSafeObject(__dockDefinition.data);
  const handlers = Object.create(null);
  const componentProperties = __dockSafeObject(properties);
  const data = Object.assign({}, __dockClone(defaults), __dockClone(componentProperties), __dockSafeObject(seedData));
  const instance = {
    data,
    properties: componentProperties,
    setData(partial) {
      __dockApplySetData(data, partial);
      this.data = data;
    },
    triggerEvent() {
      return undefined;
    },
    __dockHandlers: handlers
  };

  const methods = __dockSafeObject(__dockDefinition.methods);
  for (const name of Object.keys(methods)) {
    if (typeof methods[name] === 'function') {
      Object.defineProperty(instance, name, {
        value: methods[name].bind(instance),
        configurable: false,
        writable: false
      });
    }
  }

  const modelContext = __dockCreateModelContext(handlers);
  const viewContext = __dockCreateViewContext(handlers);
  Object.defineProperty(instance, '__dockModelContext', { value: modelContext });
  Object.defineProperty(instance, '__dockViewContext', { value: viewContext });
  return instance;
}

async function __dockCallLifecycle(name) {
  const lifetimes = __dockSafeObject(__dockDefinition.lifetimes);
  const lifecycle = lifetimes[name];
  if (typeof lifecycle === 'function') {
    __dockTrace.push({ kind: 'lifecycle', name });
    await lifecycle.call(__dockInstance);
  }
}

async function __dockNotify(type, payload) {
  const normalized = __dockNormalizeType(type);
  const handlers = (__dockInstance.__dockHandlers && __dockInstance.__dockHandlers[normalized]) || [];
  __dockTrace.push({ kind: 'notification', name: normalized });
  for (const handler of handlers) {
    await handler.call(__dockInstance, payload);
  }
}

function __dockSnapshot() {
  const snapshot = {
    data: __dockInstance ? __dockClone(__dockInstance.data) : {},
    actions: __dockActions,
    trace: __dockTrace
  };
  __dockActions = [];
  __dockTrace = [];
  return JSON.stringify(snapshot);
}

function Component(definition) {
  if (__dockDefinition !== null) {
    throw new Error('Component() may only be called once');
  }
  if (!definition || typeof definition !== 'object') {
    throw new Error('Component() requires an object definition');
  }
  __dockDefinition = definition;
}

async function __dockMount(seedDataJson, propertiesJson, inputJson) {
  if (!__dockDefinition) {
    throw new Error('component did not call Component({})');
  }
  const input = JSON.parse(inputJson);
  __dockInstance = __dockBuildInstance(JSON.parse(seedDataJson), JSON.parse(propertiesJson));
  await __dockCallLifecycle('created');
  await __dockNotify('input', { apiName: input.apiName, arguments: input.arguments || {} });
  await __dockNotify('result', input);
  await __dockCallLifecycle('attached');
  return __dockSnapshot();
}

async function __dockDispatchEvent(methodName, eventJson) {
  if (__dockExpired) {
    throw new Error('component is expired');
  }
  const methods = __dockSafeObject(__dockDefinition.methods);
  const method = methods[methodName];
  if (typeof method !== 'function') {
    throw new Error('component method not found: ' + methodName);
  }
  __dockTrace.push({ kind: 'event', name: methodName });
  await method.call(__dockInstance, JSON.parse(eventJson));
  return __dockSnapshot();
}

async function __dockExpire(payloadJson) {
  if (!__dockExpired) {
    await __dockNotify('expire', JSON.parse(payloadJson));
    await __dockCallLifecycle('detached');
    __dockExpired = true;
  }
  return __dockSnapshot();
}

const wx = Object.freeze({
  modelContext: Object.freeze({
    NotificationType: Object.freeze({
      Input: 'input',
      Result: 'result',
      Expire: 'expire',
      Overflow: 'overflow'
    }),
    getContext() {
      return __dockInstance ? __dockInstance.__dockModelContext : __dockCreateModelContext(Object.create(null));
    },
    getViewContext() {
      return __dockInstance ? __dockInstance.__dockViewContext : __dockCreateViewContext(Object.create(null));
    },
    expireAllCards(options) {
      const payload = __dockSafeObject(options);
      __dockPushAction({
        type: 'expireAllCards',
        componentPaths: Array.isArray(payload.componentPaths) ? __dockClone(payload.componentPaths) : [],
        match: typeof payload.match === 'string' ? payload.match : null
      });
      return { errMsg: 'expireAllCards:ok' };
    }
  }),
  getDeviceInfo() {
    return Object.freeze({ platform: 'anp-miniapp-dock', model: 'component-runtime', language: 'en' });
  },
  getAppBaseInfo() {
    return Object.freeze({ SDKVersion: '0.1.0', version: '0.1.0' });
  }
});

const console = Object.freeze({
  log() {},
  warn() {},
  error() {}
});

Object.defineProperty(__dockFunctionConstructor.prototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });

Object.defineProperty(globalThis, 'Component', { value: Component, configurable: false, writable: false });
Object.defineProperty(globalThis, 'wx', { value: wx, configurable: false, writable: false });
Object.defineProperty(globalThis, 'console', { value: console, configurable: false, writable: false });
Object.defineProperty(globalThis, 'eval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'Function', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'process', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'fetch', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'WebSocket', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'setTimeout', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'setInterval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'clearTimeout', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'clearInterval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'require', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dockMount', { value: __dockMount, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dockDispatchEvent', { value: __dockDispatchEvent, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dockExpire', { value: __dockExpire, configurable: false, writable: false });
})();
"#;

#[derive(Debug, Clone)]
pub struct ComponentVmConfig {
    pub timeout: Duration,
    pub memory_limit_bytes: usize,
    pub max_stack_size_bytes: usize,
}

impl Default for ComponentVmConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            memory_limit_bytes: 16 * 1024 * 1024,
            max_stack_size_bytes: 512 * 1024,
        }
    }
}

#[derive(Debug)]
pub enum ComponentVmError {
    MissingComponentJs,
    QuickJs(String),
    InvalidJson(String),
    Compile(ComponentCompileError),
    Expired,
    Timeout(Duration),
}

impl std::fmt::Display for ComponentVmError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingComponentJs => {
                formatter.write_str("component package is missing index.js")
            }
            Self::QuickJs(error) => write!(formatter, "component VM failed: {error}"),
            Self::InvalidJson(error) => {
                write!(formatter, "component VM returned invalid JSON: {error}")
            }
            Self::Compile(error) => write!(formatter, "component render compile failed: {error}"),
            Self::Expired => formatter.write_str("component is expired"),
            Self::Timeout(timeout) => write!(formatter, "component VM timed out after {timeout:?}"),
        }
    }
}

impl std::error::Error for ComponentVmError {}

impl From<ComponentCompileError> for ComponentVmError {
    fn from(error: ComponentCompileError) -> Self {
        Self::Compile(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentTraceKind {
    Lifecycle,
    Notification,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentTraceEntry {
    pub kind: ComponentTraceKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ComponentVmAction {
    #[serde(rename = "sendFollowUpMessage")]
    SendFollowUpMessage {
        #[serde(default)]
        content: Vec<Value>,
    },
    #[serde(rename = "api/call")]
    ApiCall {
        name: String,
        #[serde(default = "empty_object")]
        arguments: Value,
    },
    #[serde(rename = "expirePreviousCards")]
    ExpirePreviousCards {
        #[serde(default, rename = "componentPaths")]
        component_paths: Vec<String>,
        #[serde(default, rename = "match")]
        match_policy: Option<String>,
    },
    #[serde(rename = "expireAllCards")]
    ExpireAllCards {
        #[serde(default, rename = "componentPaths")]
        component_paths: Vec<String>,
        #[serde(default, rename = "match")]
        match_policy: Option<String>,
    },
    #[serde(rename = "openDetailPage")]
    OpenDetailPage { url: String },
    #[serde(rename = "setRelatedPage")]
    SetRelatedPage {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        query: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInput {
    pub api_name: String,
    #[serde(default = "empty_object")]
    pub arguments: Value,
    #[serde(default)]
    pub properties: Map<String, Value>,
    #[serde(default)]
    pub content: Vec<Value>,
    #[serde(default)]
    pub structured_content: Option<Map<String, Value>>,
    #[serde(default, rename = "_meta")]
    pub meta: Option<Map<String, Value>>,
}

impl ComponentInput {
    pub fn new(api_name: impl Into<String>) -> Self {
        Self {
            api_name: api_name.into(),
            arguments: Value::Object(Map::new()),
            properties: Map::new(),
            content: Vec::new(),
            structured_content: None,
            meta: None,
        }
    }

    fn seed_data(&self) -> Value {
        let mut seed = self.structured_content.clone().unwrap_or_default();
        seed.insert("apiName".to_owned(), Value::String(self.api_name.clone()));
        seed.insert("arguments".to_owned(), self.arguments.clone());
        seed.insert("content".to_owned(), Value::Array(self.content.clone()));
        if let Some(meta) = &self.meta {
            seed.insert("_meta".to_owned(), Value::Object(meta.clone()));
        }
        Value::Object(seed)
    }

    fn notification_payload(&self) -> Value {
        json!({
            "apiName": self.api_name,
            "arguments": self.arguments,
            "result": {
                "content": self.content,
                "structuredContent": self.structured_content,
                "_meta": self.meta
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentOperationOutcome {
    pub render: ComponentRenderOutput,
    pub actions: Vec<ComponentVmAction>,
    pub trace: Vec<ComponentTraceEntry>,
    pub state: Value,
}

#[derive(Debug, Clone, Copy)]
struct OperationDeadline {
    start: Instant,
    timeout: Duration,
}

pub struct ComponentInstance {
    package: ComponentPackage,
    config: ComponentVmConfig,
    runtime: Runtime,
    context: Context,
    deadline: Rc<RefCell<Option<OperationDeadline>>>,
    state: Value,
    trace: Vec<ComponentTraceEntry>,
    expired: bool,
    mounted: bool,
}

impl ComponentInstance {
    pub fn new(package: ComponentPackage) -> Result<Self, ComponentVmError> {
        Self::with_config(package, ComponentVmConfig::default())
    }

    pub fn with_config(
        package: ComponentPackage,
        config: ComponentVmConfig,
    ) -> Result<Self, ComponentVmError> {
        let source = package
            .js
            .as_deref()
            .ok_or(ComponentVmError::MissingComponentJs)?;
        let runtime = Runtime::new().map_err(to_quickjs_error)?;
        runtime.set_memory_limit(config.memory_limit_bytes);
        runtime.set_max_stack_size(config.max_stack_size_bytes);
        let deadline = Rc::new(RefCell::new(None::<OperationDeadline>));
        let interrupt_deadline = Rc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            interrupt_deadline
                .borrow()
                .as_ref()
                .map(|deadline| deadline.start.elapsed() >= deadline.timeout)
                .unwrap_or(false)
        })));

        let context = Context::builder()
            .with::<rquickjs::context::intrinsic::Eval>()
            .with::<rquickjs::context::intrinsic::Promise>()
            .with::<rquickjs::context::intrinsic::Json>()
            .build(&runtime)
            .map_err(to_quickjs_error)?;

        context.with(|ctx| {
            ctx.eval::<(), _>(COMPONENT_BOOTSTRAP)
                .catch(&ctx)
                .map_err(caught_error)?;
            ctx.eval::<(), _>(source).catch(&ctx).map_err(caught_error)
        })?;

        Ok(Self {
            package,
            config,
            runtime,
            context,
            deadline,
            state: Value::Object(Map::new()),
            trace: Vec::new(),
            expired: false,
            mounted: false,
        })
    }

    pub fn mount(
        &mut self,
        input: ComponentInput,
    ) -> Result<ComponentOperationOutcome, ComponentVmError> {
        let seed_json = serde_json::to_string(&input.seed_data())
            .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))?;
        let input_json = serde_json::to_string(&input.notification_payload())
            .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))?;
        let properties_json = serde_json::to_string(&input.properties)
            .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))?;
        let snapshot =
            self.call_snapshot("__dockMount", (seed_json, properties_json, input_json))?;
        self.mounted = true;
        self.apply_snapshot(snapshot)
    }

    pub fn dispatch_event(
        &mut self,
        event: &ComponentEvent,
    ) -> Result<ComponentOperationOutcome, ComponentVmError> {
        if self.expired {
            return Err(ComponentVmError::Expired);
        }
        let event_json = serde_json::to_string(&event.to_js_event())
            .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))?;
        let snapshot =
            self.call_snapshot("__dockDispatchEvent", (event.method.clone(), event_json))?;
        self.apply_snapshot(snapshot)
    }

    pub fn expire(
        &mut self,
        payload: Value,
    ) -> Result<ComponentOperationOutcome, ComponentVmError> {
        let payload_json = serde_json::to_string(&payload)
            .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))?;
        let snapshot = self.call_snapshot("__dockExpire", (payload_json,))?;
        self.expired = true;
        self.apply_snapshot(snapshot)
    }

    pub fn render(&self) -> Result<ComponentRenderOutput, ComponentVmError> {
        compile_component_to_render_ir(&self.package, &self.state).map_err(Into::into)
    }

    pub fn state(&self) -> &Value {
        &self.state
    }

    pub fn trace(&self) -> &[ComponentTraceEntry] {
        &self.trace
    }

    pub fn is_expired(&self) -> bool {
        self.expired
    }

    pub fn is_mounted(&self) -> bool {
        self.mounted
    }

    fn call_snapshot<A>(
        &mut self,
        function_name: &str,
        args: A,
    ) -> Result<JsSnapshot, ComponentVmError>
    where
        A: for<'js> IntoArgs<'js>,
    {
        *self.deadline.borrow_mut() = Some(OperationDeadline {
            start: Instant::now(),
            timeout: self.config.timeout,
        });
        let result = self
            .context
            .with(|ctx| call_js_snapshot(ctx, function_name, args, self.config.timeout));
        *self.deadline.borrow_mut() = None;
        result
    }

    fn apply_snapshot(
        &mut self,
        snapshot: JsSnapshot,
    ) -> Result<ComponentOperationOutcome, ComponentVmError> {
        self.state = snapshot.data;
        self.trace.extend(snapshot.trace.clone());
        let render = self.render()?;
        Ok(ComponentOperationOutcome {
            render,
            actions: snapshot.actions,
            trace: snapshot.trace,
            state: self.state.clone(),
        })
    }
}

impl Drop for ComponentInstance {
    fn drop(&mut self) {
        self.runtime.set_interrupt_handler(None);
    }
}

#[derive(Debug, Deserialize)]
struct JsSnapshot {
    #[serde(default = "empty_object")]
    data: Value,
    #[serde(default)]
    actions: Vec<ComponentVmAction>,
    #[serde(default)]
    trace: Vec<ComponentTraceEntry>,
}

fn call_js_snapshot<'js, A>(
    ctx: Ctx<'js>,
    function_name: &str,
    args: A,
    timeout: Duration,
) -> Result<JsSnapshot, ComponentVmError>
where
    A: IntoArgs<'js>,
{
    let function: Function = ctx.globals().get(function_name).map_err(to_quickjs_error)?;
    let promise: MaybePromise = function
        .call(args)
        .catch(&ctx)
        .map_err(|error| map_caught_or_timeout(error, timeout))?;
    let snapshot_json = promise
        .finish::<String>()
        .catch(&ctx)
        .map_err(|error| map_caught_or_timeout(error, timeout))?;
    serde_json::from_str(&snapshot_json)
        .map_err(|error| ComponentVmError::InvalidJson(error.to_string()))
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn to_quickjs_error(error: rquickjs::Error) -> ComponentVmError {
    ComponentVmError::QuickJs(error.to_string())
}

fn caught_error(error: CaughtError<'_>) -> ComponentVmError {
    match error {
        CaughtError::Exception(exception) => {
            ComponentVmError::QuickJs(exception.message().unwrap_or_else(|| exception.to_string()))
        }
        CaughtError::Value(value) => ComponentVmError::QuickJs(format!("{value:?}")),
        CaughtError::Error(error) => to_quickjs_error(error),
    }
}

fn map_caught_or_timeout(error: CaughtError<'_>, timeout: Duration) -> ComponentVmError {
    if caught_message(&error).as_deref() == Some("interrupted") {
        ComponentVmError::Timeout(timeout)
    } else {
        caught_error(error)
    }
}

fn caught_message(error: &CaughtError<'_>) -> Option<String> {
    match error {
        CaughtError::Exception(exception) => exception.message(),
        CaughtError::Value(value) => Some(format!("{value:?}")),
        CaughtError::Error(error) => Some(error.to_string()),
    }
}
