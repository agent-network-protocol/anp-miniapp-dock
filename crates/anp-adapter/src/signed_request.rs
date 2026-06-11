use crate::did::{DidCredentialError, DidCredentialProvider, IdentitySession};
use crate::token::{
    bearer_token_expiry_ms, CapabilityToken, CapabilityTokenCache, CapabilityTokenScope,
};
use anp::authentication::{AuthMode, DIDWbaAuthHeader};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use wx_compat::{
    Capability, CapabilityProfile, PermissionDecision, RequestBroker, WxMethod, WxRequest,
    WxRequestError, WxResponse,
};

#[derive(Debug, Clone)]
pub struct SignedRequestPolicy {
    allowlist: BTreeSet<String>,
    auth_mode: AuthMode,
}

impl SignedRequestPolicy {
    pub fn new(allowlist: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowlist: allowlist.into_iter().map(Into::into).collect(),
            auth_mode: AuthMode::HttpSignatures,
        }
    }

    pub fn with_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.auth_mode = auth_mode;
        self
    }

    pub fn auth_mode(&self) -> AuthMode {
        self.auth_mode
    }

    pub fn allows(&self, url: &str) -> bool {
        let Some(authority) = authority(url) else {
            return false;
        };
        self.allowlist.contains(&authority)
    }
}

