"""Localhost FastAPI coffee merchant service for the MiniApp Dock demo.

The Skill is still loaded from ../coffee-skill by the Rust container. This
service simulates the remote HTTP DID login and coffee business APIs that the
Skill reaches through wx.login + wx.request.
"""

from __future__ import annotations

import json
import uuid
from pathlib import Path
from typing import Any

from auth import (
    AuthConfig,
    AuthFailure,
    COFFEE_DEMO_SCOPES,
    auth_failure_detail,
    expected_challenge_payload,
    issue_capability_token,
    load_trusted_did_document,
    now_ms,
    redacted,
    verify_bearer_token,
    verify_dock_did_challenge_proof,
)
from fastapi import FastAPI, Header, HTTPException, Query
from fastapi.responses import JSONResponse, PlainTextResponse
from pydantic import BaseModel, Field

AGENT_ID = "coffee"
SKILL_ROOT = Path(__file__).resolve().parents[1] / "coffee-skill"
SCOPE_DRINKS_READ = "coffee:drinks:read"
SCOPE_ORDER_CONFIRM = "coffee:order:confirm"
SCOPE_ORDER_PAY = "coffee:order:pay"
SCOPE_ORDER_READ = "coffee:order:read"

AUTH_CONFIG = AuthConfig.from_env()

app = FastAPI(title="Localhost Coffee Skill Service", version="0.2.0")


class ChallengeRequest(BaseModel):
    session_id: str = Field(alias="sessionId")
    skill_id: str = Field(alias="skillId")
    user_did: str = Field(alias="userDid")
    agent_did: str | None = Field(default=None, alias="agentDid")


class ChallengeLoginRequest(BaseModel):
    session_id: str = Field(alias="sessionId")
    skill_id: str = Field(alias="skillId")
    user_did: str = Field(alias="userDid")
    agent_did: str | None = Field(default=None, alias="agentDid")
    merchant_did: str = Field(alias="merchantDid")
    challenge_id: str = Field(alias="challengeId")
    signed_challenge: dict[str, Any] = Field(alias="signedChallenge")


class WxLoginRequest(BaseModel):
    code: str | None = None
    session_id: str | None = Field(default=None, alias="sessionId")
    skill_id: str | None = Field(default=None, alias="skillId")
    user_did: str | None = Field(default=None, alias="userDid")
    agent_did: str | None = Field(default=None, alias="agentDid")


class ConfirmOrderRequest(BaseModel):
    drink_id: str = Field(alias="drinkId")
    size: str | None = None
    sugar: str | None = None


class PayOrderRequest(BaseModel):
    order_id: str = Field(alias="orderId")


DRINKS = [
    {"id": "latte", "name": "Latte", "price": 18, "image": "https://img.example/latte.png"},
    {"id": "americano", "name": "Americano", "price": 15, "image": "https://img.example/americano.png"},
    {"id": "mocha", "name": "Mocha", "price": 20, "image": "https://img.example/mocha.png"},
]
challenges: dict[str, dict[str, Any]] = {}
orders: dict[str, dict[str, Any]] = {}
audit_records: list[dict[str, Any]] = []


def raise_auth(error: AuthFailure) -> None:
    raise HTTPException(status_code=error.status_code, detail=auth_failure_detail(error))


def record_audit(event: str, outcome: str, **fields: Any) -> None:
    sensitive = (
        "token",
        "authorization",
        "signature",
        "proof",
        "private",
        "secret",
        "capability",
    )
    safe_fields = {
        key: ("[REDACTED]" if any(item in key.lower() for item in sensitive) else value)
        for key, value in fields.items()
        if value is not None
    }
    audit_records.append({"atMs": now_ms(), "event": event, "outcome": outcome, **safe_fields})


def authorize(authorization: str | None, required_scope: str | None) -> dict[str, Any]:
    try:
        return verify_bearer_token(
            config=AUTH_CONFIG,
            authorization=authorization,
            expected_skill_id=AGENT_ID,
            required_scope=required_scope,
        )
    except AuthFailure as error:
        raise_auth(error)
        raise AssertionError("unreachable")


@app.get("/health")
def health() -> dict[str, str]:
    return {
        "status": "ok",
        "service": "coffee-fastapi-server",
        "authMode": "anp-http-signature/v1",
    }


@app.get("/registry/agents")
def registry_agents() -> dict[str, Any]:
    return {
        "agents": [
            {
                "id": AGENT_ID,
                "name": "Coffee Merchant Agent",
                "did": AUTH_CONFIG.merchant_did,
                "manifestUrl": "/agents/coffee/manifest",
                "skillUrl": "/agents/coffee/mcp.json",
            }
        ]
    }


