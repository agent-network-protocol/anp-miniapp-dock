use crate::did::{DidCredentialError, DidCredentialProvider, IdentitySession};
use crate::token::{
    bearer_token_expiry_ms, CapabilityToken, CapabilityTokenCache, CapabilityTokenScope,
};
use anp::authentication::{AuthMode, DIDWbaAuthHeader};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;
use wx_compat::{
    Capability, CapabilityProfile, PermissionDecision, RequestBroker, WxMethod, WxRequest,
    WxRequestError, WxResponse,
};

const ANY_SCOPE: &str = "*";

#[derive(Debug, Clone)]
pub struct SignedRequestPolicy {
    rules: Vec<NetworkAllowlistRule>,
    auth_mode: AuthMode,
}

impl SignedRequestPolicy {
    pub fn new(allowlist: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            rules: allowlist
                .into_iter()
                .map(|host| NetworkAllowlistRule::host(host.into()))
                .collect(),
            auth_mode: AuthMode::HttpSignatures,
        }
    }

    pub fn with_rule(mut self, rule: NetworkAllowlistRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.auth_mode = auth_mode;
        self
    }

    pub fn auth_mode(&self) -> AuthMode {
        self.auth_mode
    }

    pub fn allows(&self, url: &str) -> bool {
        let request = WxRequest::get(url);
        self.allows_request(&request, &self.default_scope_name())
            .is_ok()
    }

    pub fn allows_request(
        &self,
        request: &WxRequest,
        scope: &str,
    ) -> Result<(), NetworkAllowlistDenial> {
        let parsed =
            ParsedUrl::parse(&request.url).map_err(|reason| NetworkAllowlistDenial { reason })?;
        if self
            .rules
            .iter()
            .any(|rule| rule.matches(&parsed, request.method, scope))
        {
            return Ok(());
        }
        Err(NetworkAllowlistDenial {
            reason: format!(
                "request URL is not in allowlist: {}",
                redact_for_log(&request.url)
            ),
        })
    }

    fn default_scope_name(&self) -> String {
        ANY_SCOPE.to_owned()
    }
}

impl Default for SignedRequestPolicy {
    fn default() -> Self {
        Self::new(std::iter::empty::<String>())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkAllowlistRule {
    scheme: Option<String>,
    host: String,
    port: Option<u16>,
    path_prefix: String,
    methods: BTreeSet<WxMethod>,
    scopes: BTreeSet<String>,
}

impl NetworkAllowlistRule {
    pub fn host(host: impl Into<String>) -> Self {
        Self {
            scheme: None,
            host: normalize_host(host.into()),
            port: None,
            path_prefix: "/".to_owned(),
            methods: BTreeSet::new(),
            scopes: BTreeSet::from([ANY_SCOPE.to_owned()]),
        }
    }

    pub fn new(scheme: impl Into<String>, host: impl Into<String>) -> Self {
        Self::host(host).with_scheme(scheme)
    }

    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = Some(scheme.into().to_ascii_lowercase());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_path_prefix(mut self, path_prefix: impl Into<String>) -> Self {
        let path_prefix = path_prefix.into();
        self.path_prefix = if path_prefix.starts_with('/') {
            path_prefix
        } else {
            format!("/{path_prefix}")
        };
        self
    }

    pub fn with_methods(mut self, methods: impl IntoIterator<Item = WxMethod>) -> Self {
        self.methods = methods.into_iter().collect();
        self
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes = BTreeSet::from([scope.into()]);
        self
    }

    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    fn matches(&self, parsed: &ParsedUrl, method: WxMethod, scope: &str) -> bool {
        self.scheme
            .as_deref()
            .is_none_or(|scheme| scheme == parsed.scheme)
            && self.host == parsed.host
            && self.port.is_none_or(|port| Some(port) == parsed.port)
            && path_has_prefix(&parsed.path, &self.path_prefix)
            && (self.methods.is_empty() || self.methods.contains(&method))
            && (scope == ANY_SCOPE
                || self.scopes.contains(ANY_SCOPE)
                || self.scopes.contains(scope))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkAllowlistDenial {
    reason: String,
}

impl NetworkAllowlistDenial {
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for NetworkAllowlistDenial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
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
        self.ensure_allowed(request)?;
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
        self.ensure_allowed(&request)?;
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

    fn ensure_allowed(&self, request: &WxRequest) -> Result<(), AnpRequestError> {
        self.policy
            .allows_request(request, &self.network_scope_name())
            .map_err(|denial| AnpRequestError::Denied(denial.to_string()))
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

    fn network_scope_name(&self) -> String {
        self.session.skill_id.clone()
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
            PermissionDecision::Allow => {
                self.client.request(request).map_err(|error| match error {
                    AnpRequestError::Denied(_)
                    | AnpRequestError::Credential(_)
                    | AnpRequestError::Authentication(_)
                    | AnpRequestError::Unauthorized(_) => {
                        WxRequestError::Denied(error.safe_message())
                    }
                    AnpRequestError::Transport(_) | AnpRequestError::Serialization(_) => {
                        WxRequestError::Transport(error.safe_message())
                    }
                })
            }
            PermissionDecision::MockAllowed { reason, .. } => Err(WxRequestError::Unsupported(
                format!("mock request permission cannot use real ANP transport: {reason}"),
            )),
            PermissionDecision::Deny { reason, .. } | PermissionDecision::Prompt { reason, .. } => {
                Err(WxRequestError::Denied(reason))
            }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUrl {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

impl ParsedUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| "request URL must include a scheme".to_owned())?;
        let scheme = scheme.trim().to_ascii_lowercase();
        if scheme.is_empty() {
            return Err("request URL scheme is empty".to_owned());
        }

        let (authority, path_with_suffix) = match rest.find(['/', '?', '#']) {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        if authority.trim().is_empty() {
            return Err("request URL host is empty".to_owned());
        }

        let (host, port) = parse_authority(authority)?;
        let path = if path_with_suffix.is_empty() || path_with_suffix.starts_with(['?', '#']) {
            "/".to_owned()
        } else {
            path_with_suffix
                .split(['?', '#'])
                .next()
                .unwrap_or("/")
                .to_owned()
        };

        Ok(Self {
            scheme,
            host,
            port,
            path,
        })
    }
}

fn parse_authority(authority: &str) -> Result<(String, Option<u16>), String> {
    let without_userinfo = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    if without_userinfo.starts_with('[') {
        let Some(end) = without_userinfo.find(']') else {
            return Err("request URL IPv6 host is invalid".to_owned());
        };
        let host = normalize_host(&without_userinfo[..=end]);
        let port = without_userinfo[end + 1..]
            .strip_prefix(':')
            .map(parse_port)
            .transpose()?;
        return Ok((host, port));
    }

    let (host, port) = match without_userinfo.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) => {
            (host, Some(parse_port(port)?))
        }
        _ => (without_userinfo, None),
    };
    let host = normalize_host(host);
    if host.is_empty() {
        return Err("request URL host is empty".to_owned());
    }
    Ok((host, port))
}

fn parse_port(port: &str) -> Result<u16, String> {
    port.parse::<u16>()
        .map_err(|_| "request URL port is invalid".to_owned())
}

fn normalize_host(host: impl AsRef<str>) -> String {
    host.as_ref()
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    let normalized_path = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    let normalized_prefix = if prefix.starts_with('/') {
        prefix.to_owned()
    } else {
        format!("/{prefix}")
    };
    if normalized_prefix == "/" {
        return true;
    }
    normalized_path == normalized_prefix
        || normalized_path
            .strip_prefix(&normalized_prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}