impl Default for SignedRequestPolicy {
    fn default() -> Self {
        Self::new(std::iter::empty::<String>())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMaterial {
    pub headers: BTreeMap<String, String>,
    pub used_cached_token: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportRequest {
    pub method: WxMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportResponse {
    pub status_code: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
}

impl TransportResponse {
    pub fn json(status_code: u16, headers: BTreeMap<String, String>, body: Value) -> Self {
        Self {
            status_code,
            headers,
            body: Some(body),
        }
    }
}

pub trait HttpTransport: Clone {
    fn send(&self, request: TransportRequest) -> Result<TransportResponse, AnpRequestError>;
}

#[derive(Debug, Clone, Default)]
pub struct ReqwestHttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport for ReqwestHttpTransport {
    fn send(&self, request: TransportRequest) -> Result<TransportResponse, AnpRequestError> {
        let method = reqwest_method(request.method);
        let mut builder = self.client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = builder
            .send()
            .map_err(|error| AnpRequestError::Transport(error.to_string()))?;
        let status_code = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        let body = response.json::<Value>().ok();

        Ok(TransportResponse {
            status_code,
            headers,
            body,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AnpHttpClient<P, C, T> {
    session: IdentitySession,
    credential_provider: P,
    token_cache: C,
    policy: SignedRequestPolicy,
    transport: T,
}

impl<P, C, T> AnpHttpClient<P, C, T>
where
    P: DidCredentialProvider,
    C: CapabilityTokenCache,
    T: HttpTransport,
{
    pub fn new(
        session: IdentitySession,
        credential_provider: P,
        token_cache: C,
        policy: SignedRequestPolicy,
        transport: T,
    ) -> Self {
        Self {
            session,
            credential_provider,
            token_cache,
            policy,
            transport,
        }
    }

    pub fn auth_material_for(
        &self,
        request: &WxRequest,
        force_signature: bool,
    ) -> Result<AuthMaterial, AnpRequestError> {
        self.ensure_allowed(&request.url)?;
        let scope = self.token_scope();
        if !force_signature {
            if let Some(token) = self.token_cache.get(&scope) {
                let mut headers = BTreeMap::new();
                headers.insert(
                    "Authorization".to_owned(),
                    format!("Bearer {}", token.value),
                );
                return Ok(AuthMaterial {
                    headers,
                    used_cached_token: true,
                });
            }
        }

        let credential = self
            .credential_provider
            .credential_for(&self.session)
            .map_err(AnpRequestError::Credential)?;
        let mut helper = DIDWbaAuthHeader::new(
            credential.did_document_path,
            credential.private_key_path,
            self.policy.auth_mode(),
        );
        let body = body_bytes(&request.data)?;
        let headers = helper
            .get_auth_header(
                &request.url,
                true,
                method_name(request.method),
                Some(&request.headers),
                body.as_deref(),
            )
            .map_err(|error| AnpRequestError::Authentication(error.to_string()))?;

        Ok(AuthMaterial {
            headers,
            used_cached_token: false,
        })
    }

    pub fn request(&self, request: WxRequest) -> Result<WxResponse, AnpRequestError> {
        self.ensure_allowed(&request.url)?;
        let body = body_bytes(&request.data)?;
        let auth = self.auth_material_for(&request, false)?;
        let mut signed = request.clone();
        signed.headers.extend(auth.headers.clone());
        let mut response = self.transport.send(TransportRequest {
            method: signed.method,
            url: signed.url.clone(),
            headers: signed.headers.clone(),
            body: body.clone(),
        })?;

        if response.status_code == 401 {
            if auth.used_cached_token {
                self.token_cache.clear(&self.token_scope());
            }
            response = self.retry_after_challenge(&request, body.as_deref(), &response.headers)?;
        }

        if let Some(token) = extract_token(&response.headers) {
            self.token_cache.put(self.token_scope(), token);
        }

        Ok(WxResponse {
            status_code: response.status_code,
            headers: response.headers,
            data: response.body.unwrap_or(Value::Null),
        })
    }

    fn retry_after_challenge(
        &self,
        request: &WxRequest,
        body: Option<&[u8]>,
        response_headers: &BTreeMap<String, String>,
    ) -> Result<TransportResponse, AnpRequestError> {
        let credential = self
            .credential_provider
            .credential_for(&self.session)
            .map_err(AnpRequestError::Credential)?;
        let mut helper = DIDWbaAuthHeader::new(
            credential.did_document_path,
            credential.private_key_path,
            self.policy.auth_mode(),
        );
        if !helper.should_retry_after_401(response_headers) {
            return Err(AnpRequestError::Unauthorized(
                "server rejected DID authentication".to_owned(),
            ));
        }

        let mut headers = request.headers.clone();
        headers.extend(
            helper
                .get_challenge_auth_header(
                    &request.url,
                    response_headers,
                    method_name(request.method),
                    Some(&request.headers),
                    body,
                )
                .map_err(|error| AnpRequestError::Authentication(error.to_string()))?,
        );

        self.transport.send(TransportRequest {
            method: request.method,
            url: request.url.clone(),
            headers,
            body: body.map(ToOwned::to_owned),
        })
    }

    fn ensure_allowed(&self, url: &str) -> Result<(), AnpRequestError> {
        if self.policy.allows(url) {
            return Ok(());
        }
        Err(AnpRequestError::Denied(format!(
            "request URL is not in allowlist: {}",
            redact_for_log(url)
        )))
    }

    fn token_scope(&self) -> CapabilityTokenScope {
        CapabilityTokenScope::for_subject(
            self.session.merchant_did.clone(),
            self.session.user_did.clone(),
            self.session.agent_did.clone(),
            self.session.skill_id.clone(),
            Some(self.session.session_id.clone()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct AnpRequestBroker<P, C, T> {
    client: AnpHttpClient<P, C, T>,
}

impl<P, C, T> AnpRequestBroker<P, C, T> {
    pub fn new(client: AnpHttpClient<P, C, T>) -> Self {
        Self { client }
    }
}

impl<P, C, T> RequestBroker for AnpRequestBroker<P, C, T>
where
    P: DidCredentialProvider,
    C: CapabilityTokenCache,
    T: HttpTransport,
{
    fn request(
        &self,
        profile: &CapabilityProfile,
        request: WxRequest,
    ) -> Result<WxResponse, WxRequestError> {
        match profile.check(Capability::Request) {
            PermissionDecision::Allow => self
                .client
                .request(request)
                .map_err(|error| WxRequestError::Denied(error.safe_message())),
            PermissionDecision::Deny { reason, .. } => Err(WxRequestError::Denied(reason)),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnpRequestError {
    #[error("request denied: {0}")]
    Denied(String),

    #[error("DID credential error: {0}")]
    Credential(DidCredentialError),

    #[error("authentication failed: {0}")]
    Authentication(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("transport failed: {0}")]
    Transport(String),

    #[error("serialization failed: {0}")]
    Serialization(String),
}

impl AnpRequestError {
    pub fn safe_message(&self) -> String {
        redact_for_log(&self.to_string())
    }
}

pub fn redact_for_log(value: &str) -> String {
    let mut redacted = value.to_owned();
    for marker in ["Authorization", "Signature", "token", "private", "secret"] {
        redacted = redact_marker(&redacted, marker);
    }
    redacted
}

fn redact_marker(value: &str, marker: &str) -> String {
    let lower_value = value.to_ascii_lowercase();
    let lower_marker = marker.to_ascii_lowercase();
    if !lower_value.contains(&lower_marker) {
        return value.to_owned();
    }
    format!("{marker}=[REDACTED]")
}

fn extract_token(headers: &BTreeMap<String, String>) -> Option<CapabilityToken> {
    header_value(headers, "Authentication-Info")
        .and_then(parse_authentication_info_token)
        .or_else(|| {
            header_value(headers, "Authorization")
                .and_then(|value| value.strip_prefix("Bearer ").map(ToOwned::to_owned))
        })
        .map(|value| {
            let expires_at_ms = bearer_token_expiry_ms(&value);
            CapabilityToken::new(value, expires_at_ms)
        })
}

fn parse_authentication_info_token(value: &str) -> Option<String> {
    value
        .split(',')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, raw)| {
            (key.trim() == "access_token")
                .then(|| raw.trim().trim_matches('"').to_owned())
                .filter(|token| !token.is_empty())
        })
}

fn header_value<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn body_bytes(data: &Option<Value>) -> Result<Option<Vec<u8>>, AnpRequestError> {
    data.as_ref()
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| AnpRequestError::Serialization(error.to_string()))
        })
        .transpose()
}

fn method_name(method: WxMethod) -> &'static str {
    match method {
        WxMethod::Get => "GET",
        WxMethod::Post => "POST",
        WxMethod::Put => "PUT",
        WxMethod::Delete => "DELETE",
        WxMethod::Patch => "PATCH",
    }
}

fn reqwest_method(method: WxMethod) -> reqwest::Method {
    match method {
        WxMethod::Get => reqwest::Method::GET,
        WxMethod::Post => reqwest::Method::POST,
        WxMethod::Put => reqwest::Method::PUT,
        WxMethod::Delete => reqwest::Method::DELETE,
        WxMethod::Patch => reqwest::Method::PATCH,
    }
}

fn authority(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    rest.split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())
        .map(ToOwned::to_owned)
}