@app.get("/agents/coffee/manifest")
def agent_manifest() -> dict[str, Any]:
    return {
        "id": AGENT_ID,
        "name": "Coffee Merchant Agent",
        "did": AUTH_CONFIG.merchant_did,
        "skill": {
            "skillMd": "/agents/coffee/SKILL.md",
            "mcpJson": "/agents/coffee/mcp.json",
            "package": "/agents/coffee/package.zip",
        },
        "auth": {
            "mode": "anp-http-signature/v1",
            "challenge": "/agents/coffee/auth/challenge",
            "login": "/agents/coffee/auth/login",
            "wxLogin": "/api/login",
            "tokenProfile": "dock.capability.v1",
            "tokenTransport": "bearer",
            "scopes": COFFEE_DEMO_SCOPES,
        },
        "apis": {
            "drinks": "/api/drinks",
            "confirmOrder": "/api/order/confirm",
            "payOrder": "/api/order/pay",
        },
    }


@app.get("/agents/coffee/SKILL.md", response_class=PlainTextResponse)
def skill_markdown() -> str:
    return (SKILL_ROOT / "SKILL.md").read_text(encoding="utf-8")


@app.get("/agents/coffee/mcp.json")
def mcp_json() -> JSONResponse:
    return JSONResponse(content=json.loads((SKILL_ROOT / "mcp.json").read_text(encoding="utf-8")))


@app.get("/agents/coffee/package.zip")
def package_zip_noop() -> dict[str, str]:
    return {"status": "p0_noop", "reason": "Skill package is loaded from the local filesystem in this demo"}


@app.post("/agents/coffee/auth/challenge")
def auth_challenge(request: ChallengeRequest) -> dict[str, Any]:
    issued_at_ms = now_ms()
    challenge_id = f"challenge-{uuid.uuid4().hex}"
    challenge = {
        "challengeId": challenge_id,
        "merchantDid": AUTH_CONFIG.merchant_did,
        "nonce": f"nonce-{uuid.uuid4().hex}",
        "issuedAtMs": issued_at_ms,
        "expiresAtMs": issued_at_ms + AUTH_CONFIG.challenge_ttl_ms,
        "audience": AUTH_CONFIG.login_audience,
    }
    challenges[challenge_id] = {"request": request.model_dump(by_alias=True), "challenge": challenge}
    record_audit(
        "auth.challenge",
        "ok",
        sessionId=request.session_id,
        skillId=request.skill_id,
        userDid=request.user_did,
        verifier=AUTH_CONFIG.verifier_name,
    )
    return challenge


@app.post("/agents/coffee/auth/login")
def auth_login(request: ChallengeLoginRequest) -> dict[str, Any]:
    record = challenges.get(request.challenge_id)
    audit_scope = {
        "sessionId": request.session_id,
        "skillId": request.skill_id,
        "userDid": request.user_did,
    }
    if not record:
        record_audit("auth.login", "denied", **audit_scope)
        raise_auth(AuthFailure("unknown_challenge"))
    challenge = record["challenge"]
    challenge_request = record["request"]
    if challenge["expiresAtMs"] <= now_ms():
        record_audit("auth.login", "denied", **audit_scope)
        raise_auth(AuthFailure("expired_challenge"))
    if request.merchant_did != AUTH_CONFIG.merchant_did:
        record_audit("auth.login", "denied", **audit_scope)
        raise_auth(AuthFailure("scope_mismatch"))
    if (
        challenge_request.get("sessionId") != request.session_id
        or challenge_request.get("skillId") != request.skill_id
        or challenge_request.get("userDid") != request.user_did
        or challenge_request.get("agentDid") != request.agent_did
        or challenge.get("merchantDid") != request.merchant_did
    ):
        record_audit("auth.login", "denied", **audit_scope)
        raise_auth(AuthFailure("scope_mismatch"))

    expected_payload = expected_challenge_payload(
        challenge=challenge,
        session_id=request.session_id,
        skill_id=request.skill_id,
        user_did=request.user_did,
        agent_did=request.agent_did,
        audience=challenge["audience"],
    )
    try:
        did_document = load_trusted_did_document(AUTH_CONFIG, request.user_did)
        verified = verify_dock_did_challenge_proof(
            proof=request.signed_challenge,
            expected_payload=expected_payload,
            did_document=did_document,
            config=AUTH_CONFIG,
        )
        outcome = issue_capability_token(
            config=AUTH_CONFIG,
            user_did=request.user_did,
            agent_did=request.agent_did,
            skill_id=request.skill_id,
            session_id=request.session_id,
            scopes=list(COFFEE_DEMO_SCOPES),
        )
    except AuthFailure as error:
        record_audit("auth.login", "denied", **audit_scope, signedChallenge=request.signed_challenge)
        raise_auth(error)

    challenges.pop(request.challenge_id, None)
    record_audit(
        "auth.login",
        "ok",
        **audit_scope,
        signerDid=verified.signer_did,
        keyId=verified.key_id,
        authScheme=verified.auth_scheme,
        verifier=verified.verifier,
        capabilityToken=outcome.token,
    )
    return {
        "capabilityToken": outcome.token,
        "expiresAtMs": outcome.expires_at_ms,
        "scopes": outcome.claims["scopes"],
        "merchantDid": AUTH_CONFIG.merchant_did,
        "userDid": request.user_did,
        "agentDid": request.agent_did,
        "skillId": request.skill_id,
        "sessionId": request.session_id,
    }


