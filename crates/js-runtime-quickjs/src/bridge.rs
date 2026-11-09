use crate::middleware::MIDDLEWARE_BOOTSTRAP;
use wx_compat::{notification_type_js_literal, unsupported_api_registry_js_literal};

pub const BRIDGE_BOOTSTRAP_TEMPLATE: &str = r#"
(() => {
'use strict';

function __dockSafeJson(value) {
  if (typeof value === 'undefined') {
    return 'null';
  }
  return JSON.stringify(value);
}

function __dockJsonOptionsOrFailure(apiName, value) {
  try {
    return { ok: true, json: __dockSafeJson(value) };
  } catch (_error) {
    return {
      ok: false,
      json: JSON.stringify({
        errMsg: apiName + ':fail invalid_options',
        code: 'invalid_options',
        reason: 'options must be JSON-safe',
        suggestion: 'Pass plain JSON values without functions, symbols, BigInt, or cycles.'
      })
    };
  }
}

function __dockLog(level, args) {
  __dock.log(level, args.map((value) => {
    if (typeof value === 'string') {
      return value;
    }
    try {
      return JSON.stringify(value);
    } catch (_err) {
      return String(value);
    }
  }));
}

const console = Object.freeze({
  log: (...args) => __dockLog('log', args),
  warn: (...args) => __dockLog('warn', args),
  error: (...args) => __dockLog('error', args)
});

const __dockModules = JSON.parse(__dock.modulesJson());
const __dockCache = Object.create(null);
const __dockRegisteredApis = Object.create(null);
const __dockMiddlewares = [];
const __dockModuleFactory = Function;
const __dockAsyncFunctionPrototype = Object.getPrototypeOf(async function() {});
const __dockGeneratorFunctionPrototype = Object.getPrototypeOf(function* () {});
const __dockAsyncGeneratorFunctionPrototype = Object.getPrototypeOf(async function* () {});

function __dockNormalizeRequire(parentId, specifier) {
  if (typeof specifier !== 'string' || specifier.length === 0) {
    throw new Error('require specifier must be a non-empty string');
  }
  if (specifier.includes('\0') || specifier.includes('://') || specifier.startsWith('/') || specifier.startsWith('\\')) {
    throw new Error('require path outside skill package: ' + specifier);
  }

  const parentParts = parentId.split('/');
  parentParts.pop();
  const base = specifier.startsWith('.') ? parentParts : [];
  const parts = base.slice();

  for (const rawPart of specifier.split('/')) {
    if (!rawPart || rawPart === '.') {
      continue;
    }
    if (rawPart === '..') {
      if (parts.length === 0) {
        throw new Error('require path outside skill package: ' + specifier);
      }
      parts.pop();
      continue;
    }
    parts.push(rawPart);
  }

  let id = parts.join('/');
  if (id.endsWith('.js')) {
    id = id.slice(0, -3);
  }
  if (!Object.prototype.hasOwnProperty.call(__dockModules, id)) {
    throw new Error('module not found: ' + specifier);
  }
  return id;
}

function __dockRequire(parentId, specifier) {
  const id = __dockNormalizeRequire(parentId, specifier);
  if (Object.prototype.hasOwnProperty.call(__dockCache, id)) {
    return __dockCache[id].exports;
  }

  const moduleDef = __dockModules[id];
  const module = { id, filename: moduleDef.filename, exports: {} };
  __dockCache[id] = module;
  const require = (childSpecifier) => __dockRequire(id, childSpecifier);
  const fn = __dockModuleFactory('exports', 'require', 'module', '__filename', '__dirname', moduleDef.source);
  fn(module.exports, require, module, moduleDef.filename, moduleDef.dirname);
  return module.exports;
}

function __dockNormalizeSkillPath(skillPath) {
  if (typeof skillPath !== 'string') {
    throw new Error('createSkill skillPath must be a string');
  }
  if (skillPath.includes('\0') || skillPath.includes('://') || skillPath.startsWith('/') || skillPath.startsWith('\\') || skillPath.includes('\\')) {
    throw new Error('createSkill path outside skill package: ' + skillPath);
  }

  const parts = [];
  for (const rawPart of skillPath.split('/')) {
    if (!rawPart || rawPart === '.') {
      continue;
    }
    if (rawPart === '..') {
      if (parts.length === 0) {
        throw new Error('createSkill path outside skill package: ' + skillPath);
      }
      parts.pop();
      continue;
    }
    parts.push(rawPart);
  }
  return parts.join('/');
}

function __dockCreateSkill(skillPath) {
  const normalizedSkillPath = __dockNormalizeSkillPath(skillPath);
  return {
    skillPath: normalizedSkillPath,
    registerAPI(name, handler) {
      if (typeof name !== 'string' || name.length === 0) {
        throw new Error('registerAPI name must be a non-empty string');
      }
      if (typeof handler !== 'function') {
        throw new Error('registerAPI handler for ' + name + ' must be a function');
      }
      if (Object.prototype.hasOwnProperty.call(__dockRegisteredApis, name)) {
        throw new Error('duplicate API registration: ' + name);
      }
      __dockRegisteredApis[name] = handler;
    },
    use(middleware) {
      if (typeof middleware !== 'function') {
        throw new Error('middleware must be a function');
      }
      __dockMiddlewares.push(middleware);
    }
  };
}

function __dockCallbackOptions(options) {
  return options && typeof options === 'object' ? options : {};
}

function __dockInvokeCallback(callback, payload) {
  try {
    callback(payload);
  } catch (error) {
    console.warn('wx callback error redacted');
  }
}

function __dockAsyncOutcome(apiName, options, hostCall) {
  const callbacks = __dockCallbackOptions(options);
  return Promise.resolve().then(() => {
    const payload = JSON.parse(hostCall());
    const ok = typeof payload.errMsg !== 'string' || payload.errMsg.indexOf(':fail') === -1;
    if (ok && typeof callbacks.success === 'function') {
      __dockInvokeCallback(callbacks.success, payload);
    }
    if (!ok && typeof callbacks.fail === 'function') {
      __dockInvokeCallback(callbacks.fail, payload);
    }
    if (typeof callbacks.complete === 'function') {
      __dockInvokeCallback(callbacks.complete, payload);
    }
    if (!ok) {
      throw payload;
    }
    return payload;
  });
}

function __dockUnsupportedAsync(apiName, options) {
  return __dockAsyncOutcome(apiName, options, () => __dock.unsupportedApi(apiName));
}

function __dockUnsupportedSync(apiName) {
  const payload = JSON.parse(__dock.unsupportedApi(apiName));
  const error = new Error(payload.errMsg);
  error.errMsg = payload.errMsg;
  error.code = payload.code;
  error.reason = payload.reason;
  error.suggestion = payload.suggestion;
  throw error;
}

function __dockStoragePayloadError(payload) {
  const error = new Error(payload.errMsg);
  error.errMsg = payload.errMsg;
  error.code = payload.code;
  error.reason = payload.reason;
  error.suggestion = payload.suggestion;
  return error;
}

function __dockStorageFailureJson(apiName, reason, suggestion) {
  return JSON.stringify({
    errMsg: apiName + ':fail invalid_options',
    code: 'invalid_options',
    reason,
    suggestion
  });
}

function __dockJsonSafetyFailure(value, seen) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') {
    return null;
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? null : 'storage data must contain only finite numbers';
  }
  if (typeof value === 'undefined') {
    return 'storage data must not contain undefined';
  }
  if (typeof value === 'function') {
    return 'storage data must not contain functions';
  }
  if (typeof value === 'symbol') {
    return 'storage data must not contain symbols';
  }
  if (typeof value === 'bigint') {
    return 'storage data must not contain BigInt values';
  }
  if (typeof value !== 'object') {
    return 'storage data must be JSON-safe';
  }
  if (seen.indexOf(value) !== -1) {
    return 'storage data must not contain cyclic references';
  }
  seen.push(value);
  const keys = Array.isArray(value) ? value.keys() : Object.keys(value);
  for (const key of keys) {
    const child = Array.isArray(value) ? value[key] : value[key];
    const failure = __dockJsonSafetyFailure(child, seen);
    if (failure) {
      return failure;
    }
  }
  seen.pop();
  return null;
}

