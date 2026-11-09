# Local Demo Runbook

This runbook describes how to run the Rust MVP locally with the mock coffee Skill.

## Prerequisites

- Rust toolchain `1.88.0`; the repository pins it with `rust-toolchain.toml`.
- Network access for the first Cargo dependency fetch.
- A non-production DID identity for demo-server challenge signing. The repository includes a test fixture under `examples/identity`; real DID credentials and private keys must stay local and ignored by Git.
- Optional Python `3.10+` for the FastAPI localhost coffee service.
- No real merchant secrets, capability tokens, OpenAI API keys, or user data are required. The coffee demo uses mock-only business data; demo-server challenge/login and capability token flows are exercised with local test credentials.

## Verify The Workspace

Run the normal gates from the repository root:

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Focused checks:

```bash
cargo test -p dock-cli --test coffee_order_flow
cargo test -p demo-server
cargo test -p js-runtime-quickjs
```

Run local environment diagnostics before debugging DID, Host provider, persistence, or remote server issues:

```bash
cargo run -p dock-cli -- doctor
```

The command emits `dock.doctor-report.v1` JSON. It checks Rust toolchain, workspace layout, runtime config contract, Skill package, DID identity, signing credential file permissions, trusted resolver, allowlist, storage/audit backend profile, Host providers, sandbox gate surface, and optional remote server health. By default it does not contact a server; pass `--server http://127.0.0.1:3000` or another demo URL to check `/health`. Use `--ci` when a failing check should return a non-zero exit code after the JSON report is printed.

Expected local defaults are not production-ready: unsigned/demo Skills, missing resolver/allowlist, in-memory storage/audit, missing production Host providers, skipped server health, and sandbox gates that doctor records but does not execute should be treated as warning/skip evidence. Real signing credential material, raw tokens, Authorization values, signatures, secrets, and absolute local paths must not appear in the report.

Run a CI-friendly performance/stress smoke baseline before release notes or canary decisions:

```bash
cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1
```

The command emits `dock.perf-baseline-report.v1` JSON. It records hardware-dependent local evidence for Skill load, API VM call, component render, Render IR size, token lookup, storage read/write, process RSS memory sample, concurrent sessions, multi-Skill, multi-component render, dynamic request/timer, and resource-limit fail-closed behavior. The checked-in sample artifact is `testdata/perf/coffee-smoke-baseline.json`. Treat these numbers as local baseline evidence only, not production SLOs.

For the full Skill developer workflow, see the developer docs:

- [Developer docs index](../developer/README.md)
- [Import WeChat MiniApp MCP Skill](../developer/import-wechat-mcp-skill.md)
- [wx API compatibility guide](../developer/wx-api-compatibility.md)
- [Component compatibility guide](../developer/component-compatibility.md)
- [Security guidelines](../developer/security-guidelines.md)
- [Host adapter guide](../developer/host-adapter-guide.md)

## Run Compatibility Fixtures

The repository includes four mock-only compatibility fixtures beyond coffee. Each fixture has its own `README.md`, `expected-test-skill.json`, and golden Render IR snapshot under `testdata/render-ir`.

```bash
cargo run -p dock-cli -- validate examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
cargo run -p dock-cli -- validate examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
cargo run -p dock-cli -- validate examples/fixtures/dynamic-status
cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
cargo run -p dock-cli -- validate examples/fixtures/location-map-preview
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
```

These fixtures cover form/address Host boundary, media/file handles, dynamic component request/timer behavior, and static location map preview. They are regression evidence only: all handles and URLs are mock values, headless providers are dev-only, and none of the fixture reports certify a production Host provider.

## Start The FastAPI Coffee Service

The current demo can use a Python/FastAPI localhost service to simulate the remote merchant HTTP server. The Skill package is still loaded from `examples/coffee-skill` on disk; only login and business calls go over HTTP.

```bash
cd examples/coffee-fastapi-server
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn app:app --host 127.0.0.1 --port 8008
```

The FastAPI service exposes:

