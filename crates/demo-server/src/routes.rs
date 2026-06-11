use crate::audit::{AuditLog, AuditRecord};
use crate::auth::{
    login_audience, AuthError, AuthStore, ChallengeLoginRequest, ChallengeRequest, ServerAuthConfig,
};
use crate::coffee::{CoffeeError, CoffeeStore, ConfirmOrderRequest, PayOrderRequest};
use anp_adapter::{CapabilityTokenClaims, ExpectedCapability};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

const AGENT_ID: &str = "coffee";
const DEFAULT_MERCHANT_DID: &str = "did:wba:coffee-merchant.example";
const DEFAULT_PUBLIC_BASE_URL: &str = "http://127.0.0.1:3000";
const SCOPE_DRINKS_READ: &str = "coffee:drinks:read";
const SCOPE_ORDER_CONFIRM: &str = "coffee:order:confirm";
const SCOPE_ORDER_PAY: &str = "coffee:order:pay";
const SCOPE_ORDER_READ: &str = "coffee:order:read";

#[derive(Debug, Clone)]
pub struct DemoState {
    inner: Arc<DemoStateInner>,
}

#[derive(Debug)]
struct DemoStateInner {
    skill_root: PathBuf,
    auth_config: ServerAuthConfig,
    auth: AuthStore,
    coffee: CoffeeStore,
    audit: AuditLog,
}

impl DemoState {
    pub fn new(skill_root: impl Into<PathBuf>) -> Self {
        Self::with_auth_config(skill_root, ServerAuthConfig::new(DEFAULT_MERCHANT_DID))
    }

    pub fn with_auth_config(skill_root: impl Into<PathBuf>, auth_config: ServerAuthConfig) -> Self {
        Self {
            inner: Arc::new(DemoStateInner {
                skill_root: skill_root.into(),
                auth_config,
                auth: AuthStore::default(),
                coffee: CoffeeStore::default(),
                audit: AuditLog::default(),
            }),
        }
    }

    pub fn merchant_did(&self) -> &str {
        &self.inner.auth_config.merchant_did
    }

    pub fn auth_config(&self) -> &ServerAuthConfig {
        &self.inner.auth_config
    }

    pub fn audit_records(&self) -> Vec<crate::audit::AuditRecord> {
        self.inner.audit.records()
    }

    pub fn public_base_url(&self) -> &str {
        DEFAULT_PUBLIC_BASE_URL
    }
}

pub fn app(state: DemoState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/registry/agents", get(registry_agents))
        .route("/agents/coffee/manifest", get(agent_manifest))
        .route("/agents/coffee/SKILL.md", get(skill_markdown))
        .route("/agents/coffee/mcp.json", get(mcp_json))
        .route("/agents/coffee/package.zip", get(package_zip_noop))
        .route("/agents/coffee/auth/challenge", post(auth_challenge))
        .route("/agents/coffee/auth/login", post(auth_login))
        .route("/api/drinks", get(search_drinks))
        .route("/api/order/confirm", post(confirm_order))
        .route("/api/order/pay", post(pay_order))
        .route("/api/order/:order_id", get(get_order))
        .route("/audit", get(audit_records))
        .with_state(state)
}

pub async fn serve(state: DemoState, addr: SocketAddr) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(state)).await
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "demo-server"}))
}

async fn registry_agents(State(state): State<DemoState>) -> Json<Value> {
    Json(json!({
        "agents": [{
            "id": AGENT_ID,
            "name": "Coffee Merchant Agent",
            "did": state.inner.auth_config.merchant_did,
            "manifestUrl": "/agents/coffee/manifest",
            "skillUrl": "/agents/coffee/mcp.json"
        }]
    }))
}

async fn agent_manifest(State(state): State<DemoState>) -> Json<Value> {
    Json(json!({
        "id": AGENT_ID,
        "name": "Coffee Merchant Agent",
        "did": state.inner.auth_config.merchant_did,
        "skill": {
            "skillMd": "/agents/coffee/SKILL.md",
            "mcpJson": "/agents/coffee/mcp.json",
            "package": "/agents/coffee/package.zip"
        },
        "auth": {
            "challenge": "/agents/coffee/auth/challenge",
            "login": "/agents/coffee/auth/login"
        },
        "apis": {
            "drinks": "/api/drinks",
            "confirmOrder": "/api/order/confirm",
            "payOrder": "/api/order/pay"
        }
    }))
}