function __dockStorageOptionsJson(apiName, options, requireData) {
  const source = __dockCallbackOptions(options);
  const payload = {};
  if (Object.prototype.hasOwnProperty.call(source, 'key')) {
    payload.key = source.key;
  }
  if (requireData) {
    if (!Object.prototype.hasOwnProperty.call(source, 'data')) {
      return {
        ok: false,
        json: __dockStorageFailureJson(apiName, 'storage data must be provided', 'Pass options.data for setStorage.')
      };
    }
    const dataFailure = __dockJsonSafetyFailure(source.data, []);
    if (dataFailure) {
      return {
        ok: false,
        json: __dockStorageFailureJson(apiName, dataFailure, 'Store only plain JSON-safe values.')
      };
    }
    payload.data = source.data;
  }
  return __dockJsonOptionsOrFailure(apiName, payload);
}

function __dockAsyncStorageOutcome(apiName, options, requireData, hostCall) {
  return __dockAsyncOutcome(apiName, options, () => {
    const optionsJson = __dockStorageOptionsJson(apiName, options || {}, requireData);
    if (!optionsJson.ok) {
      return optionsJson.json;
    }
    return hostCall(optionsJson.json);
  });
}

function __dockSyncStorageOutcome(apiName, hostCall) {
  const payload = JSON.parse(hostCall());
  if (typeof payload.errMsg === 'string' && payload.errMsg.indexOf(':fail') !== -1) {
    payload.errMsg = apiName + payload.errMsg.slice(payload.errMsg.indexOf(':'));
    throw __dockStoragePayloadError(payload);
  }
  return payload;
}

