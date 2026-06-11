"""DID challenge proof and scoped capability auth for the coffee demo.

This module mirrors the Rust ``anp-adapter`` demo contract closely enough for
FastAPI to validate ``DockDidChallengeProof`` values produced by the Dock
container.  The ANP Python SDK HTTP signature verifier is used when it is
available; the verifier below is an explicit compatibility layer for the same
Rust contract:

* proof type: ``anp-http-signature/v1``
* method: ``POST``
* signed payload: Rust ``ChallengeProofPayload`` JSON, camelCase field order
* HTTP message signature covered components: ``@method``, ``@target-uri``,
  ``@authority``, ``content-digest``
* key material: trusted DID document, Ed25519 ``Multikey`` / JWK / base58
* capability token: HS256 JWT with Rust ``dock.capability.v1`` claims

If the ``anp`` package exposes the expected HTTP signature helper, that SDK
verifier is used first.  The local compatibility verifier remains as a fallback
so the service can still start in partially provisioned demo environments.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import os
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping
from urllib.parse import urlparse

try:  # Optional: keep the demo service importable before dependencies are installed.
    from anp.authentication.http_signatures import (  # type: ignore
        verify_http_message_signature as _sdk_verify_http_message_signature,
    )

    ANP_SDK_AVAILABLE = True
except Exception:  # pragma: no cover - diagnostic only.
    _sdk_verify_http_message_signature = None
    ANP_SDK_AVAILABLE = False

try:  # ``cryptography`` is required for real Ed25519 signature verification.
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
except Exception:  # pragma: no cover - handled as verifier unavailable at runtime.
    Ed25519PublicKey = None  # type: ignore[assignment]

CHALLENGE_PROOF_TYPE = "anp-http-signature/v1"
CHALLENGE_PROOF_METHOD = "POST"
CAPABILITY_TOKEN_VERSION = "dock.capability.v1"
SIGNATURE_SKEW_MS = 300_000
DEFAULT_MERCHANT_DID = "did:wba:coffee-merchant.example"
DEFAULT_PUBLIC_BASE_URL = "http://127.0.0.1:8008"
DEFAULT_TOKEN_ISSUER_SECRET = "test-only-token-issuer-secret"
COFFEE_DEMO_SCOPES = [
    "coffee:drinks:read",
    "coffee:order:confirm",
    "coffee:order:pay",
    "coffee:order:read",
]

_B58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
_B58_INDEX = {char: index for index, char in enumerate(_B58_ALPHABET)}


@dataclass(frozen=True)
class AuthConfig:
    merchant_did: str
    public_base_url: str
    trusted_did_document: Path
    token_issuer_secret: str
    challenge_ttl_ms: int
    token_ttl_ms: int

    @classmethod
    def from_env(cls) -> "AuthConfig":
        examples_root = Path(__file__).resolve().parents[1]
        default_did_doc = examples_root / "identity" / "did_document.json"
        return cls(
            merchant_did=os.getenv("ANP_COFFEE_MERCHANT_DID", DEFAULT_MERCHANT_DID),
            public_base_url=os.getenv("ANP_COFFEE_PUBLIC_BASE_URL", DEFAULT_PUBLIC_BASE_URL),
            trusted_did_document=Path(
                os.getenv("ANP_COFFEE_TRUSTED_DID_DOCUMENT", str(default_did_doc))
            ),
            token_issuer_secret=os.getenv(
                "ANP_COFFEE_TOKEN_ISSUER_SECRET", DEFAULT_TOKEN_ISSUER_SECRET
            ),
            challenge_ttl_ms=_int_env("ANP_COFFEE_CHALLENGE_TTL_MS", 5 * 60 * 1000),
            token_ttl_ms=_int_env("ANP_COFFEE_TOKEN_TTL_MS", 15 * 60 * 1000),
        )

    @property
    def login_audience(self) -> str:
        return f"{self.public_base_url.rstrip('/')}/agents/coffee/auth/login"

    @property
    def verifier_name(self) -> str:
        suffix = "+anp-sdk-detected" if ANP_SDK_AVAILABLE else "+compat-only"
        return f"dock-python-http-signature{suffix}"


def _int_env(name: str, default: int) -> int:
    raw = os.getenv(name)
    if raw is None:
        return default
    try:
        value = int(raw)
    except ValueError:
        return default
    return value if value > 0 else default


def now_ms() -> int:
    return int(time.time() * 1000)


class AuthFailure(Exception):
    def __init__(self, code: str, status_code: int = 401, message: str | None = None) -> None:
        super().__init__(message or code)
        self.code = code
        self.status_code = status_code
        self.message = message or code


class VerifierUnavailable(AuthFailure):
    def __init__(self, message: str) -> None:
        super().__init__("did_verifier_unavailable", 500, message)


@dataclass(frozen=True)
class VerifiedChallengeProof:
    signer_did: str
    key_id: str
    auth_scheme: str
    verifier: str


@dataclass(frozen=True)
class CapabilityIssueOutcome:
    token: str
    expires_at_ms: int
    claims: dict[str, Any]


def make_error(code: str, status_code: int = 401, message: str | None = None) -> AuthFailure:
    return AuthFailure(code=code, status_code=status_code, message=message)


def expected_challenge_payload(
    *,
    challenge: Mapping[str, Any],
    session_id: str,
    skill_id: str,
    user_did: str,
    agent_did: str | None,
    audience: str,
) -> dict[str, Any]:
    """Build the exact Rust ChallengeProofPayload JSON shape and field order."""

    return {
        "challengeId": challenge["challengeId"],
        "nonce": challenge["nonce"],
        "merchantDid": challenge["merchantDid"],
        "userDid": user_did,
        "agentDid": agent_did,
        "skillId": skill_id,
        "sessionId": session_id,
        "audience": audience,
        "issuedAtMs": challenge["issuedAtMs"],
        "expiresAtMs": challenge["expiresAtMs"],
    }


def verify_dock_did_challenge_proof(
    *,
    proof: Mapping[str, Any],
    expected_payload: Mapping[str, Any],
    did_document: Mapping[str, Any],
    config: AuthConfig,
    at_ms: int | None = None,
) -> VerifiedChallengeProof:
    """Verify a Rust DockDidChallengeProof-compatible signed challenge."""

    if proof.get("type") != CHALLENGE_PROOF_TYPE:
        raise make_error("invalid_signature")
    if proof.get("method") != CHALLENGE_PROOF_METHOD:
        raise make_error("scope_mismatch")
    if proof.get("url") != expected_payload.get("audience"):
        raise make_error("scope_mismatch")
    if proof.get("payload") != dict(expected_payload):
        raise make_error("scope_mismatch")

    _validate_payload(expected_payload, at_ms or now_ms())
    headers = proof.get("headers")
    if not isinstance(headers, Mapping):
        raise make_error("invalid_signature")

    metadata = _extract_signature_metadata(headers)
    _validate_signature_metadata(metadata, expected_payload)
    signer_did = _signer_did_from_key_id(metadata["keyid"])
    if signer_did != expected_payload.get("userDid"):
        raise make_error("scope_mismatch")
    if did_document.get("id") != signer_did:
        raise make_error("unknown_did")

    body = _canonical_payload_bytes(expected_payload)
    _verify_http_message_signature(
        did_document=did_document,
        method=CHALLENGE_PROOF_METHOD,
        url=str(expected_payload["audience"]),
        headers=headers,
        body=body,
        parsed_metadata=metadata,
    )
    return VerifiedChallengeProof(
        signer_did=signer_did,
        key_id=metadata["keyid"],
        auth_scheme=CHALLENGE_PROOF_TYPE,
        verifier=config.verifier_name,
    )


def load_trusted_did_document(config: AuthConfig, did: str) -> dict[str, Any]:
    try:
        document = json.loads(config.trusted_did_document.read_text(encoding="utf-8"))
    except Exception as exc:
        raise make_error("unknown_did", message=f"trusted DID document is not readable: {exc}")
    if document.get("id") != did:
        raise make_error("unknown_did", message="trusted DID document does not match userDid")
    return document


def issue_capability_token(
    *,
    config: AuthConfig,
    user_did: str,
    agent_did: str | None,
    skill_id: str,
    session_id: str,
    scopes: list[str] | None = None,
) -> CapabilityIssueOutcome:
    issued_at_ms = now_ms()
    iat = issued_at_ms // 1000
    exp = _ceil_ms_to_seconds(issued_at_ms + config.token_ttl_ms)
    claims = {
        "iss": config.merchant_did,
        "aud": config.merchant_did,
        "sub": user_did,
        "merchantDid": config.merchant_did,
        "userDid": user_did,
        "agentDid": agent_did,
        "skillId": skill_id,
        "sessionId": session_id,
        "scopes": scopes or list(COFFEE_DEMO_SCOPES),
        "iat": iat,
        "nbf": iat,
        "exp": exp,
        "jti": uuid.uuid4().hex,
        "version": CAPABILITY_TOKEN_VERSION,
    }
    _validate_capability_claims_basic(claims)
    header = {"alg": "HS256", "typ": CAPABILITY_TOKEN_VERSION}
    token = _jwt_encode(header, claims, config.token_issuer_secret)
    return CapabilityIssueOutcome(token=token, expires_at_ms=exp * 1000, claims=claims)


def verify_bearer_token(
    *,
    config: AuthConfig,
    authorization: str | None,
    expected_skill_id: str,
    required_scope: str | None,
) -> dict[str, Any]:
    if not authorization or not authorization.startswith("Bearer "):
        raise make_error("missing_token")
    token = authorization.removeprefix("Bearer ").strip()
    if not token or token.startswith("demo-cap-"):
        raise make_error("invalid_token", status_code=403)
    claims = _jwt_decode(token, config.token_issuer_secret)
    _validate_capability_claims_basic(claims)
    _validate_capability_claims_time(claims)
    if (
        claims.get("iss") != config.merchant_did
        or claims.get("aud") != config.merchant_did
        or claims.get("merchantDid") != config.merchant_did
        or claims.get("skillId") != expected_skill_id
    ):
        raise make_error("scope_mismatch")
    if required_scope and required_scope not in claims.get("scopes", []):
        raise make_error("insufficient_scope", status_code=403)
    return claims


def auth_failure_detail(error: AuthFailure) -> dict[str, str]:
    return {"code": error.code, "message": error.message}


def redacted(value: Any) -> str:
    return "[REDACTED]" if value is not None else "[REDACTED]"


def _validate_payload(payload: Mapping[str, Any], at_ms: int) -> None:
    required = [
        "challengeId",
        "nonce",
        "merchantDid",
        "userDid",
        "skillId",
        "sessionId",
        "audience",
        "issuedAtMs",
        "expiresAtMs",
    ]
    if any(payload.get(name) in (None, "") for name in required):
        raise make_error("invalid_signature")
    issued_at_ms = int(payload["issuedAtMs"])
    expires_at_ms = int(payload["expiresAtMs"])
    if issued_at_ms >= expires_at_ms:
        raise make_error("invalid_signature")
    if expires_at_ms <= at_ms:
        raise make_error("expired_challenge")
    if issued_at_ms > at_ms + SIGNATURE_SKEW_MS:
        raise make_error("invalid_signature")


def _canonical_payload_bytes(payload: Mapping[str, Any]) -> bytes:
    ordered = {
        "challengeId": payload["challengeId"],
        "nonce": payload["nonce"],
        "merchantDid": payload["merchantDid"],
        "userDid": payload["userDid"],
        "agentDid": payload.get("agentDid"),
        "skillId": payload["skillId"],
        "sessionId": payload["sessionId"],
        "audience": payload["audience"],
        "issuedAtMs": payload["issuedAtMs"],
        "expiresAtMs": payload["expiresAtMs"],
    }
    return json.dumps(ordered, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def _extract_signature_metadata(headers: Mapping[str, Any]) -> dict[str, Any]:
    signature_input = _get_header(headers, "Signature-Input")
    signature_header = _get_header(headers, "Signature")
    if not signature_input or not signature_header:
        raise make_error("invalid_signature")
    label_input, components, params = _parse_signature_input(signature_input)
    label_signature, _ = _parse_signature_header(signature_header)
    if label_input != label_signature:
        raise make_error("invalid_signature")
    keyid = params.get("keyid")
    created = _parse_int_param(params.get("created"))
    expires = _parse_optional_int_param(params.get("expires"))
    if not keyid or created is None:
        raise make_error("invalid_signature")
    return {
        "label": label_input,
        "components": components,
        "keyid": keyid,
        "nonce": params.get("nonce"),
        "created": created,
        "expires": expires,
    }


def _validate_signature_metadata(metadata: Mapping[str, Any], payload: Mapping[str, Any]) -> None:
    if metadata.get("nonce") != payload.get("nonce"):
        raise make_error("invalid_signature")
    if metadata.get("created") != int(payload["issuedAtMs"]) // 1000:
        raise make_error("invalid_signature")
    if metadata.get("expires") != _ceil_ms_to_seconds(int(payload["expiresAtMs"])):
        raise make_error("invalid_signature")
    components = [str(component).lower() for component in metadata.get("components", [])]
    for required in ["@method", "@target-uri", "@authority", "content-digest"]:
        if required not in components:
            raise make_error("invalid_signature")


def _verify_http_message_signature(
    *,
    did_document: Mapping[str, Any],
    method: str,
    url: str,
    headers: Mapping[str, Any],
    body: bytes,
    parsed_metadata: Mapping[str, Any],
) -> None:
    if _sdk_verify_http_message_signature is not None:
        ok, _message, _metadata = _sdk_verify_http_message_signature(
            dict(did_document),
            method,
            url,
            {str(key): str(value) for key, value in headers.items()},
            body,
        )
        if not ok:
            raise make_error("invalid_signature")
        return

    signature_header = _get_header(headers, "Signature")
    label_signature, signature_bytes = _parse_signature_header(signature_header or "")
    if label_signature != parsed_metadata.get("label"):
        raise make_error("invalid_signature")
    digest = _get_header(headers, "Content-Digest")
    if not digest or digest.strip() != _content_digest(body):
        raise make_error("invalid_signature")
    signature_base = _build_signature_base(
        components=list(parsed_metadata["components"]),
        method=method,
        url=url,
        headers=headers,
        created=int(parsed_metadata["created"]),
        expires=parsed_metadata.get("expires"),
        nonce=parsed_metadata.get("nonce"),
        keyid=str(parsed_metadata["keyid"]),
    )
    public_key_bytes = _extract_ed25519_public_key(did_document, str(parsed_metadata["keyid"]))
    if Ed25519PublicKey is None:
        raise VerifierUnavailable("cryptography is required for Ed25519 DID verification")
    try:
        Ed25519PublicKey.from_public_bytes(public_key_bytes).verify(
            signature_bytes,
            signature_base.encode("utf-8"),
        )
    except Exception:
        raise make_error("invalid_signature")


def _build_signature_base(
    *,
    components: list[str],
    method: str,
    url: str,
    headers: Mapping[str, Any],
    created: int,
    expires: int | None,
    nonce: str | None,
    keyid: str,
) -> str:
    lines = []
    for component in components:
        lines.append(f'"{component}": {_component_value(component, method, url, headers)}')
    lines.append(
        f'"@signature-params": '
        f'{_serialize_signature_params(components, created, expires, nonce, keyid)}'
    )
    return "\n".join(lines)


def _component_value(component: str, method: str, url: str, headers: Mapping[str, Any]) -> str:
    lower = component.lower()
    if lower == "@method":
        return method.upper()
    if lower == "@target-uri":
        return url
    if lower == "@authority":
        parsed = urlparse(url)
        if not parsed.hostname:
            raise make_error("invalid_signature")
        return f"{parsed.hostname}:{parsed.port}" if parsed.port else parsed.hostname
    value = _get_header(headers, component)
    if value is None:
        raise make_error("invalid_signature")
    return value


def _serialize_signature_params(
    components: list[str], created: int, expires: int | None, nonce: str | None, keyid: str
) -> str:
    quoted = " ".join(f'"{value}"' for value in components)
    params = [f"created={created}"]
    if expires is not None:
        params.append(f"expires={expires}")
    if nonce is not None:
        params.append(f'nonce="{nonce}"')
    params.append(f'keyid="{keyid}"')
    return f"({quoted});{';'.join(params)}"


def _parse_signature_input(value: str) -> tuple[str, list[str], dict[str, str]]:
    try:
        label, remainder = value.split("=", 1)
        open_index = remainder.index("(")
        close_index = remainder.index(")")
    except ValueError:
        raise make_error("invalid_signature")
    if close_index <= open_index:
        raise make_error("invalid_signature")
    raw_components = remainder[open_index + 1 : close_index]
    components = [item.strip().strip('"') for item in raw_components.split() if item.strip()]
    if not components:
        raise make_error("invalid_signature")
    raw_params = remainder[close_index + 1 :].lstrip(";")
    params: dict[str, str] = {}
    for raw in raw_params.split(";"):
        if not raw.strip():
            continue
        if "=" not in raw:
            raise make_error("invalid_signature")
        name, param_value = raw.split("=", 1)
        params[name] = param_value.strip().strip('"')
    return label, components, params


def _parse_signature_header(value: str) -> tuple[str, bytes]:
    try:
        label, remainder = value.split("=", 1)
    except ValueError:
        raise make_error("invalid_signature")
    if not (remainder.startswith(":") and remainder.endswith(":")):
        raise make_error("invalid_signature")
    try:
        signature = base64.b64decode(remainder[1:-1], validate=True)
    except Exception:
        raise make_error("invalid_signature")
    return label, signature


def _extract_ed25519_public_key(did_document: Mapping[str, Any], key_id: str) -> bytes:
    method = _find_verification_method(did_document, key_id)
    if not method:
        raise make_error("unknown_did")
    method_type = method.get("type")
    if method_type not in {"Multikey", "Ed25519VerificationKey2018", "Ed25519VerificationKey2020"}:
        raise make_error("invalid_signature")
    if isinstance(method.get("publicKeyJwk"), Mapping):
        jwk = method["publicKeyJwk"]
        if jwk.get("crv") != "Ed25519" or not isinstance(jwk.get("x"), str):
            raise make_error("invalid_signature")
        return _b64url_decode(jwk["x"])
    if isinstance(method.get("publicKeyMultibase"), str):
        decoded = _b58decode(method["publicKeyMultibase"].removeprefix("z"))
        if len(decoded) == 34 and decoded.startswith(bytes([0xED, 0x01])):
            decoded = decoded[2:]
        if len(decoded) != 32:
            raise make_error("invalid_signature")
        return decoded
    if isinstance(method.get("publicKeyBase58"), str):
        decoded = _b58decode(method["publicKeyBase58"])
        if len(decoded) != 32:
            raise make_error("invalid_signature")
        return decoded
    raise make_error("invalid_signature")


def _find_verification_method(did_document: Mapping[str, Any], key_id: str) -> Mapping[str, Any] | None:
    for field in ("verificationMethod", "authentication", "assertionMethod"):
        values = did_document.get(field, [])
        if not isinstance(values, list):
            continue
        for item in values:
            if isinstance(item, Mapping) and item.get("id") == key_id:
                return item
            if isinstance(item, str) and item == key_id:
                for method in did_document.get("verificationMethod", []):
                    if isinstance(method, Mapping) and method.get("id") == key_id:
                        return method
    return None


def _signer_did_from_key_id(key_id: str) -> str:
    if "#" not in key_id:
        raise make_error("invalid_signature")
    did, _ = key_id.split("#", 1)
    if not did:
        raise make_error("invalid_signature")
    return did


def _get_header(headers: Mapping[str, Any], name: str) -> str | None:
    for key, value in headers.items():
        if str(key).lower() == name.lower():
            return str(value)
    return None


def _content_digest(body: bytes) -> str:
    return f"sha-256=:{base64.b64encode(hashlib.sha256(body).digest()).decode('ascii')}:"


def _ceil_ms_to_seconds(ms: int) -> int:
    return (ms + 999) // 1000


def _parse_int_param(value: str | None) -> int | None:
    if value in (None, ""):
        return None
    try:
        return int(value)
    except ValueError:
        return None


def _parse_optional_int_param(value: str | None) -> int | None:
    if value in (None, ""):
        return None
    return _parse_int_param(value)


def _jwt_encode(header: Mapping[str, Any], claims: Mapping[str, Any], secret: str) -> str:
    signing_input = ".".join(
        [
            _b64url_encode(json.dumps(header, separators=(",", ":")).encode("utf-8")),
            _b64url_encode(json.dumps(claims, separators=(",", ":")).encode("utf-8")),
        ]
    )
    signature = hmac.new(secret.encode("utf-8"), signing_input.encode("ascii"), hashlib.sha256)
    return f"{signing_input}.{_b64url_encode(signature.digest())}"


def _jwt_decode(token: str, secret: str) -> dict[str, Any]:
    parts = token.split(".")
    if len(parts) != 3:
        raise make_error("invalid_token", status_code=403)
    signing_input = f"{parts[0]}.{parts[1]}"
    expected = hmac.new(secret.encode("utf-8"), signing_input.encode("ascii"), hashlib.sha256)
    try:
        actual = _b64url_decode(parts[2])
    except Exception:
        raise make_error("invalid_token", status_code=403)
    if not hmac.compare_digest(expected.digest(), actual):
        raise make_error("invalid_token", status_code=403)
    try:
        header = json.loads(_b64url_decode(parts[0]))
        claims = json.loads(_b64url_decode(parts[1]))
    except Exception:
        raise make_error("invalid_token", status_code=403)
    if header.get("alg") != "HS256":
        raise make_error("invalid_token", status_code=403)
    return claims


def _validate_capability_claims_basic(claims: Mapping[str, Any]) -> None:
    required = [
        "iss",
        "aud",
        "sub",
        "merchantDid",
        "userDid",
        "skillId",
        "sessionId",
        "scopes",
        "iat",
        "nbf",
        "exp",
        "jti",
        "version",
    ]
    if any(claims.get(name) in (None, "") for name in required):
        raise make_error("invalid_token", status_code=403)
    if claims.get("version") != CAPABILITY_TOKEN_VERSION:
        raise make_error("invalid_token", status_code=403)
    if claims.get("sub") != claims.get("userDid"):
        raise make_error("invalid_token", status_code=403)
    scopes = claims.get("scopes")
    if not isinstance(scopes, list) or not scopes or any(not str(scope).strip() for scope in scopes):
        raise make_error("insufficient_scope", status_code=403)
    if int(claims["iat"]) > int(claims["nbf"]) or int(claims["nbf"]) >= int(claims["exp"]):
        raise make_error("invalid_token", status_code=403)


def _validate_capability_claims_time(claims: Mapping[str, Any]) -> None:
    now = now_ms() // 1000
    if int(claims["exp"]) <= now:
        raise make_error("expired_token", status_code=403)
    if int(claims["nbf"]) > now:
        raise make_error("invalid_token", status_code=403)


def _b64url_encode(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).decode("ascii").rstrip("=")


def _b64url_decode(value: str) -> bytes:
    padded = value + "=" * (-len(value) % 4)
    return base64.urlsafe_b64decode(padded.encode("ascii"))


def _b58decode(value: str) -> bytes:
    number = 0
    for char in value:
        try:
            digit = _B58_INDEX[char]
        except KeyError:
            raise make_error("invalid_signature")
        number = number * 58 + digit
    combined = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    leading_zeroes = len(value) - len(value.lstrip("1"))
    return b"\x00" * leading_zeroes + combined
