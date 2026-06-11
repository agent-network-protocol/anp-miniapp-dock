# anp-miniapp-dock

`anp-miniapp-dock` is a DID-native Rust Skill runtime for running MiniApp MCP-compatible agent skills over ANP.

The MVP is now implemented as a Cargo workspace. It can load a MiniApp MCP-style Skill, validate `mcp.json`, run atomic API JavaScript in an isolated QuickJS-backed VM, compile and execute a MiniApp MCP component runtime subset, route high-risk actions through consent/audit, and run a local coffee ordering demo through `dock-cli` and `demo-server`.

## Architecture Documents

- [Agentic MiniApp Container MVP PRD](docs/architecture/agentic-miniapp-container-prd.md)
- [anp-miniapp-dock System Architecture](docs/architecture/anp-skill-dock-architecture.md)
- [MiniApp MCP Compatibility MVP](docs/architecture/miniapp-mcp-compatibility-mvp.md)
- [MiniApp MCP Component Runtime](docs/architecture/miniapp-mcp-component-runtime.md)
- [MiniApp MCP protocol notes](docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt)
- [Local demo runbook](docs/runbook/local-demo.md)
- [Security runbook](docs/runbook/security.md)

## Workspace Layout

- `crates/mcp-schema`: MiniApp MCP manifest/result models and validation.
- `crates/skill-loader`: `SKILL.md`, `mcp.json`, API module, and component package loading.
- `crates/dock-core`: Orchestrator, API registry, permission, consent, audit, and render routing boundaries.
- `crates/js-runtime-quickjs`: QuickJS-backed atomic API VM using `rquickjs`.
- `crates/wx-compat`: P0 `wx` capability profiles, scoped storage, request broker traits, and model context helpers.
- `crates/anp-adapter`: ANP DID-aware signed HTTP, challenge proof contracts, allowlist, and scoped capability token cache.
- `crates/consent-audit`: risk policy, mock consent provider, proof, audit records, and redaction.
- `crates/card-spec`: structured fallback card schema.
- `crates/component-runtime`: Component VM, WXML/WXSS subset compiler, events, and Render IR.
- `crates/demo-server`: coffee merchant Agent demo server.
- `crates/dock-cli`: developer CLI and coffee E2E harness.
- `examples/coffee-skill`: mock MiniApp MCP coffee Skill fixture.

## Development Commands

The repository pins Rust `1.88.0` through `rust-toolchain.toml` to match the ANP Rust SDK path dependency.

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Focused commands:

```bash
cargo test -p dock-cli --test coffee_order_flow
cargo test -p demo-server
cargo test -p component-runtime component_vm
```

## CLI

`dock-cli` prints JSON so outputs can be used as validation evidence or piped into other tools.

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- call-api examples/coffee-skill searchDrinks '{}'
cargo run -p dock-cli -- preview-component examples/coffee-skill components/drink-list/index '{"apiName":"searchDrinks","structuredContent":{"drinks":[{"id":"latte","name":"Latte","price":18}]}}'
cargo run -p dock-cli -- preview-card '{"content":[{"type":"text","text":"paid"}],"structuredContent":{"orderId":"order_demo_001","status":"paid"}}'
```

To run the coffee flow against the local demo server:

```bash
cargo run -p demo-server -- \
  --host 127.0.0.1 \
  --port 3000 \
  --skill examples/coffee-skill \
  --token-issuer-secret test-only-local-secret \
  --trusted-did-document did:wba:miniapp-test.example=/path/to/identity/did_document.json

cargo run -p dock-cli -- run-demo \
  --skill examples/coffee-skill \
  --server http://127.0.0.1:3000 \
  --identity-handle miniapp-test.awiki.ai \
  --identity-root /path/to/identity-store/identities
```

`run-demo` performs ANP DID challenge/login, exercises demo-server coffee business APIs, runs the local Skill API VM through `dock-core`, triggers component `api/call` actions, mock-approves high-risk consent, renders Component VM output to Render IR JSON, and verifies card expiration. Capability tokens are used internally and redacted from CLI output. The command above references a local test identity by path; do not copy or commit private keys into this repository. The DID passed to `--trusted-did-document` must match the `did` field in that identity's `identity.json`.

## MVP Boundary

The MVP is contract-compatible with the MiniApp MCP Skill shape, not a full WeChat Mini Program runtime.

P0 implemented:

- `SKILL.md`, `mcp.json`, `apis[]`, `components[]`, `_meta.ui.componentPath`.
- Atomic API JS loading with restricted CommonJS, `wx.modelContext.createSkill`, `registerAPI`, middleware, input validation, timeout, and sandboxed globals.
- Runtime boundaries for permission, consent, audit, render routing, and model-visible result filtering.
- ANP DID-aware adapter contracts, signed request helper, `anp-http-signature/v1` challenge proof, allowlist, and scoped capability token cache.
- Component runtime subset: `Component({})`, `data`, `properties`, `methods`, `created/attached/detached`, `setData`, `NotificationType.Input/Result/Expire`, `sendFollowUpMessage`, `api/call`, `expirePreviousCards`, tap/image events, WXML/WXSS subset, Render IR JSON.
- CardSpec fallback for structured results or render failures.
- Coffee merchant demo server and CLI/E2E flow.

P0.5 auth/token now uses real ANP DID challenge signing and scoped capability tokens for the demo server flow. The runtime still intentionally does not implement a real Flutter host, complete WXML/WXSS, full component/page routing, WeChat login, real payment provider, cloud development, social APIs, consent UI, or host renderer.

## Security Notes

Do not commit private keys, DID credentials, capability tokens, merchant secrets, or real user data. The coffee Skill and demo server use mock-only business data, but challenge/login and capability tokens are no longer mock. Runtime code should keep DID signing, tokens, and high-risk authorization below the Skill/CLI boundary, and user-facing output should redact tokens and signatures.