function __dockHighRiskApi(apiName, options) {
  return __dockAsyncOutcome(apiName, options, () => {
    const optionsJson = __dockJsonOptionsOrFailure(apiName, options || {});
    if (!optionsJson.ok) {
      return optionsJson.json;
    }
    return __dock.highRiskApi(apiName, optionsJson.json);
  });
}

function __dockInstallUnsupportedWxApi(apiDef) {
  const parts = apiDef.name.split('.');
  let index = parts[0] === 'wx' ? 1 : 0;
  let target = wx;
  while (index < parts.length - 1) {
    const part = parts[index];
    if (!Object.prototype.hasOwnProperty.call(target, part)) {
      target[part] = {};
    }
    target = target[part];
    index += 1;
  }
  const leaf = parts[index];
  if (Object.prototype.hasOwnProperty.call(target, leaf)) {
    return;
  }
  if (apiDef.kind === 'sync') {
    target[leaf] = () => __dockUnsupportedSync(apiDef.name);
  } else {
    target[leaf] = (options) => __dockUnsupportedAsync(apiDef.name, options);
  }
}

function __dockFreezeObjectTree(value, seen) {
  if (!value || typeof value !== 'object' || seen.indexOf(value) !== -1) {
    return value;
  }
  seen.push(value);
  for (const key of Object.keys(value)) {
    __dockFreezeObjectTree(value[key], seen);
  }
  return Object.freeze(value);
}

const wx = {
  login(options) {
    return __dockAsyncOutcome('login', options, () => __dock.login());
  },
  checkSession(options) {
    return __dockAsyncOutcome('checkSession', options, () => __dock.checkSession());
  },
  request(options) {
    return __dockAsyncOutcome('request', options, () => __dock.request(__dockSafeJson(options || {})));
  },
  getStorage(options) {
    return __dockAsyncStorageOutcome('getStorage', options, false, (optionsJson) => __dock.getStorage(optionsJson));
  },
  setStorage(options) {
    return __dockAsyncStorageOutcome('setStorage', options, true, (optionsJson) => __dock.setStorage(optionsJson));
  },
  removeStorage(options) {
    return __dockAsyncStorageOutcome('removeStorage', options, false, (optionsJson) => __dock.removeStorage(optionsJson));
  },
  clearStorage(options) {
    return __dockAsyncOutcome('clearStorage', options, () => __dock.clearStorage());
  },
  getStorageSync(key) {
    const optionsJson = __dockStorageOptionsJson('getStorageSync', { key }, false);
    if (!optionsJson.ok) {
      throw __dockStoragePayloadError(JSON.parse(optionsJson.json));
    }
    const payload = __dockSyncStorageOutcome('getStorageSync', () => __dock.getStorage(optionsJson.json));
    return payload.data;
  },
  setStorageSync(key, data) {
    const optionsJson = __dockStorageOptionsJson('setStorageSync', { key, data }, true);
    if (!optionsJson.ok) {
      throw __dockStoragePayloadError(JSON.parse(optionsJson.json));
    }
    __dockSyncStorageOutcome('setStorageSync', () => __dock.setStorage(optionsJson.json));
  },
  removeStorageSync(key) {
    const optionsJson = __dockStorageOptionsJson('removeStorageSync', { key }, false);
    if (!optionsJson.ok) {
      throw __dockStoragePayloadError(JSON.parse(optionsJson.json));
    }
    __dockSyncStorageOutcome('removeStorageSync', () => __dock.removeStorage(optionsJson.json));
  },
  clearStorageSync() {
    __dockSyncStorageOutcome('clearStorageSync', () => __dock.clearStorage());
  },
  getDeviceInfo() {
    return Object.freeze(JSON.parse(__dock.getDeviceInfo()));
  },
  getAppBaseInfo() {
    return Object.freeze(JSON.parse(__dock.getAppBaseInfo()));
  },
  getPhoneNumber(options) {
    return __dockHighRiskApi('getPhoneNumber', options);
  },
  chooseAddress(options) {
    return __dockHighRiskApi('chooseAddress', options);
  },
  getLocation(options) {
    return __dockHighRiskApi('getLocation', options);
  },
  getFuzzyLocation(options) {
    return __dockHighRiskApi('getFuzzyLocation', options);
  },
  chooseLocation(options) {
    return __dockHighRiskApi('chooseLocation', options);
  },
  chooseMedia(options) {
    return __dockHighRiskApi('chooseMedia', options);
  },
  chooseMessageFile(options) {
    return __dockHighRiskApi('chooseMessageFile', options);
  },
  requestPayment(options) {
    return __dockHighRiskApi('requestPayment', options);
  },
  requestVirtualPayment(options) {
    return __dockHighRiskApi('requestVirtualPayment', options);
  },
  requestJointPayment(options) {
    return __dockHighRiskApi('requestJointPayment', options);
  },
  scanCode(options) {
    return __dockHighRiskApi('scanCode', options);
  },
  makePhoneCall(options) {
    return __dockHighRiskApi('makePhoneCall', options);
  },
  modelContext: Object.freeze({
    NotificationType: Object.freeze(__DOCK_NOTIFICATION_TYPE__),
    createSkill: __dockCreateSkill,
    getSessionId() {
      return __dock.modelContextGetSessionId();
    },
    expireAllCards(options) {
      return __dockAsyncOutcome('modelContext.expireAllCards', options, () => __dock.modelContextExpireAllCards(__dockSafeJson(options || {})));
    }
  })
};

