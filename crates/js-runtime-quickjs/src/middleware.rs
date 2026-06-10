pub const MIDDLEWARE_BOOTSTRAP: &str = r#"
function __dockRunMiddlewareChain(middlewares, handler, context) {
  let index = -1;

  function dispatch(position) {
    if (position <= index) {
      throw new Error('middleware next() called multiple times');
    }

    index = position;
    const middleware = position === middlewares.length ? handler : middlewares[position];
    if (!middleware) {
      return Promise.resolve();
    }

    let called = false;
    let nextResult;
    const next = () => {
      if (called) {
        throw new Error('middleware next() called multiple times');
      }
      called = true;
      nextResult = dispatch(position + 1);
      return nextResult;
    };

    if (position === middlewares.length) {
      return Promise.resolve(middleware(context));
    }

    return Promise.resolve(middleware(context, next)).then(async (value) => {
      if (typeof value !== 'undefined') {
        return value;
      }
      return called ? await nextResult : undefined;
    });
  }

  return dispatch(0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_contains_double_next_guard() {
        assert!(MIDDLEWARE_BOOTSTRAP.contains("next() called multiple times"));
    }
}
