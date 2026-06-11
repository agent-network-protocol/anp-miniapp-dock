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

During `run-demo`, the Skill JavaScript calls `wx.login()`, then uses `wx.request()` to access `/api/login`, `/api/drinks`, `/api/order/confirm`, and `/api/order/pay` on localhost.

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

## Run CLI Commands

Validate the Skill:

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
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
5. Skill-side `wx.login` and `wx.request` calls to the localhost coffee service.
6. Component VM rendering for `drink-list`, `order-confirm`, and `payment-result`.
7. Component `api/call` action routing for `confirmOrder` and `payOrder`.
8. Mock approval for high-risk consent and audit proof recording.
9. Payment-result card expiration handling.

CLI output is JSON. Capability tokens are used internally and are printed only as `[REDACTED]`.

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
- `token_issuer_unavailable`: start `demo-server` with `--token-issuer-secret`.
- `validation_failed`: inspect the `inputSchema` requirements in `examples/coffee-skill/mcp.json`.
- `component VM failed`: run `cargo test -p component-runtime` and inspect the component `index.js`, `index.wxml`, and `index.wxss`.
- `consent_required`: production hosts must provide a consent decision. The CLI demo uses a mock approval gate for P0 verification.