- `GET /health`
- `GET /registry/agents`
- `GET /agents/coffee/manifest`
- `GET /agents/coffee/SKILL.md`
- `GET /agents/coffee/mcp.json`
- `POST /agents/coffee/auth/challenge`
- `POST /agents/coffee/auth/login`
- `POST /api/login` for the Skill-side `wx.login` + `wx.request` exchange
- `GET /api/drinks`
- `POST /api/order/confirm`
- `POST /api/order/pay`
- `GET /audit`

Then run the local container against that localhost service:

```bash
cargo run -p dock-cli -- run-demo --skill examples/coffee-skill --server http://127.0.0.1:8008
```

During `run-demo`, the Skill JavaScript calls `wx.login()`, then uses `wx.request()` to access `/api/login`, `/api/drinks`, `/api/order/confirm`, and `/api/order/pay` on localhost. With Host DID credentials configured, the Atomic API VM keeps the capability token inside `DidAuthSessionManager`, returns only a code-like redacted receipt to Skill JS, and `wx.checkSession()` can validate the cached session without exposing the token.

The DID challenge/login path is one-time by default: a login attempt consumes its challenge even when the signature is invalid, so the same `challengeId` cannot be retried with a later valid proof. The Rust demo server also checks bearer tokens against an in-memory lifecycle store for revoked jti values, while high-risk hosts can use the explicit one-time jti verification mode.

Step 04-05 adds a token cache persistence contract and restart restore policy in `anp-adapter`: restored token entries must still be unexpired, signature/trust valid, scope-matching, not revoked, and not replay/consumed-once entries. The included in-memory token persistence backend is dev/test only and reports `productionReady = false`; production deployments still need a Host secure store or encrypted token backend, cross-process replay/revocation store, DID resolver rotation policy, and secret-store integration.

Step 04-06 adds a scoped storage persistence contract in `wx-compat`: persisted storage scope includes user DID, merchant DID, Skill id, and namespace; restore rejects invalid or over-quota entries, cleanup supports remove/clear/delete scope, and reports/debug output redact raw keys and values. The included `LocalFileScopedStorageBackend` writes unencrypted JSON and reports `productionReady = false`; it is only for local/dev evidence. Production deployments still need a Host encrypted store or encrypted SQLite backend, migration/repair policy, access control, backup handling, and privacy deletion integration.

Step 04-07 adds a persistent audit sink contract in `consent-audit` and `dock-core`: audit profiles distinguish `inMemoryDev`, `localFileJsonl`, `hostPersistentSink`, and `encryptedSqlite`; export and retention report only redacted records plus profile/count/redaction metadata; `runtime.getAuditRecords` reports `audit_unavailable` for a corrupt persistent backend; and L3/L4 actions fail closed before executor execution when the audit sink cannot be opened. The included `FileAuditSink` writes unencrypted local JSONL and reports `productionReady = false`; it is only for local/dev evidence. Production deployments still need a Host persistent sink or encrypted SQLite backend, access control, export approval, retention configuration, durability/alerting, and privacy deletion integration.

## Start The Rust Demo Server

The Rust `demo-server` remains available as a test-compatible local merchant server. It exercises the newer ANP DID challenge proof and scoped capability token path, while still exposing the same localhost coffee business endpoints used by the Skill JavaScript.

Use port `3000` for a stable local URL:

```bash
cargo run -p demo-server -- \
  --host 127.0.0.1 \
  --port 3000 \
  --skill examples/coffee-skill \
  --token-issuer-secret test-only-local-secret \
  --trusted-did-document '<user-did>=examples/identity/did_document.json'
```

The `--trusted-did-document` value must use the same DID that the CLI signs with. The path points to the public DID document, not the private key. By default, `dock-cli run-demo` reads DID credentials from:

```text
examples/identity/did_document.json
examples/identity/key-1-private.pem
```

The CLI derives `userDid` from the DID document `id`. The checked-in files are test fixtures only; production DID credentials must not be committed.

`demo-server` trusts only DID documents registered with `--trusted-did-document`. Resolver mismatch, unknown DID, expired challenge, wrong audience/scope, expired token, revoked token, and replay-sensitive jti reuse fail closed with stable auth error codes and without printing raw token, proof, signature, or private key paths.