async fn skill_markdown(State(state): State<DemoState>) -> Result<Response, ApiError> {
    let markdown = fs::read_to_string(state.inner.skill_root.join("SKILL.md"))
        .map_err(|error| ApiError::internal(format!("failed to read SKILL.md: {error}")))?;
    Ok((
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        markdown,
    )
        .into_response())
}

async fn mcp_json(State(state): State<DemoState>) -> Result<Json<Value>, ApiError> {
    let payload = fs::read_to_string(state.inner.skill_root.join("mcp.json"))
        .map_err(|error| ApiError::internal(format!("failed to read mcp.json: {error}")))?;
    let value = serde_json::from_str(&payload)
        .map_err(|error| ApiError::internal(format!("invalid mcp.json fixture: {error}")))?;
    Ok(Json(value))
}

async fn package_zip_noop() -> Json<Value> {
    Json(json!({
        "status": "p0_noop",
        "reason": "package.zip is not generated in the Rust MVP demo server"
    }))
}

async fn auth_challenge(
    State(state): State<DemoState>,
    Json(request): Json<ChallengeRequest>,
) -> Json<Value> {
    let challenge = state.inner.auth.challenge(
        &state.inner.auth_config.merchant_did,
        &login_audience(state.public_base_url()),
        state.inner.auth_config.challenge_ttl_ms,
        request.clone(),
    );
    state
        .inner
        .audit
        .record(AuditRecord::new("auth.challenge", "ok").with_scope(
            Some(request.session_id),
            Some(request.skill_id),
            Some(request.user_did),
        ));
    Json(serde_json::to_value(challenge).expect("challenge serializes"))
}

async fn auth_login(
    State(state): State<DemoState>,
    Json(request): Json<ChallengeLoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let audit_scope = (
        Some(request.session_id.clone()),
        Some(request.skill_id.clone()),
        Some(request.user_did.clone()),
    );
    match state.inner.auth.login(&state.inner.auth_config, request) {
        Ok(response) => {
            state
                .inner
                .audit
                .record(AuditRecord::new("auth.login", "ok").with_scope(
                    audit_scope.0,
                    audit_scope.1,
                    audit_scope.2,
                ));
            Ok(Json(
                serde_json::to_value(response).expect("login serializes"),
            ))
        }
        Err(error) => {
            state
                .inner
                .audit
                .record(AuditRecord::new("auth.login", "denied").with_scope(
                    audit_scope.0,
                    audit_scope.1,
                    audit_scope.2,
                ));
            Err(ApiError::from_auth(error))
        }
    }
}

#[derive(Debug, Deserialize)]
struct DrinksQuery {
    query: Option<String>,
}

async fn search_drinks(
    State(state): State<DemoState>,
    headers: HeaderMap,
    Query(query): Query<DrinksQuery>,
) -> Result<Json<Value>, ApiError> {
    let claims = authorize(&state, &headers, SCOPE_DRINKS_READ)?;
    let response = state.inner.coffee.search_drinks(query.query.as_deref());
    state
        .inner
        .audit
        .record(AuditRecord::new("api.drinks", "ok").with_scope(
            Some(claims.session_id),
            Some(claims.skill_id),
            Some(claims.user_did),
        ));
    Ok(Json(
        serde_json::to_value(response).expect("drinks serialize"),
    ))
}

async fn confirm_order(
    State(state): State<DemoState>,
    headers: HeaderMap,
    Json(parameters): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let claims = authorize(&state, &headers, SCOPE_ORDER_CONFIRM)?;
    let request =
        serde_json::from_value::<ConfirmOrderRequest>(parameters.clone()).map_err(|error| {
            ApiError::bad_request(format!("invalid confirm order request: {error}"))
        })?;
    match state.inner.coffee.confirm_order(request) {
        Ok(order) => {
            state.inner.audit.record(
                AuditRecord::new("api.order.confirm", "ok")
                    .with_scope(
                        Some(claims.session_id),
                        Some(claims.skill_id),
                        Some(claims.user_did),
                    )
                    .with_order(order.order_id.clone())
                    .with_parameters(&parameters),
            );
            Ok(Json(serde_json::to_value(order).expect("order serializes")))
        }
        Err(error) => Err(ApiError::from_coffee(error)),
    }
}

