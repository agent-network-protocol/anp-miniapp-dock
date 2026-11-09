use crate::error::DockCoreError;
use crate::error::ErrorCode;
use crate::orchestrator::{ApiCallContext, ComponentRenderInput};
use consent_audit::RiskLevel;
use consent_audit::{
    ConsentError, ConsentProof, ConsentRequest, ConsentStatus, HostConsentAdapter,
};
use mcp_schema::AtomicApiResult;
use mcp_schema::TextContent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const HOST_ADAPTER_CONTRACT_VERSION: &str = "dock.host-adapter.v1";
pub const HOST_ACTION_REDACTION_POLICY: &str = "dock.host-action.redaction.v1";

pub trait RuntimeHost {
    fn check_permission(
        &self,
        context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError>;

    fn adapter_contract(&self) -> HostAdapterContract {
        HostAdapterContract::unknown()
    }

    fn handle_host_action(
        &self,
        _context: &ApiCallContext,
        request: HostActionRequest,
    ) -> Result<HostActionOutcome, DockCoreError> {
        Ok(HostActionOutcome::unsupported(
            request.action_type,
            "host_action_not_supported",
        ))
    }
}

pub trait ConsentGate {
    fn check_consent(
        &self,
        context: &ApiCallContext,
        request: &ConsentRequest,
    ) -> Result<ConsentDecision, DockCoreError>;
}

pub trait ApiExecutor {
    fn execute(
        &self,
        context: &ApiCallContext,
        component_path: Option<&str>,
    ) -> Result<AtomicApiResult, DockCoreError>;
}

pub trait RenderRouter {
    fn render(
        &self,
        context: &ApiCallContext,
        input: &ComponentRenderInput,
    ) -> Result<RenderOutcome, DockCoreError>;