## Run CLI Commands

Validate the Skill:

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
```

Run local performance/stress smoke evidence:

```bash
cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1
```

Call an atomic API with local mock data only:

```bash
cargo run -p dock-cli -- call-api examples/coffee-skill searchDrinks '{}'
```

Call an atomic API through the localhost HTTP service:

```bash
cargo run -p dock-cli -- call-api examples/coffee-skill searchDrinks '{"query":"latte","serverUrl":"http://127.0.0.1:8008"}'
```

Preview a component:

```bash
cargo run -p dock-cli -- preview-component examples/coffee-skill components/drink-list/index '{"apiName":"searchDrinks","structuredContent":{"drinks":[{"id":"latte","name":"Latte","price":18}]}}'
```

Preview a CardSpec fallback:

```bash
cargo run -p dock-cli -- preview-card '{"content":[{"type":"text","text":"paid"}],"structuredContent":{"orderId":"order_demo_001","status":"paid"}}'
```

Run the headless Runtime JSON IPC surface:

```bash
cargo run -p dock-cli -- runtime-json examples/coffee-skill \
  '{"apiVersion":"dock.runtime.v1","requestId":"req-1","method":"runtime.negotiateVersion","params":{}}'
```

Call a Skill API through the same local envelope:

```bash
cargo run -p dock-cli -- runtime-json examples/coffee-skill \
  '{"apiVersion":"dock.runtime.v1","requestId":"req-call-1","method":"runtime.callApi","params":{"session":{"userDid":"did:wba:user.example","agentDid":"did:wba:agent.example","merchantDid":"did:wba:coffee-merchant.example","skillId":"coffee","sessionId":"session-ipc"},"apiName":"searchDrinks","arguments":{"query":"latte"}}}'
```

`runtime-json` is the first Phase 4 headless Host integration surface. It uses `headless-cli-json` over local process stdio and returns a stable envelope with `apiVersion`, `requestId`, `method`, `status`, `result` or `error`, `redaction`, and `transport`. It is not an HTTP/gRPC sidecar and does not provide production Host UI, persistent session storage, or production consent providers. Capability tokens and private key paths must not appear in responses; parse errors, invalid params, unsupported versions, and unsupported methods all return redacted JSON envelopes.

Run the coffee flow against a localhost server:

```bash
cargo run -p dock-cli -- run-demo \
  --skill examples/coffee-skill \
  --server http://127.0.0.1:3000
```

Equivalent explicit credential flags are also supported:

```bash
cargo run -p dock-cli -- run-demo \
  --skill examples/coffee-skill \
  --server http://127.0.0.1:3000 \
  --did-document /path/to/identity/did_document.json \
  --private-key /path/to/identity/key-1-private.pem \
  --agent-did did:wba:agent.example