async fn pay_order(
    State(state): State<DemoState>,
    headers: HeaderMap,
    Json(parameters): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let claims = authorize(&state, &headers, SCOPE_ORDER_PAY)?;
    let request = serde_json::from_value::<PayOrderRequest>(parameters.clone())
        .map_err(|error| ApiError::bad_request(format!("invalid pay order request: {error}")))?;
    match state.inner.coffee.pay_order(request) {
        Ok(order) => {
            state.inner.audit.record(
                AuditRecord::new("api.order.pay", "ok")
                    .with_scope(
                        Some(claims.session_id),
                        Some(claims.skill_id),
                        Some(claims.user_did),
                    )
                    .with_order(order.order_id.clone())
                    .with_parameters(&parameters),
            );
            Ok(Json(serde_json::to_value(order).expect("order serializes")))
        }
        Err(error) => Err(ApiError::from_coffee(error)),
    }
}

async fn get_order(
    State(state): State<DemoState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let claims = authorize(&state, &headers, SCOPE_ORDER_READ)?;
    let order = state
        .inner
        .coffee
        .get_order(&order_id)
        .map_err(ApiError::from_coffee)?;
    state
        .inner
        .audit
        .record(AuditRecord::new("api.order.read", "ok").with_scope(
            Some(claims.session_id),
            Some(claims.skill_id),
            Some(claims.user_did),
        ));
    Ok(Json(serde_json::to_value(order).expect("order serializes")))
}

async fn audit_records(State(state): State<DemoState>) -> Json<Value> {
    Json(json!({ "records": state.inner.audit.records() }))
}

fn authorize(
    state: &DemoState,
    headers: &HeaderMap,
    required_scope: &str,
) -> Result<CapabilityTokenClaims, ApiError> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let expected = ExpectedCapability::for_route(
        state.inner.auth_config.merchant_did.clone(),
        state.inner.auth_config.merchant_did.clone(),
        state.inner.auth_config.merchant_did.clone(),
        AGENT_ID,
        required_scope,
    );
    state
        .inner
        .auth
        .verify_bearer(&state.inner.auth_config, authorization, expected)
        .map_err(ApiError::from_auth)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message,
        }
    }

    fn internal(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message,
        }
    }

    fn from_auth(error: AuthError) -> Self {
        let (status, code, message) = match error {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "missing_token",
                "missing Bearer capability token",
            ),
            AuthError::InvalidToken => (
                StatusCode::FORBIDDEN,
                "invalid_token",
                "capability token is invalid",
            ),
            AuthError::ExpiredToken => (
                StatusCode::FORBIDDEN,
                "expired_token",
                "capability token is expired",
            ),
            AuthError::InsufficientScope => (
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "capability token does not include the required scope",
            ),
            AuthError::UnknownChallenge => (
                StatusCode::UNAUTHORIZED,
                "unknown_challenge",
                "challenge is unknown or already used",
            ),
            AuthError::ExpiredChallenge => (
                StatusCode::UNAUTHORIZED,
                "expired_challenge",
                "challenge is expired",
            ),
            AuthError::InvalidSignature => (
                StatusCode::UNAUTHORIZED,
                "invalid_signature",
                "signedChallenge signature is invalid",
            ),
            AuthError::UnknownDid => (
                StatusCode::UNAUTHORIZED,
                "unknown_did",
                "DID document is unknown or invalid",
            ),
            AuthError::ScopeMismatch => (
                StatusCode::UNAUTHORIZED,
                "scope_mismatch",
                "auth scope does not match the issued challenge or token",
            ),
            AuthError::TokenIssuerUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "token_issuer_unavailable",
                "token issuer is not configured",
            ),
            AuthError::Unavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth_unavailable",
                "auth store is unavailable",
            ),
        };
        Self {
            status,
            code,
            message: message.to_owned(),
        }
    }

    fn from_coffee(error: CoffeeError) -> Self {
        let (status, code, message) = match error {
            CoffeeError::UnknownDrink => (
                StatusCode::NOT_FOUND,
                "unknown_drink",
                "drinkId does not match a demo drink",
            ),
            CoffeeError::UnknownOrder => (
                StatusCode::NOT_FOUND,
                "unknown_order",
                "orderId does not match a demo order",
            ),
            CoffeeError::Unavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "coffee_store_unavailable",
                "coffee store is unavailable",
            ),
        };
        Self {
            status,
            code,
            message: message.to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                code: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_uses_demo_merchant_did() {
        let state = DemoState::new("../../examples/coffee-skill");

        assert_eq!(state.merchant_did(), DEFAULT_MERCHANT_DID);
    }
}
