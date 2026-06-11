# Security Runbook

This runbook records the MVP security boundaries for `anp-miniapp-dock`.

## Scope

The MVP is a local Rust runtime and demo. It proves the runtime shape and security boundaries, but it is not a production deployment profile.

Implemented boundaries:

- Skill code runs in isolated QuickJS contexts.
- Atomic API VM and Component VM do not share JavaScript globals.
- CommonJS loading is restricted to files inside the Skill package.
- `eval`, `Function`, `process`, direct `fetch`, timers in Component VM, and unrestricted `require` are disabled.
- `dock-core` applies input validation, permission checks, consent checks, executor calls, result validation, render routing, and audit recording in order.
- `_meta` is not included in model-visible API results.
- High-risk APIs such as order confirmation and payment require consent before execution.
- Audit records and proofs use redacted parameter summaries.
- ANP adapter request policy denies by default unless the URL authority is allowlisted.
- Capability tokens are scoped by merchant DID, user DID, agent DID, Skill ID, session ID, and route scope.

## DID And Token Handling

Runtime and adapter layers own DID signing, challenge/login contracts, signed HTTP, and capability token cache behavior. Skill code and CLI output must not expose DID private keys, raw capability tokens, HTTP signatures, or bearer values.

The demo server verifies ANP DID challenge proofs and issues short-lived scoped capability tokens after challenge/login. `dock-cli run-demo` uses the token internally for demo-server business checks but prints only:

```json
{
  "capabilityToken": "[REDACTED]",
  "tokenReceived": true
}
```

## Consent And Audit

Risk is inferred from `mcp.json` metadata such as `_meta.anp.risk` and API shape. `order` and `payment` are treated as high-risk labels.

High-risk execution path:

```text
inputSchema validation
  -> permission check
  -> consent request
  -> executor
  -> result validation
  -> render routing
  -> audit record
```

If consent is required and not approved, execution fails closed before the API executor runs.

## Redaction Checks

Run these checks before release or after touching auth, logging, CLI, audit, or demo code:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
rg -n "capabilityToken|Authorization|Signature|private_key|private key|secret|token" crates examples docs README.md AGENTS.md -S
```

Expected hits include:

- redaction code and tests,
- fixture warnings,
- security documentation,
- internal token handling code.

Unexpected hits include raw bearer values, real DID credential paths, private key material, production merchant secrets, or logged signatures.

## Demo Limitations

- The coffee Skill and demo server use mock data only.
- Demo challenge proof uses ANP HTTP signatures over a typed challenge payload; `demo-signature` must not be accepted.
- Mock payment does not integrate a real payment provider.
- The CLI mock-approves consent for E2E verification. Production hosts must connect consent to a human authorization UI or policy engine.
- The Rust MVP outputs Render IR JSON and CardSpec fallback; production host rendering is a separate adapter concern.

## Required Review Areas

Security-sensitive changes require review of:

- DID credential provider and signed request code,
- token cache scope and expiry behavior,
- URL allowlist behavior,
- CLI and server output redaction,
- audit parameter summaries,
- consent fail-closed behavior,
- sandbox globals and CommonJS path resolution,
- component actions returning to `dock-core` instead of direct execution.