```

The same values can be supplied through `ANP_DOCK_DID_DOCUMENT`, `ANP_DOCK_PRIVATE_KEY`, `ANP_DOCK_USER_DID`, `ANP_DOCK_AGENT_DID`, `ANP_DOCK_IDENTITY_HANDLE`, and `ANP_DOCK_IDENTITY_ROOT`. `ANP_DOCK_USER_DID` is optional when the DID document contains a valid `id`.

`run-demo` performs:

1. ANP DID challenge/login against the localhost coffee service.
2. Local server coffee API checks for drinks, order confirmation, and mock payment.
3. Local Skill loading from `examples/coffee-skill`.
4. Local Skill API execution through `dock-core` and the QuickJS API VM.
5. Skill-side `wx.login`, `wx.checkSession`, and `wx.request` calls to the localhost coffee service.
6. Component VM rendering for `drink-list`, `order-confirm`, and `payment-result`.
7. Component `api/call` action routing for `confirmOrder` and `payOrder`.
8. Dev/headless mock approval for high-risk consent and audit proof recording.
9. Payment-result card expiration handling.

CLI output is JSON. Capability tokens, `Authorization`, HTTP signature headers, and DID private key paths are used internally and are printed only as `[REDACTED]` or omitted from JS-visible response headers.

The local CLI harness uses the explicit dev/headless consent adapter (`dev-headless-consent`, actor `host.headless.dev`) so the coffee flow can run without a real Host UI. That adapter is not a production consent provider. Production Host integrations must provide their own `HostConsentAdapter` and must preserve fail-closed behavior for denied or unavailable consent providers.

Step 04-07 keeps the append-only JSONL `FileAuditSink` as local evidence only, not a production audit backend. The local coffee demo still prints collected audit events in the run output for developer inspection; deployment-grade audit backend configuration, encryption, access control, migration, export approval, durability monitoring, and privacy deletion are handled by the later production runtime and operations phases.

## Run The Mac Chatbot Host

The desktop demo lives in `mac-app/AnpMiniappDockMac`. It keeps Skill loading local (`examples/coffee-skill` on disk), while login and business API calls go to a localhost coffee HTTP service. The UI is a chatbot:

1. enter a user need, for example `我要点一杯咖啡`;
2. the app recognizes the intent with an OpenAI-compatible chat-completions API;
3. the app calls the local MiniApp container / Coffee Skill;
4. Skill-returned components are rendered as SwiftUI chat attachments.

Configure the OpenAI-compatible API in your shell startup file if you want Xcode/Finder launches to see it:

```bash
# ~/.zshrc
export OPENAI_BASE_URL=https://didhost.cc
export OPENAI_API_KEY=...
export OPENAI_MODEL=gpt-5.4
```

Do not commit or print real API keys. If `OPENAI_API_KEY` is empty or the remote call fails, the app uses a local keyword fallback for the coffee demo. Force that deterministic fallback with `ANP_DOCK_DISABLE_OPENAI=1`.

Open the Xcode project:

```bash
open mac-app/AnpMiniappDockMac/AnpMiniappDockMac.xcodeproj
```

Run a headless smoke test:

```bash
cd mac-app/AnpMiniappDockMac
ANP_DOCK_DISABLE_OPENAI=1 ANP_DOCK_MAC_HEADLESS=1 \
  ANP_DOCK_CHAT_PROMPT='我要点一杯咖啡' \
  swift run
```

The Mac app uses `examples/coffee-fastapi-server/.venv/bin/uvicorn` when that venv exists. If not, it starts the Rust `demo-server` fallback on a random localhost port.

## Random Port Smoke

For automated Rust checks, start `demo-server` with `--port 0`, read the printed `listening on` URL, and pass it to `dock-cli run-demo`.

```bash
cargo run -p demo-server -- \
  --port 0 \
  --skill examples/coffee-skill \
  --token-issuer-secret test-only-local-secret \
  --trusted-did-document '<user-did>=examples/identity/did_document.json'
```

This avoids port conflicts in CI-like local runs. The FastAPI runbook uses fixed port `8008` for easy localhost testing.

## Troubleshooting

- `connection refused`: confirm the FastAPI or Rust demo server is running and use the exact printed URL.
- `ModuleNotFoundError: fastapi`: activate the venv and run `pip install -r examples/coffee-fastapi-server/requirements.txt`.
- `unknown_did` or `invalid_signature`: verify that `--trusted-did-document` uses the DID document `id` and that the CLI signs with the matching private key.
- `unknown_challenge`: the challenge is unknown, expired, or already consumed. Request a new challenge before retrying login; do not reuse a `challengeId` after any failed login attempt.
- `revoked_token` or `replayed_token`: clear the local session and run `wx.login` / `dock-cli run-demo` again so the host obtains a fresh capability token.
- `token_issuer_unavailable`: start `demo-server` with `--token-issuer-secret`.
- `validation_failed`: inspect the `inputSchema` requirements in `examples/coffee-skill/mcp.json`.
- `component VM failed`: run `cargo test -p component-runtime` and inspect the component `index.js`, `index.wxml`, and `index.wxss`.
- `consent_required`: production hosts must provide a consent decision. The CLI demo uses a mock approval gate for P0 verification.
