# Local Demo Runbook

This runbook describes how to run the Rust MVP locally with the mock coffee Skill.

## Prerequisites

- Rust toolchain `1.88.0`; the repository pins it with `rust-toolchain.toml`.
- Network access for the first Cargo dependency fetch.
- A local test DID identity is required for challenge signing. Use a non-production fixture identity and keep its private key outside the repository.

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
```

## Start The Demo Server

Use port `3000` for a stable local URL:

```bash
cargo run -p demo-server -- \
  --host 127.0.0.1 \
  --port 3000 \
  --skill examples/coffee-skill \
  --token-issuer-secret test-only-local-secret \
  --trusted-did-document <user-did>=/path/to/identity/did_document.json
```

The server exposes:

- `GET /health`
- `GET /registry/agents`
- `GET /agents/coffee/manifest`
- `GET /agents/coffee/SKILL.md`
- `GET /agents/coffee/mcp.json`
- `POST /agents/coffee/auth/challenge`
- `POST /agents/coffee/auth/login`
- `GET /api/drinks`
- `POST /api/order/confirm`
- `POST /api/order/pay`
- `GET /audit`

## Run CLI Commands

Validate the Skill:

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
```

Call an atomic API:

```bash
cargo run -p dock-cli -- call-api examples/coffee-skill searchDrinks '{}'
```

Preview a component:

```bash
cargo run -p dock-cli -- preview-component examples/coffee-skill components/drink-list/index '{"apiName":"searchDrinks","structuredContent":{"drinks":[{"id":"latte","name":"Latte","price":18}]}}'
```

Preview a CardSpec fallback:

```bash
cargo run -p dock-cli -- preview-card '{"content":[{"type":"text","text":"paid"}],"structuredContent":{"orderId":"order_demo_001","status":"paid"}}'
```

Run the coffee flow against the server:

```bash
cargo run -p dock-cli -- run-demo \
  --skill examples/coffee-skill \
  --server http://127.0.0.1:3000 \
  --identity-handle miniapp-test.awiki.ai \
  --identity-root /path/to/identity-store/identities
```

`run-demo` performs:

1. ANP DID challenge/login against `demo-server`.
2. Demo-server coffee API checks for drinks, order confirmation, and mock payment.
3. Local Skill API execution through `dock-core` and the QuickJS API VM.
4. Component VM rendering for `drink-list`, `order-confirm`, and `payment-result`.
5. Component `api/call` action routing for `confirmOrder` and `payOrder`.
6. Mock approval for high-risk consent and audit proof recording.
7. Payment-result card expiration handling.

CLI output is JSON. Capability tokens are used internally and are printed only as `[REDACTED]`.

## Random Port Smoke

For automated checks, start `demo-server` with `--port 0`, read the printed `listening on` URL, and pass it to `dock-cli run-demo`.

```bash
cargo run -p demo-server -- \
  --port 0 \
  --skill examples/coffee-skill \
  --token-issuer-secret test-only-local-secret \
  --trusted-did-document <user-did>=/path/to/identity/did_document.json
```

This avoids port conflicts in CI-like local runs.

## Troubleshooting

- `connection refused`: confirm `demo-server` is running and use the exact printed URL.
- `validation_failed`: inspect the `inputSchema` requirements in `examples/coffee-skill/mcp.json`.
- `component VM failed`: run `cargo test -p component-runtime` and inspect the component `index.js`, `index.wxml`, and `index.wxss`.
- `consent_required`: production hosts must provide a consent decision. The CLI demo uses a mock approval gate for P0 verification.