const __dockUnsupportedWxApis = Object.freeze(__DOCK_UNSUPPORTED_WX_APIS__);
for (const apiDef of __dockUnsupportedWxApis) {
  __dockInstallUnsupportedWxApi(apiDef);
}
const __dockWxRoot = __dockFreezeObjectTree(wx, []);
const __dockWxProxy = new Proxy(__dockWxRoot, {
  get(target, property, receiver) {
    if (typeof property === 'symbol' || Reflect.has(target, property)) {
      return Reflect.get(target, property, receiver);
    }
    if (typeof property === 'string') {
      return (options) => __dockUnsupportedAsync(property, options);
    }
    return undefined;
  },
  set() {
    return false;
  },
  defineProperty() {
    return false;
  },
  deleteProperty() {
    return false;
  }
});

Object.defineProperty(__dockModuleFactory.prototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });

Object.defineProperty(globalThis, 'wx', { value: __dockWxProxy, configurable: false, writable: false });
Object.defineProperty(globalThis, 'console', { value: console, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dirname', { value: '', configurable: false, writable: false });
Object.defineProperty(globalThis, '__filename', { value: 'index', configurable: false, writable: false });
Object.defineProperty(globalThis, 'require', { value: (specifier) => __dockRequire('index', specifier), configurable: false, writable: false });
Object.defineProperty(globalThis, 'eval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'Function', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'Proxy', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'process', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'fetch', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'WebSocket', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'setTimeout', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'setInterval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'clearTimeout', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'clearInterval', { value: undefined, configurable: false, writable: false });

function __dockLoadEntry() {
  return __dockRequire('index', 'index');
}

function __dockRegisteredApiNames() {
  return Object.keys(__dockRegisteredApis);
}

async function __dockCallApi(name, contextJson) {
  const handler = __dockRegisteredApis[name];
  if (!handler) {
    throw new Error('API is not registered: ' + name);
  }
  const context = JSON.parse(contextJson);
  context.name = name;
  const result = await __dockRunMiddlewareChain(__dockMiddlewares, handler, context);
  return __dockSafeJson(result);
}

Object.defineProperty(globalThis, '__dockLoadEntry', { value: __dockLoadEntry, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dockRegisteredApiNames', { value: __dockRegisteredApiNames, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dockCallApi', { value: __dockCallApi, configurable: false, writable: false });
})();
"#;

pub fn runtime_bootstrap() -> String {
    let bridge = BRIDGE_BOOTSTRAP_TEMPLATE
        .replace("__DOCK_NOTIFICATION_TYPE__", notification_type_js_literal())
        .replace(
            "__DOCK_UNSUPPORTED_WX_APIS__",
            &unsupported_api_registry_js_literal(),
        );
    format!("{MIDDLEWARE_BOOTSTRAP}\n{bridge}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_disables_node_and_network_globals() {
        let bootstrap = runtime_bootstrap();
        assert!(bootstrap.contains("globalThis, 'process'"));
        assert!(bootstrap.contains("globalThis, 'fetch'"));
        assert!(bootstrap.contains("globalThis, 'WebSocket'"));
        assert!(bootstrap.contains("globalThis, 'setTimeout'"));
        assert!(bootstrap.contains("globalThis, 'setInterval'"));
        assert!(bootstrap.contains("globalThis, 'eval'"));
        assert!(bootstrap.contains("globalThis, 'Function'"));
        assert!(bootstrap.contains("globalThis, 'Proxy'"));
    }

    #[test]
    fn bridge_exposes_wx_login_and_request_host_boundary() {
        let bootstrap = runtime_bootstrap();
        assert!(bootstrap.contains("login(options)"));
        assert!(bootstrap.contains("request(options)"));
        assert!(bootstrap.contains("__dock.request"));
    }
}