@app.post("/api/login")
def wx_login(request: WxLoginRequest, authorization: str | None = Header(default=None)) -> dict[str, Any]:
    claims = authorize(authorization, required_scope=None)
    record_audit(
        "api.login",
        "ok",
        sessionId=claims["sessionId"],
        skillId=claims["skillId"],
        userDid=claims["userDid"],
        authorization=authorization,
    )
    return {
        "loginStatus": "ok",
        "tokenReceived": True,
        "capabilityToken": redacted(authorization),
        "merchantDid": claims["merchantDid"],
        "userDid": claims["userDid"],
        "agentDid": claims.get("agentDid"),
        "skillId": claims["skillId"],
        "sessionId": claims["sessionId"],
        "scopes": claims["scopes"],
        "codeAccepted": bool((request.code or "").strip()),
    }


@app.get("/api/drinks")
def search_drinks(query: str | None = Query(default=None), authorization: str | None = Header(default=None)) -> dict[str, Any]:
    claims = authorize(authorization, SCOPE_DRINKS_READ)
    needle = (query or "").lower()
    drinks = [drink for drink in DRINKS if not needle or needle in drink["id"] or needle in drink["name"].lower()]
    record_audit("api.drinks", "ok", sessionId=claims["sessionId"], skillId=claims["skillId"], userDid=claims["userDid"])
    return {"drinks": drinks}


@app.post("/api/order/confirm")
def confirm_order(request: ConfirmOrderRequest, authorization: str | None = Header(default=None)) -> dict[str, Any]:
    claims = authorize(authorization, SCOPE_ORDER_CONFIRM)
    drink = next((item for item in DRINKS if item["id"] == request.drink_id), None)
    if not drink:
        raise HTTPException(status_code=404, detail={"code": "unknown_drink"})
    order_id = f"order_demo_{len(orders) + 1:03d}"
    order = {
        "orderId": order_id,
        "drinkId": drink["id"],
        "drinkName": drink["name"],
        "payable": drink["price"],
        "status": "pending_payment",
    }
    orders[order_id] = order
    record_audit(
        "api.order.confirm",
        "ok",
        sessionId=claims["sessionId"],
        skillId=claims["skillId"],
        userDid=claims["userDid"],
        orderId=order_id,
    )
    return order


@app.post("/api/order/pay")
def pay_order(request: PayOrderRequest, authorization: str | None = Header(default=None)) -> dict[str, Any]:
    claims = authorize(authorization, SCOPE_ORDER_PAY)
    order = orders.get(request.order_id)
    if not order:
        raise HTTPException(status_code=404, detail={"code": "unknown_order"})
    order["status"] = "paid"
    record_audit(
        "api.order.pay",
        "ok",
        sessionId=claims["sessionId"],
        skillId=claims["skillId"],
        userDid=claims["userDid"],
        orderId=request.order_id,
    )
    return order


@app.get("/api/order/{order_id}")
def get_order(order_id: str, authorization: str | None = Header(default=None)) -> dict[str, Any]:
    claims = authorize(authorization, SCOPE_ORDER_READ)
    order = orders.get(order_id)
    if not order:
        raise HTTPException(status_code=404, detail={"code": "unknown_order"})
    record_audit("api.order.read", "ok", sessionId=claims["sessionId"], skillId=claims["skillId"], userDid=claims["userDid"], orderId=order_id)
    return order


@app.get("/audit")
def get_audit_records() -> dict[str, Any]:
    return {"records": audit_records}
