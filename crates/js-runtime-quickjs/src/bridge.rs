use crate::middleware::MIDDLEWARE_BOOTSTRAP;

pub const BRIDGE_BOOTSTRAP: &str = r#"
(() => {
'use strict';

function __dockSafeJson(value) {
  if (typeof value === 'undefined') {
    return 'null';
  }
  return JSON.stringify(value);
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

function __dockCreateSkill(skillPath) {
  return {
    skillPath,
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

const wx = Object.freeze({
  modelContext: Object.freeze({
    createSkill: __dockCreateSkill
  })
});

Object.defineProperty(__dockModuleFactory.prototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });
Object.defineProperty(__dockAsyncGeneratorFunctionPrototype, 'constructor', { value: undefined, configurable: false, writable: false });

Object.defineProperty(globalThis, 'wx', { value: wx, configurable: false, writable: false });
Object.defineProperty(globalThis, 'console', { value: console, configurable: false, writable: false });
Object.defineProperty(globalThis, '__dirname', { value: '', configurable: false, writable: false });
Object.defineProperty(globalThis, '__filename', { value: 'index', configurable: false, writable: false });
Object.defineProperty(globalThis, 'require', { value: (specifier) => __dockRequire('index', specifier), configurable: false, writable: false });
Object.defineProperty(globalThis, 'eval', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'Function', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'process', { value: undefined, configurable: false, writable: false });
Object.defineProperty(globalThis, 'fetch', { value: undefined, configurable: false, writable: false });

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
    format!("{MIDDLEWARE_BOOTSTRAP}\n{BRIDGE_BOOTSTRAP}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_disables_node_and_network_globals() {
        let bootstrap = runtime_bootstrap();
        assert!(bootstrap.contains("globalThis, 'process'"));
        assert!(bootstrap.contains("globalThis, 'fetch'"));
        assert!(bootstrap.contains("globalThis, 'eval'"));
        assert!(bootstrap.contains("globalThis, 'Function'"));
    }
}
