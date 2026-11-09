# Dynamic Status Compatibility Fixture

This mock-only fixture demonstrates `scope.dynamic`, component-side `wx.request`, limited timers, `expire` cleanup, and the RequestBroker boundary. It does not expose auth headers, bearer values, credentials, user data, or production network transport.

## Run

```bash
cargo run -p dock-cli -- validate examples/fixtures/dynamic-status
cargo run -p dock-cli -- inspect examples/fixtures/dynamic-status
cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
```

## Expected Evidence

- Expected report summary: `expected-test-skill.json`
- Render IR snapshot: `testdata/render-ir/dynamic-status.refreshDynamicStatus.json`
- Fixture set: `dynamic-status`
- Component path: `components/dynamic-status/index`
- Primary API: `refreshDynamicStatus`

## Risk Boundary

`refreshDynamicStatus` is an L2 fixture. The dynamic component may call the injected headless RequestBroker during `test-skill`, but the broker is dev-only and `productionReady` remains `false`. Authorization-like response headers are redacted before report output.