    fn fallback(
        &self,
        context: &ApiCallContext,
        result: &AtomicApiResult,
        reason: &str,
    ) -> RenderOutcome;
}

pub trait AuditSink {
    fn ensure_available(&self) -> Result<(), DockCoreError> {
        Ok(())
    }

    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError>;
}

#[derive(Debug, Clone)]
pub struct HostConsentGateAdapter<A> {
    adapter: A,
}

impl<A> HostConsentGateAdapter<A> {
    pub fn new(adapter: A) -> Self {
        Self { adapter }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }
}

impl<A> ConsentGate for HostConsentGateAdapter<A>
where
    A: HostConsentAdapter,
{
    fn check_consent(
        &self,
        _context: &ApiCallContext,
        request: &ConsentRequest,
    ) -> Result<ConsentDecision, DockCoreError> {
        match self.adapter.request_host_consent(request) {
            Ok(decision) if decision.status == ConsentStatus::Approved => Ok(
                ConsentDecision::approved(decision.provider, decision.decision_actor),
            ),
            Ok(decision) => Ok(ConsentDecision::Required(format!(
                "consent denied by {}",
                decision.provider
            ))),
            Err(ConsentError::Denied { .. }) => {
                Ok(ConsentDecision::Required("consent denied".to_owned()))
            }
            Err(ConsentError::ProviderUnavailable { .. }) => Err(DockCoreError::core(
                ErrorCode::ConsentRequired,
                "host consent provider unavailable",
            )),
            Err(ConsentError::Provider(_)) => Err(DockCoreError::core(
                ErrorCode::ConsentRequired,
                "host consent provider failed",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny { reason: String, reason_code: String },
    Prompt { reason: String, reason_code: String },
    MockAllowedDevOnly { reason: String, reason_code: String },
}

impl PermissionDecision {
    pub fn deny(reason: impl Into<String>, reason_code: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
            reason_code: reason_code.into(),
        }
    }

    pub fn prompt(reason: impl Into<String>, reason_code: impl Into<String>) -> Self {
        Self::Prompt {
            reason: reason.into(),
            reason_code: reason_code.into(),
        }
    }

    pub fn mock_allowed_dev_only(
        reason: impl Into<String>,
        reason_code: impl Into<String>,
    ) -> Self {
        Self::MockAllowedDevOnly {
            reason: reason.into(),
            reason_code: reason_code.into(),
        }
    }

    pub fn summary(&self) -> PermissionDecisionSummary {
        match self {
            Self::Allow => PermissionDecisionSummary {
                decision: "allow".to_owned(),
                reason_code: "host_allow".to_owned(),
                reason: "Host allowed this API call".to_owned(),
                dev_only: false,
            },
            Self::Deny {
                reason,
                reason_code,
            } => PermissionDecisionSummary {
                decision: "deny".to_owned(),
                reason_code: reason_code.clone(),
                reason: redact_permission_reason(reason),
                dev_only: false,
            },
            Self::Prompt {
                reason,
                reason_code,
            } => PermissionDecisionSummary {
                decision: "prompt".to_owned(),
                reason_code: reason_code.clone(),
                reason: redact_permission_reason(reason),
                dev_only: false,
            },
            Self::MockAllowedDevOnly {
                reason,
                reason_code,
            } => PermissionDecisionSummary {
                decision: "mock_allowed".to_owned(),
                reason_code: reason_code.clone(),
                reason: redact_permission_reason(reason),
                dev_only: true,
            },
        }
    }
}

fn redact_permission_reason(reason: &str) -> String {
    let lower = reason.to_ascii_lowercase();
    for marker in [
        "authorization",
        "signature",
        "token",
        "secret",
        "private",
        "password",
        "cookie",
        "credential",
    ] {
        if lower.contains(marker) {
            return format!("{marker}=[REDACTED]");
        }
    }
    reason.to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecisionSummary {
    pub decision: String,
    pub reason_code: String,
    pub reason: String,
    pub dev_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentDecision {
    Approved {
        provider: String,
        decision_actor: String,
    },
    Required(String),
}

impl ConsentDecision {
    pub fn approved(provider: impl Into<String>, decision_actor: impl Into<String>) -> Self {
        Self::Approved {
            provider: provider.into(),
            decision_actor: decision_actor.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutcome {
    pub renderer: String,
    pub component_path: Option<String>,
    pub payload: Value,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditEvent {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub session_id: String,
    pub skill_id: String,
    pub api_name: String,
    pub risk_level: RiskLevel,
    pub parameter_summary: Value,
    pub permission_decision: PermissionDecisionSummary,
    pub consent_proof: Option<ConsentProof>,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAdapterContract {
    pub version: String,
    pub adapter_name: String,
    pub production_ready: bool,
    pub capabilities: Vec<HostCapabilityDeclaration>,
    pub redaction: HostActionRedaction,
}

impl HostAdapterContract {
    pub fn unknown() -> Self {
        Self {
            version: HOST_ADAPTER_CONTRACT_VERSION.to_owned(),
            adapter_name: "custom-runtime-host".to_owned(),
            production_ready: false,
            capabilities: host_adapter_contract_v1()
                .into_iter()
                .map(|mut capability| {
                    capability.status = match capability.requirement {
                        HostCapabilityRequirement::UnsupportedByDesign => {
                            HostCapabilityStatus::UnsupportedByDesign
                        }
                        HostCapabilityRequirement::Required
                        | HostCapabilityRequirement::Optional => HostCapabilityStatus::Unsupported,
                    };
                    capability.reason = "custom Host must declare support explicitly".to_owned();
                    capability
                })
                .collect(),
            redaction: HostActionRedaction::default(),
        }
    }

    pub fn headless_mock() -> Self {
        Self {
            version: HOST_ADAPTER_CONTRACT_VERSION.to_owned(),
            adapter_name: "headless-mock-host".to_owned(),
            production_ready: false,
            capabilities: host_adapter_contract_v1()
                .into_iter()
                .map(|mut capability| {
                    capability.status = match capability.name.as_str() {
                        "renderIrRenderer"
                        | "cardSpecFallbackRenderer"
                        | "consentPrompt"
                        | "eventDispatch"
                        | "openDetailPage" => HostCapabilityStatus::DevOnly,
                        "fullMiniappPageRouting" => HostCapabilityStatus::UnsupportedByDesign,
                        _ => HostCapabilityStatus::Unsupported,
                    };
                    capability.reason = match capability.status {
                        HostCapabilityStatus::Supported => "supported".to_owned(),
                        HostCapabilityStatus::DevOnly => {
                            "headless/mock support only; not production-ready".to_owned()
                        }
                        HostCapabilityStatus::Unsupported => {
                            "not implemented by headless Host".to_owned()
                        }
                        HostCapabilityStatus::UnsupportedByDesign => {
                            "outside Agentic MiniApp Container boundary".to_owned()
                        }
                    };
                    capability
                })
                .collect(),
            redaction: HostActionRedaction::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCapabilityDeclaration {
    pub name: String,
    pub requirement: HostCapabilityRequirement,
    pub status: HostCapabilityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCapabilityRequirement {
    Required,
    Optional,
    UnsupportedByDesign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCapabilityStatus {
    Supported,
    DevOnly,
    Unsupported,
    UnsupportedByDesign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostActionRequest {
    pub action_type: String,
    pub source_api_name: String,
    pub payload: Value,
    pub route: HostActionRoute,
}

impl HostActionRequest {
    pub fn send_follow_up_message(
        source_api_name: impl Into<String>,
        content: &[TextContent],
    ) -> Self {
        Self {
            action_type: "sendFollowUpMessage".to_owned(),
            source_api_name: source_api_name.into(),
            payload: redact_host_payload(&json!({
                "content": content,
                "contentCount": content.len()
            })),
            route: HostActionRoute::HostAdapter,
        }
    }

    pub fn open_detail_page(source_api_name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            action_type: "openDetailPage".to_owned(),
            source_api_name: source_api_name.into(),
            payload: json!({ "url": url.into() }),
            route: HostActionRoute::HostAdapter,
        }
    }

    pub fn expire_previous_cards(
        source_api_name: impl Into<String>,
        component_paths: Vec<String>,
        match_policy: Option<String>,
    ) -> Self {
        Self {
            action_type: "expirePreviousCards".to_owned(),
            source_api_name: source_api_name.into(),
            payload: json!({
                "componentPaths": component_paths,
                "match": match_policy,
            }),
            route: HostActionRoute::HostAdapter,
        }
    }

    pub fn unsupported(
        source_api_name: impl Into<String>,
        action_type: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            action_type: action_type.into(),
            source_api_name: source_api_name.into(),
            payload: redact_host_payload(&payload),
            route: HostActionRoute::Unsupported,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostActionRoute {
    RuntimeOrchestrator,
    HostAdapter,
    Unsupported,
}

impl HostActionRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeOrchestrator => "runtime-orchestrator",
            Self::HostAdapter => "host-adapter",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostActionOutcome {
    pub action_type: String,
    pub status: HostActionStatus,
    pub boundary: String,
    pub reason_code: String,
    pub payload: Value,
    pub redaction: HostActionRedaction,
}

impl HostActionOutcome {
    pub fn accepted(action_type: impl Into<String>, payload: Value) -> Self {
        Self {
            action_type: action_type.into(),
            status: HostActionStatus::Accepted,
            boundary: "host-adapter".to_owned(),
            reason_code: "accepted".to_owned(),
            payload: redact_host_payload(&payload),
            redaction: HostActionRedaction::default(),
        }
    }

    pub fn unsupported(action_type: impl Into<String>, reason_code: impl Into<String>) -> Self {
        Self {
            action_type: action_type.into(),
            status: HostActionStatus::Unsupported,
            boundary: "host-adapter".to_owned(),
            reason_code: reason_code.into(),
            payload: json!({}),
            redaction: HostActionRedaction::default(),
        }
    }

    pub fn redacted(mut self) -> Self {
        self.payload = redact_host_payload(&self.payload);
        self.redaction = HostActionRedaction::default();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostActionStatus {
    Accepted,
    Unsupported,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostActionRedaction {
    pub marker: String,
    pub policy: String,
    pub applied_by_default: bool,
}

impl Default for HostActionRedaction {
    fn default() -> Self {
        Self {
            marker: "[REDACTED]".to_owned(),
            policy: HOST_ACTION_REDACTION_POLICY.to_owned(),
            applied_by_default: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeadlessHostAdapter;

impl RuntimeHost for HeadlessHostAdapter {
    fn check_permission(
        &self,
        _context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError> {
        Ok(PermissionDecision::Allow)
    }

    fn adapter_contract(&self) -> HostAdapterContract {
        HostAdapterContract::headless_mock()
    }

    fn handle_host_action(
        &self,
        _context: &ApiCallContext,
        request: HostActionRequest,
    ) -> Result<HostActionOutcome, DockCoreError> {
        match request.action_type.as_str() {
            "sendFollowUpMessage" => Ok(HostActionOutcome::accepted(
                request.action_type,
                json!({
                    "delivery": "agent-message-boundary",
                    "contentCount": request.payload.get("contentCount").cloned().unwrap_or(Value::Null),
                }),
            )),
            "expirePreviousCards" => Ok(HostActionOutcome::accepted(
                request.action_type,
                json!({
                    "boundary": "host-managed-card-store",
                    "accepted": true,
                }),
            )),
            "openDetailPage" => {
                let url = request
                    .payload
                    .get("url")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        DockCoreError::core(
                            ErrorCode::PermissionDenied,
                            "openDetailPage target is invalid",
                        )
                    })?;
                let canonical_url = canonicalize_open_detail_page_target(url)?;
                Ok(HostActionOutcome::accepted(
                    request.action_type,
                    json!({ "canonicalUrl": canonical_url }),
                ))
            }
            _ => Ok(HostActionOutcome::unsupported(
                request.action_type,
                "host_action_not_supported",
            )),
        }
    }
}

pub fn canonicalize_open_detail_page_target(target: &str) -> Result<String, DockCoreError> {
    let target = target.trim();
    let lower = target.to_ascii_lowercase();
    if target.is_empty()
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("//")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("file:")
        || lower.contains("://")
        || lower.contains("%2e")
        || lower.contains("%2f")
        || lower.contains("%5c")
        || lower.contains("\\")
        || contains_sensitive_marker(&lower)
    {
        return Err(DockCoreError::core(
            ErrorCode::PermissionDenied,
            "openDetailPage target is not allowed",
        ));
    }

    let without_fragment = target.split('#').next().unwrap_or(target);
    let (path, query) = without_fragment
        .split_once('?')
        .map_or((without_fragment, None), |(path, query)| {
            (path, Some(query))
        });
    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(DockCoreError::core(
                ErrorCode::PermissionDenied,
                "openDetailPage target is not allowed",
            ));
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(DockCoreError::core(
            ErrorCode::PermissionDenied,
            "openDetailPage target is not allowed",
        ));
    }
    let mut canonical = format!("/{}", segments.join("/"));
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        if contains_sensitive_marker(&query.to_ascii_lowercase()) {
            return Err(DockCoreError::core(
                ErrorCode::PermissionDenied,
                "openDetailPage target is not allowed",
            ));
        }
        canonical.push('?');
        canonical.push_str(query);
    }
    Ok(canonical)
}

fn host_adapter_contract_v1() -> Vec<HostCapabilityDeclaration> {
    [
        ("renderIrRenderer", HostCapabilityRequirement::Required),
        (
            "cardSpecFallbackRenderer",
            HostCapabilityRequirement::Required,
        ),
        ("consentPrompt", HostCapabilityRequirement::Required),
        ("eventDispatch", HostCapabilityRequirement::Required),
        (
            "secureIdentityProvider",
            HostCapabilityRequirement::Required,
        ),
        ("providerPayment", HostCapabilityRequirement::Optional),
        ("providerPhone", HostCapabilityRequirement::Optional),
        ("providerAddress", HostCapabilityRequirement::Optional),
        ("providerLocation", HostCapabilityRequirement::Optional),
        ("providerFile", HostCapabilityRequirement::Optional),
        ("providerMedia", HostCapabilityRequirement::Optional),
        ("openDetailPage", HostCapabilityRequirement::Optional),
        (
            "fullMiniappPageRouting",
            HostCapabilityRequirement::UnsupportedByDesign,
        ),
    ]
    .into_iter()
    .map(|(name, requirement)| HostCapabilityDeclaration {
        name: name.to_owned(),
        requirement,
        status: HostCapabilityStatus::Unsupported,
        reason: "support must be declared by Host adapter".to_owned(),
    })
    .collect()
}

fn redact_host_payload(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if contains_sensitive_marker(&lower) {
                        (key.clone(), Value::String("[REDACTED]".to_owned()))
                    } else {
                        (key.clone(), redact_host_payload(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_host_payload).collect()),
        Value::String(text) if contains_sensitive_marker(&text.to_ascii_lowercase()) => {
            Value::String("[REDACTED]".to_owned())
        }
        _ => value.clone(),
    }
}

fn contains_sensitive_marker(lower: &str) -> bool {
    [
        "authorization",
        "signature",
        "capabilitytoken",
        "capability_token",
        "token",
        "secret",
        "private",
        "password",
        "cookie",
        "credential",
        "/home/",
        "/users/",
        "\\users\\",
        "c:\\",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}
