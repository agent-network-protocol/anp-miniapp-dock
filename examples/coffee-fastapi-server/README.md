# Localhost Coffee FastAPI Server

This is the Python/FastAPI remote HTTP service for the coffee demo. The MiniApp
Skill is still read from `examples/coffee-skill` locally; this service only
simulates the remote HTTP DID login and coffee business APIs reached through
`wx.login()` + `wx.request()`.

## What this server validates

The auth endpoints are aligned with the Rust Dock `DockDidChallengeProof`
contract:

1. `POST /agents/coffee/auth/challenge` returns `challengeId`, `nonce`,
   `merchantDid`, `issuedAtMs`, `expiresAtMs`, and `audience`.
2. The container signs a challenge payload with the Example Identity private key.
3. `POST /agents/coffee/auth/login` verifies the signed payload using the trusted
   DID document and returns a scoped `dock.capability.v1` Bearer token.
4. `/api/login` no longer exchanges arbitrary wx codes for tokens. It requires an
   existing `Authorization: Bearer <capabilityToken>` header and only returns
   login/account status.
5. Coffee business APIs validate both the Bearer token and route scope.

The implementation in `auth.py` uses the ANP Python SDK HTTP signature verifier
first when that API is importable. A small compatibility verifier for HTTP
Message Signatures and Ed25519 DID `Multikey` documents is kept as a fallback so
the service can still start in partially provisioned demo environments.

## Setup

```bash
cd examples/coffee-fastapi-server
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

Required demo identity files are loaded from the repository by default:

```text
examples/identity/did_document.json
examples/identity/key-1-private.pem   # used by the Rust container, never by this server
```

## Configuration

All values have localhost demo defaults:

```bash
export ANP_COFFEE_MERCHANT_DID=did:wba:coffee-merchant.example
export ANP_COFFEE_PUBLIC_BASE_URL=http://127.0.0.1:8008
export ANP_COFFEE_TRUSTED_DID_DOCUMENT=../identity/did_document.json
export ANP_COFFEE_TOKEN_ISSUER_SECRET=test-only-token-issuer-secret
export ANP_COFFEE_CHALLENGE_TTL_MS=300000
export ANP_COFFEE_TOKEN_TTL_MS=900000
```

Do not use the default token issuer secret outside local demo runs.

## Run

```bash
uvicorn app:app --host 127.0.0.1 --port 8008
```

Then run the Rust container demo against this service:

```bash
cargo run -p dock-cli -- run-demo \
  --skill examples/coffee-skill \
  --server http://127.0.0.1:8008
```

The Skill package is still read locally; the server's `/agents/coffee/package.zip`
endpoint intentionally returns a no-op marker.

## Auth contract summary

### Challenge response

```json
{
  "challengeId": "challenge-...",
  "merchantDid": "did:wba:coffee-merchant.example",
  "nonce": "nonce-...",
  "issuedAtMs": 1780000000000,
  "expiresAtMs": 1780000300000,
  "audience": "http://127.0.0.1:8008/agents/coffee/auth/login"
}
```

### Login request

`/agents/coffee/auth/login` expects `signedChallenge` shaped like Rust
`DockDidChallengeProof`:

```json
{
  "type": "anp-http-signature/v1",
  "method": "POST",
  "url": "http://127.0.0.1:8008/agents/coffee/auth/login",
  "headers": {
    "Signature-Input": "sig1=(...)...",
    "Signature": "sig1=:...:",
    "Content-Digest": "sha-256=:...:"
  },
  "payload": {
    "challengeId": "challenge-...",
    "nonce": "nonce-...",
    "merchantDid": "did:wba:coffee-merchant.example",
    "userDid": "did:wba:...",
    "agentDid": "did:wba:agent.example",
    "skillId": "coffee",
    "sessionId": "session-cli",
    "audience": "http://127.0.0.1:8008/agents/coffee/auth/login",
    "issuedAtMs": 1780000000000,
    "expiresAtMs": 1780000300000
  }
}
```

The payload bytes are Rust-compatible `serde_json::to_vec` style JSON with this
field order and no spaces. If `agentDid` is absent it is encoded as `null`, not
omitted.

### Token scopes

Login returns a scoped token with these scopes:

```text
coffee:drinks:read
coffee:order:confirm
coffee:order:pay
coffee:order:read
```

Route requirements:

```text
POST /api/login          valid Bearer token, no new token issuance
GET  /api/drinks         coffee:drinks:read
POST /api/order/confirm  coffee:order:confirm
POST /api/order/pay      coffee:order:pay
GET  /api/order/{id}     coffee:order:read
```

Responses and audit records redact tokens, signatures, Authorization values,
proof material, secrets, and private-key references.

## Local syntax check

Per the demo integration workflow, a local syntax check is enough before the
larger Rust/container work is integrated:

```bash
cd examples/coffee-fastapi-server
PYTHONDONTWRITEBYTECODE=1 python -m py_compile app.py auth.py
```
