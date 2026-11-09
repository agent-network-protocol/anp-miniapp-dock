use crate::error::{DockCoreError, ErrorCode};
use crate::host::{
    canonicalize_open_detail_page_target, ApiExecutor, AuditEvent, AuditSink, ConsentGate,
    HostActionOutcome, HostActionRequest, HostActionStatus, HostAdapterContract, RenderOutcome,
    RenderRouter,
};
use crate::observability::{
    next_observability_id, MetricsSink, NoopMetricsSink, NoopObservabilitySink, ObservabilityEvent,
    ObservabilityEventKind, ObservabilityMetric, ObservabilitySink, TraceContext, TraceSpan,
    TraceSpanKind,
};
use crate::orchestrator::{
    ApiCallContext, CallOutcome, ComponentAction, ComponentRenderInput, Orchestrator,
};
use crate::RuntimeHost;
use consent_audit::{redact_value, AuditOutcome, AuditRecord, AuditRecordInput, RiskLevel};
use mcp_schema::{
    AtomicApiResult, ModelVisibleApiResult, TextContent, ValidationIssue, ValidationReport,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use skill_loader::{load_skill, LoadedSkill, SkillPackageError};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const RUNTIME_API_VERSION: &str = "dock.runtime.v1";
pub const RUNTIME_IPC_TRANSPORT: &str = "headless-cli-json";
pub const RUNTIME_IPC_BINDING: &str = "local-process-stdio";
const RUNTIME_RENDER_IR_SCHEMA_VERSION: &str = "dock.render-ir.v1";

pub type RuntimeError = Box<RuntimeErrorResponse>;
pub type RuntimeResult<T> = Result<RuntimeResponse<T>, RuntimeError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersion {
    pub current: String,
    pub supported: Vec<String>,
}

impl Default for RuntimeVersion {
    fn default() -> Self {
        Self {
            current: RUNTIME_API_VERSION.to_owned(),
            supported: vec![RUNTIME_API_VERSION.to_owned()],
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIpcRequest {
    pub api_version: String,
    pub request_id: String,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIpcResponse {
    pub api_version: String,
    pub request_id: String,
    pub method: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RuntimeErrorDto>,
    pub redaction: RuntimeIpcRedaction,
    pub transport: RuntimeIpcTransport,
}

impl RuntimeIpcResponse {
    pub fn ok<T: Serialize>(
        request_id: impl Into<String>,
        method: impl Into<String>,
        response: RuntimeResponse<T>,
    ) -> Self {
        match serde_json::to_value(response) {
            Ok(result) => Self {
                api_version: RUNTIME_API_VERSION.to_owned(),
                request_id: request_id.into(),
                method: method.into(),
                status: "ok".to_owned(),
                result: Some(result),
                error: None,
                redaction: RuntimeIpcRedaction::default(),
                transport: RuntimeIpcTransport::default(),
            },
            Err(error) => Self::error(
                request_id,
                method,
                RuntimeErrorResponse::new(
                    "serialization_failed",
                    format!("runtime response serialization failed: {error}"),
                    None,
                ),
            ),
        }
    }

    pub fn error(
        request_id: impl Into<String>,
        method: impl Into<String>,
        error: RuntimeErrorResponse,
    ) -> Self {
        Self {
            api_version: RUNTIME_API_VERSION.to_owned(),
            request_id: request_id.into(),
            method: method.into(),
            status: "error".to_owned(),
            result: None,
            error: Some(error.error),
            redaction: RuntimeIpcRedaction::default(),
            transport: RuntimeIpcTransport::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIpcRedaction {
    pub marker: String,
    pub policy: String,
    pub applied_by_default: bool,
}

impl Default for RuntimeIpcRedaction {
    fn default() -> Self {
        Self {
            marker: "[REDACTED]".to_owned(),
            policy: "dock.runtime.redaction.v1".to_owned(),
            applied_by_default: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIpcTransport {
    pub mode: String,
    pub binding: String,
}

impl Default for RuntimeIpcTransport {
    fn default() -> Self {
        Self {
            mode: RUNTIME_IPC_TRANSPORT.to_owned(),
            binding: RUNTIME_IPC_BINDING.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIpcVersionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResponse<T> {
    pub version: String,
    pub status: String,
    pub data: T,
}

impl<T> RuntimeResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            version: RUNTIME_API_VERSION.to_owned(),
            status: "ok".to_owned(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorResponse {
    pub version: String,
    pub status: String,
    pub error: RuntimeErrorDto,
}

impl RuntimeErrorResponse {
    pub fn from_core(error: DockCoreError) -> Self {
        let validation = match &error {
            DockCoreError::Validation { report, .. } => Some(redacted_validation_report(report)),
            DockCoreError::Core { .. } => None,
        };
        let code = error.code().as_str().to_owned();
        let message = match error {
            DockCoreError::Core { message, .. } | DockCoreError::Validation { message, .. } => {
                redact_text(&message)
            }
        };
        Self::new(code, message, validation)
    }

    pub fn from_skill_load(error: SkillPackageError) -> Self {
        Self::new("skill_load_failed", redact_text(&error.to_string()), None)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new("unsupported", redact_text(&message.into()), None)
    }

    pub fn invalid_method(message: impl Into<String>) -> Self {
        Self::new("invalid_method", redact_text(&message.into()), None)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new("invalid_params", redact_text(&message.into()), None)
    }

    pub fn boxed(self) -> RuntimeError {
        Box::new(self)
    }

    fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        validation: Option<ValidationReport>,
    ) -> Self {
        Self {
            version: RUNTIME_API_VERSION.to_owned(),
            status: "error".to_owned(),
            error: RuntimeErrorDto {
                code: code.into(),
                message: message.into(),
                validation,
            },
        }
    }
}

impl fmt::Display for RuntimeErrorResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.error.code, self.error.message)
    }
}

impl Error for RuntimeErrorResponse {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSkillSummary {
    pub skill_id: String,
    pub package_ref: String,
    pub api_names: Vec<String>,
    pub component_paths: Vec<String>,
    pub validation: ValidationReport,
    pub supply_chain_status: String,
    pub production_ready: bool,
}

impl RuntimeSkillSummary {
    pub fn from_loaded(skill: &LoadedSkill) -> Self {
        Self {
            skill_id: skill_id(skill),
            package_ref: package_ref(skill),
            api_names: skill
                .manifest
                .apis
                .iter()
                .map(|api| api.name.clone())
                .collect(),
            component_paths: skill.components.keys().cloned().collect(),
            validation: skill.validation.clone(),
            supply_chain_status: skill.integrity.status.as_str().to_owned(),
            production_ready: skill.integrity.production_ready,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeValidateSkillResponse {
    pub skill: RuntimeSkillSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadSkillResponse {
    pub skill: RuntimeSkillSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostContractResponse {
    pub contract: HostAdapterContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSessionContext {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: String,
    pub session_id: String,
}

impl RuntimeSessionContext {
    fn to_api_context(
        &self,
        api_name: impl Into<String>,
        arguments: Value,
        capability_token: Option<String>,
        trace: Option<TraceContext>,
    ) -> ApiCallContext {
        ApiCallContext {
            user_did: self.user_did.clone(),
            agent_did: self.agent_did.clone(),
            merchant_did: self.merchant_did.clone(),
            skill_id: self.skill_id.clone(),
            session_id: self.session_id.clone(),
            api_name: api_name.into(),
            arguments,
            capability_token,
            trace,
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallRequest {
    pub session: RuntimeSessionContext,
    pub api_name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default, skip_serializing)]
    pub capability_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<RuntimeOperationOptions>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallResponse {
    pub api_name: String,
    pub result: AtomicApiResult,
    pub model_visible: ModelVisibleApiResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderOutcome>,
}

impl RuntimeCallResponse {
    pub fn from_outcome(api_name: impl Into<String>, outcome: CallOutcome) -> Self {
        let model_visible = outcome.result.model_visible();
        Self {
            api_name: api_name.into(),
            result: outcome.result,
            model_visible,
            render: outcome.render,
        }
    }

    pub fn into_call_outcome(self) -> CallOutcome {
        let model_visible = serde_json::to_value(&self.model_visible).unwrap_or(Value::Null);
        CallOutcome {
            result: self.result,
            model_visible,
            render: self.render,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderComponentRequest {
    pub session: RuntimeSessionContext,
    pub api_name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub content: Vec<TextContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Map<String, Value>>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Map<String, Value>>,
    pub component_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRenderComponentResponse {
    pub render: RenderOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuntimeComponentAction {
    SendFollowUpMessage {
        content: Vec<TextContent>,
    },
    ApiCall {
        name: String,
        #[serde(default)]
        arguments: Value,
    },
    OpenDetailPage {
        url: String,
    },
    ExpirePreviousCards {
        #[serde(default)]
        component_paths: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_policy: Option<String>,
    },
}

impl From<RuntimeComponentAction> for ComponentAction {
    fn from(action: RuntimeComponentAction) -> Self {
        match action {
            RuntimeComponentAction::SendFollowUpMessage { content } => {
                ComponentAction::SendFollowUpMessage { content }
            }
            RuntimeComponentAction::ApiCall { name, arguments } => {
                ComponentAction::ApiCall { name, arguments }
            }
            RuntimeComponentAction::OpenDetailPage { url } => {
                ComponentAction::OpenDetailPage { url }
            }
            RuntimeComponentAction::ExpirePreviousCards {
                component_paths,
                match_policy,
            } => ComponentAction::ExpirePreviousCards {
                component_paths,
                match_policy,
            },
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDispatchComponentActionRequest {
    pub session: RuntimeSessionContext,
    pub source_api_name: String,
    #[serde(default)]
    pub source_arguments: Value,
    pub action: RuntimeComponentAction,
    #[serde(default, skip_serializing)]
    pub capability_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<RuntimeOperationOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<RuntimeTraceContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancellation_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceContext {
    pub trace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDispatchComponentActionResponse {
    pub handled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<RuntimeCallResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_action: Option<HostActionOutcome>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExpireCardsRequest {
    pub session: RuntimeSessionContext,
    #[serde(default)]
    pub filters: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExpireCardsResponse {
    pub accepted: bool,
    pub expired_count: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuditRecordsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuditRecordsResponse {
    pub records: Vec<RuntimeAuditEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAuditEvent {
    pub user_did: Option<String>,
    pub agent_did: Option<String>,
    pub merchant_did: Option<String>,
    pub session_id: String,
    pub skill_id: String,
    pub api_name: String,
    pub risk_level: String,
    pub parameter_summary: Value,
    pub permission_decision: crate::host::PermissionDecisionSummary,
    pub consent_proof: Option<consent_audit::ConsentProof>,
    pub outcome: String,
}

impl RuntimeAuditEvent {
    pub fn from_event(event: AuditEvent) -> Self {
        let mut consent_proof = event.consent_proof;
        if let Some(proof) = &mut consent_proof {
            proof.parameter_summary = redact_value(&proof.parameter_summary);
        }
        Self {
            user_did: event.user_did,
            agent_did: event.agent_did,
            merchant_did: event.merchant_did,
            session_id: event.session_id,
            skill_id: event.skill_id,
            api_name: event.api_name,
            risk_level: event.risk_level.to_string(),
            parameter_summary: redact_value(&event.parameter_summary),
            permission_decision: event.permission_decision,
            consent_proof,
            outcome: event.outcome,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCloseSessionRequest {
    pub session: RuntimeSessionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCloseSessionResponse {
    pub session_id: String,
    pub closed: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConcurrencyPolicy {
    pub version: String,
    pub low_risk_parallel_per_session: bool,
    pub high_risk_serial_scope: String,
    pub cancellation_boundary: String,
    pub timeout_boundary: String,
    pub retry_policy: RuntimeRetryPolicy,
    pub idempotency: RuntimeIdempotencyPolicy,
}

impl Default for RuntimeConcurrencyPolicy {
    fn default() -> Self {
        Self {
            version: "dock.runtime.concurrency.v1".to_owned(),
            low_risk_parallel_per_session: true,
            high_risk_serial_scope: "session+api+idempotencyKey".to_owned(),
            cancellation_boundary: "runtime-pre-dispatch".to_owned(),
            timeout_boundary: "runtime-pre-dispatch-deadline".to_owned(),
            retry_policy: RuntimeRetryPolicy::default(),
            idempotency: RuntimeIdempotencyPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRetryPolicy {
    pub auth_challenge_retry: String,
    pub non_idempotent_business_retry: String,
}

impl Default for RuntimeRetryPolicy {
    fn default() -> Self {
        Self {
            auth_challenge_retry: "single-401-did-challenge-or-stale-token-refresh".to_owned(),
            non_idempotent_business_retry: "disabled-by-default".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIdempotencyPolicy {
    pub forwarded_argument: String,
    pub required_for_high_risk: bool,
    pub audit_summary: String,
}

impl Default for RuntimeIdempotencyPolicy {
    fn default() -> Self {
        Self {
            forwarded_argument: "idempotencyKey".to_owned(),
            required_for_high_risk: false,
            audit_summary: "redacted-parameter-summary".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConcurrencyPolicyResponse {
    pub policy: RuntimeConcurrencyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancelOperationRequest {
    pub session: RuntimeSessionContext,
    pub cancellation_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancelOperationResponse {
    pub session_id: String,
    pub cancellation_token: String,
    pub cancelled: bool,
    pub boundary: String,
}

pub trait RuntimeAuditReader {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, DockCoreError>;
}

impl RuntimeAuditReader for () {
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, DockCoreError> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeSessionKey {
    user_did: Option<String>,
    agent_did: Option<String>,
    merchant_did: Option<String>,
    skill_id: String,
    session_id: String,
}

impl RuntimeSessionKey {
    fn from_session(session: &RuntimeSessionContext) -> Self {
        Self {
            user_did: session.user_did.clone(),
            agent_did: session.agent_did.clone(),
            merchant_did: session.merchant_did.clone(),
            skill_id: session.skill_id.clone(),
            session_id: session.session_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HighRiskOperationKey {
    session: RuntimeSessionKey,
    api_name: String,
    idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct RuntimeConcurrencyState {
    closed_sessions: BTreeSet<RuntimeSessionKey>,
    cancelled_tokens: BTreeSet<(RuntimeSessionKey, String)>,
    high_risk_in_flight: BTreeSet<HighRiskOperationKey>,
    high_risk_completed: BTreeMap<HighRiskOperationKey, RuntimeCallResponse>,
}

#[derive(Debug, Default)]
struct RuntimeConcurrencyControl {
    state: Mutex<RuntimeConcurrencyState>,
}

impl RuntimeConcurrencyControl {
    fn begin(
        &self,
        session: &RuntimeSessionContext,
        api_name: &str,
        arguments: &mut Value,
        operation: Option<&RuntimeOperationOptions>,
        risk_level: RiskLevel,
    ) -> Result<RuntimeOperationGuard<'_>, DockCoreError> {
        let session_key = RuntimeSessionKey::from_session(session);
        let timeout_ms = operation.and_then(|operation| operation.timeout_ms);
        if timeout_ms == Some(0) {
            return Err(runtime_operation_error(
                ErrorCode::Timeout,
                "runtime operation timed out before dispatch",
            ));
        }
        let deadline =
            timeout_ms.map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms.max(1)));

        let cancellation_token = operation
            .and_then(|operation| operation.cancellation_token.as_deref())
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned);
        let mut state = self.state.lock().map_err(|_| {
            runtime_operation_error(
                ErrorCode::AuditUnavailable,
                "runtime concurrency state is unavailable",
            )
        })?;
        if state.closed_sessions.contains(&session_key) {
            return Err(runtime_operation_error(
                ErrorCode::PermissionDenied,
                "runtime session is closed",
            ));
        }
        if let Some(token) = &cancellation_token {
            if state
                .cancelled_tokens
                .contains(&(session_key.clone(), token.clone()))
            {
                return Err(runtime_operation_error(
                    ErrorCode::PermissionDenied,
                    "runtime operation was cancelled before dispatch",
                ));
            }
        }

        let high_risk_key = if risk_level.requires_consent() {
            let idempotency_key = extract_or_inject_idempotency_key(arguments, operation)?;
            let key = HighRiskOperationKey {
                session: session_key.clone(),
                api_name: api_name.to_owned(),
                idempotency_key: idempotency_key.clone(),
            };
            if idempotency_key.is_some()
                && state.high_risk_completed.get(&key).is_some_and(|_| true)
            {
                let response = state
                    .high_risk_completed
                    .get(&key)
                    .cloned()
                    .expect("checked completed response");
                return Ok(RuntimeOperationGuard::completed(response));
            }
            if !state.high_risk_in_flight.insert(key.clone()) {
                return Err(runtime_operation_error(
                    ErrorCode::PermissionDenied,
                    "high-risk runtime operation is already in flight",
                ));
            }
            Some(key)
        } else {
            None
        };
        drop(state);

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            if let Some(key) = high_risk_key {
                self.release_high_risk(&key);
            }
            return Err(runtime_operation_error(
                ErrorCode::Timeout,
                "runtime operation timed out before dispatch",
            ));
        }

        Ok(RuntimeOperationGuard::active(self, high_risk_key, deadline))
    }

    fn cancel(
        &self,
        session: &RuntimeSessionContext,
        cancellation_token: String,
    ) -> Result<bool, DockCoreError> {
        let token = cancellation_token.trim();
        if token.is_empty() {
            return Err(runtime_operation_error(
                ErrorCode::ValidationFailed,
                "cancellation token is required",
            ));
        }
        let mut state = self.state.lock().map_err(|_| {
            runtime_operation_error(
                ErrorCode::AuditUnavailable,
                "runtime concurrency state is unavailable",
            )
        })?;
        Ok(state
            .cancelled_tokens
            .insert((RuntimeSessionKey::from_session(session), token.to_owned())))
    }

    fn close_session(&self, session: &RuntimeSessionContext) -> Result<(), DockCoreError> {
        let session_key = RuntimeSessionKey::from_session(session);
        let mut state = self.state.lock().map_err(|_| {
            runtime_operation_error(
                ErrorCode::AuditUnavailable,
                "runtime concurrency state is unavailable",
            )
        })?;
        state.closed_sessions.insert(session_key.clone());
        state
            .cancelled_tokens
            .retain(|(key, _)| key != &session_key);
        state
            .high_risk_in_flight
            .retain(|key| key.session != session_key);
        state
            .high_risk_completed
            .retain(|key, _| key.session != session_key);
        Ok(())
    }

    fn release_high_risk(&self, key: &HighRiskOperationKey) {
        if let Ok(mut state) = self.state.lock() {
            state.high_risk_in_flight.remove(key);
        }
    }

    fn complete_high_risk(&self, key: &HighRiskOperationKey, response: RuntimeCallResponse) {
        if let Ok(mut state) = self.state.lock() {
            state.high_risk_in_flight.remove(key);
            if key.idempotency_key.is_some() {
                state.high_risk_completed.insert(key.clone(), response);
            }
        }
    }
}

enum RuntimeOperationGuard<'a> {
    Active {
        control: &'a RuntimeConcurrencyControl,
        high_risk_key: Option<HighRiskOperationKey>,
        deadline: Option<Instant>,
        completed: bool,
    },
    Completed(RuntimeCallResponse),
}

impl<'a> RuntimeOperationGuard<'a> {
    fn active(
        control: &'a RuntimeConcurrencyControl,
        high_risk_key: Option<HighRiskOperationKey>,
        deadline: Option<Instant>,
    ) -> Self {
        Self::Active {
            control,
            high_risk_key,
            deadline,
            completed: false,
        }
    }

    fn completed(response: RuntimeCallResponse) -> Self {
        Self::Completed(response)
    }

    fn cached_response(&self) -> Option<RuntimeCallResponse> {
        match self {
            Self::Completed(response) => Some(response.clone()),
            Self::Active { .. } => None,
        }
    }

    fn check_deadline(&self) -> Result<(), DockCoreError> {
        match self {
            Self::Active {
                deadline: Some(deadline),
                ..
            } if Instant::now() >= *deadline => Err(runtime_operation_error(
                ErrorCode::Timeout,
                "runtime operation timed out before executor dispatch completed",
            )),
            _ => Ok(()),
        }
    }

    fn mark_completed(&mut self, response: &RuntimeCallResponse) {
        if let Self::Active {
            control,
            high_risk_key: Some(key),
            completed,
            ..
        } = self
        {
            control.complete_high_risk(key, response.clone());
            *completed = true;
        }
    }
}

impl Drop for RuntimeOperationGuard<'_> {
    fn drop(&mut self) {
        if let Self::Active {
            control,
            high_risk_key: Some(key),
            completed,
            ..
        } = self
        {
            if !*completed {
                control.release_high_risk(key);
            }
        }
    }
}

fn extract_or_inject_idempotency_key(
    arguments: &mut Value,
    operation: Option<&RuntimeOperationOptions>,
) -> Result<Option<String>, DockCoreError> {
    let operation_key = operation
        .and_then(|operation| operation.idempotency_key.as_deref())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned);
    let argument_key = arguments
        .get("idempotencyKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned);

    match (operation_key, argument_key) {
        (Some(operation_key), Some(argument_key)) if operation_key != argument_key => {
            Err(runtime_operation_error(
                ErrorCode::ValidationFailed,
                "idempotencyKey in operation metadata does not match arguments",
            ))
        }
        (Some(key), _) => {
            inject_idempotency_key(arguments, &key)?;
            Ok(Some(key))
        }
        (None, Some(key)) => Ok(Some(key)),
        (None, None) => Ok(None),
    }
}

fn inject_idempotency_key(
    arguments: &mut Value,
    idempotency_key: &str,
) -> Result<(), DockCoreError> {
    match arguments {
        Value::Object(map) => {
            map.entry("idempotencyKey".to_owned())
                .or_insert_with(|| Value::String(idempotency_key.to_owned()));
            Ok(())
        }
        Value::Null => {
            let mut map = Map::new();
            map.insert(
                "idempotencyKey".to_owned(),
                Value::String(idempotency_key.to_owned()),
            );
            *arguments = Value::Object(map);
            Ok(())
        }
        _ => Err(runtime_operation_error(
            ErrorCode::ValidationFailed,
            "idempotencyKey can only be injected into object arguments",
        )),
    }
}

fn runtime_operation_error(code: ErrorCode, message: impl Into<String>) -> DockCoreError {
    DockCoreError::core(code, message)
}

#[derive(Debug, Clone)]
pub struct RuntimePersistentAuditSink<S> {
    sink: Arc<S>,
}

impl<S> RuntimePersistentAuditSink<S> {
    pub fn new(sink: S) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    pub fn sink(&self) -> &S {
        self.sink.as_ref()
    }
}

impl<S> AuditSink for RuntimePersistentAuditSink<S>
where
    S: consent_audit::AuditSink,
{
    fn ensure_available(&self) -> Result<(), DockCoreError> {
        self.sink.ensure_available().map_err(|_| {
            DockCoreError::core(
                ErrorCode::AuditUnavailable,
                "audit sink unavailable for runtime audit",
            )
        })
    }

    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError> {
        self.sink
            .record(audit_record_from_event(event))
            .map_err(|_| {
                DockCoreError::core(
                    ErrorCode::AuditUnavailable,
                    "audit sink unavailable for runtime audit",
                )
            })
    }
}

impl<S> RuntimeAuditReader for RuntimePersistentAuditSink<S>
where
    S: consent_audit::AuditSink + PersistentAuditRecordReader,
{
    fn runtime_audit_records(&self) -> Result<Vec<AuditEvent>, DockCoreError> {
        Ok(self
            .sink
            .records()
            .map_err(|_| {
                DockCoreError::core(
                    ErrorCode::AuditUnavailable,
                    "audit sink unavailable for runtime audit",
                )
            })?
            .into_iter()
            .map(AuditEvent::from)
            .collect())
    }
}

pub trait PersistentAuditRecordReader {
    fn records(&self) -> Result<Vec<AuditRecord>, consent_audit::AuditError>;
}

impl PersistentAuditRecordReader for consent_audit::FileAuditSink {
    fn records(&self) -> Result<Vec<AuditRecord>, consent_audit::AuditError> {
        consent_audit::FileAuditSink::records(self)
    }
}

fn audit_record_from_event(event: AuditEvent) -> AuditRecord {
    let permission_decision = serde_json::to_value(&event.permission_decision)
        .map_or(Value::Null, |value| redact_value(&value));
    let mut record = AuditRecord::new(AuditRecordInput {
        user_did: event.user_did,
        agent_did: event.agent_did,
        merchant_did: event.merchant_did,
        session_id: event.session_id,
        skill_id: event.skill_id,
        api_name: event.api_name,
        risk_level: event.risk_level,
        arguments: &event.parameter_summary,
        consent_proof: event.consent_proof,
        outcome: AuditOutcome::from_label(&event.outcome),
    });
    record.permission_decision = Some(permission_decision);
    record
}

impl From<AuditRecord> for AuditEvent {
    fn from(record: AuditRecord) -> Self {
        let parameter_summary = redact_value(&record.parameter_summary);
        let permission_decision = permission_decision_from_audit_record(&record);
        Self {
            user_did: record.user_did,
            agent_did: record.agent_did,
            merchant_did: record.merchant_did,
            session_id: record.session_id,
            skill_id: record.skill_id,
            api_name: record.api_name,
            risk_level: record.risk_level,
            parameter_summary,
            permission_decision,
            consent_proof: record.consent_proof,
            outcome: record.outcome.to_string(),
        }
    }
}

fn permission_decision_from_audit_record(
    record: &AuditRecord,
) -> crate::host::PermissionDecisionSummary {
    record
        .permission_decision
        .as_ref()
        .and_then(|value| {
            serde_json::from_value::<crate::host::PermissionDecisionSummary>(redact_value(value))
                .ok()
        })
        .unwrap_or_else(|| crate::host::PermissionDecisionSummary {
            decision: "persisted_audit".to_owned(),
            reason_code: "persistent_audit_record".to_owned(),
            reason: "record restored from persistent audit sink".to_owned(),
            dev_only: false,
        })
}

pub fn negotiate_runtime_version(requested: Option<&str>) -> RuntimeResult<RuntimeVersion> {
    match requested {
        Some(version) if version != RUNTIME_API_VERSION => Err(RuntimeErrorResponse::new(
            "unsupported_version",
            format!("runtime API version `{version}` is not supported"),
            None,
        )
        .boxed()),
        _ => Ok(RuntimeResponse::ok(RuntimeVersion::default())),
    }
}

pub fn validate_skill_path(
    skill_ref: impl AsRef<Path>,
) -> RuntimeResult<RuntimeValidateSkillResponse> {
    let skill = load_skill(skill_ref)
        .map_err(|error| RuntimeErrorResponse::from_skill_load(error).boxed())?;
    Ok(RuntimeResponse::ok(RuntimeValidateSkillResponse {
        skill: RuntimeSkillSummary::from_loaded(&skill),
    }))
}

pub fn load_skill_path(skill_ref: impl AsRef<Path>) -> RuntimeResult<RuntimeLoadSkillResponse> {
    let skill = load_skill(skill_ref)
        .map_err(|error| RuntimeErrorResponse::from_skill_load(error).boxed())?;
    Ok(RuntimeResponse::ok(RuntimeLoadSkillResponse {
        skill: RuntimeSkillSummary::from_loaded(&skill),
    }))
}

pub struct RuntimeService<H, C, E, R, A, Q = (), O = NoopObservabilitySink, M = NoopMetricsSink> {
    orchestrator: Orchestrator<H, C, E, R, A>,
    audit_reader: Q,
    concurrency: RuntimeConcurrencyControl,
    observability: O,
    metrics: M,
}

pub struct RuntimeServiceParts<H, C, E, R, A, Q, O> {
    pub skill: LoadedSkill,
    pub host: H,
    pub consent: C,
    pub executor: E,
    pub renderer: R,
    pub audit: A,
    pub audit_reader: Q,
    pub observability: O,
}

pub struct RuntimeServiceObservabilityParts<H, C, E, R, A, Q, O, M> {
    pub skill: LoadedSkill,
    pub host: H,
    pub consent: C,
    pub executor: E,
    pub renderer: R,
    pub audit: A,
    pub audit_reader: Q,
    pub observability: O,
    pub metrics: M,
}

impl<H, C, E, R, A, Q> RuntimeService<H, C, E, R, A, Q, NoopObservabilitySink, NoopMetricsSink>
where
    H: RuntimeHost,
    C: ConsentGate,
    E: ApiExecutor,
    R: RenderRouter,
    A: AuditSink,
    Q: RuntimeAuditReader,
{
    pub fn load_skill(
        skill: LoadedSkill,
        host: H,
        consent: C,
        executor: E,
        renderer: R,
        audit: A,
        audit_reader: Q,
    ) -> Self {
        Self {
            orchestrator: Orchestrator::load_skill(skill, host, consent, executor, renderer, audit),
            audit_reader,
            concurrency: RuntimeConcurrencyControl::default(),
            observability: NoopObservabilitySink,
            metrics: NoopMetricsSink,
        }
    }

    pub fn from_orchestrator(orchestrator: Orchestrator<H, C, E, R, A>, audit_reader: Q) -> Self {
        Self {
            orchestrator,
            audit_reader,
            concurrency: RuntimeConcurrencyControl::default(),
            observability: NoopObservabilitySink,
            metrics: NoopMetricsSink,
        }
    }
}

impl<H, C, E, R, A, Q, O> RuntimeService<H, C, E, R, A, Q, O, NoopMetricsSink>
where
    H: RuntimeHost,
    C: ConsentGate,
    E: ApiExecutor,
    R: RenderRouter,
    A: AuditSink,
    Q: RuntimeAuditReader,
    O: ObservabilitySink,
{
    pub fn load_skill_with_observability(parts: RuntimeServiceParts<H, C, E, R, A, Q, O>) -> Self {
        Self::load_skill_with_observability_and_metrics(RuntimeServiceObservabilityParts {
            skill: parts.skill,
            host: parts.host,
            consent: parts.consent,
            executor: parts.executor,
            renderer: parts.renderer,
            audit: parts.audit,
            audit_reader: parts.audit_reader,
            observability: parts.observability,
            metrics: NoopMetricsSink,
        })
    }
}

impl<H, C, E, R, A, Q, O, M> RuntimeService<H, C, E, R, A, Q, O, M>
where
    H: RuntimeHost,
    C: ConsentGate,
    E: ApiExecutor,
    R: RenderRouter,
    A: AuditSink,
    Q: RuntimeAuditReader,
    O: ObservabilitySink,
    M: MetricsSink,
{
    pub fn load_skill_with_observability_and_metrics(
        parts: RuntimeServiceObservabilityParts<H, C, E, R, A, Q, O, M>,
    ) -> Self {
        let service = Self {
            orchestrator: Orchestrator::load_skill(
                parts.skill,
                parts.host,
                parts.consent,
                parts.executor,
                parts.renderer,
                parts.audit,
            ),
            audit_reader: parts.audit_reader,
            concurrency: RuntimeConcurrencyControl::default(),
            observability: parts.observability,
            metrics: parts.metrics,
        };
        let summary = RuntimeSkillSummary::from_loaded(service.skill());
        let trace_id = next_observability_id("trace");
        service.emit_observability(
            ObservabilityEvent::new(ObservabilityEventKind::SkillLoadStart)
                .with_trace_id(trace_id.clone())
                .with_skill_id(summary.skill_id.clone())
                .with_outcome("started")
                .with_field("apiCount", json!(summary.api_names.len()))
                .with_field("componentCount", json!(summary.component_paths.len())),
        );
        service.emit_observability(
            ObservabilityEvent::new(ObservabilityEventKind::SkillLoadEnd)
                .with_trace_id(trace_id)
                .with_skill_id(summary.skill_id)
                .with_outcome(if summary.production_ready {
                    "ok"
                } else {
                    "warning"
                })
                .with_field("supplyChainStatus", json!(summary.supply_chain_status))
                .with_field("productionReady", json!(summary.production_ready)),
        );
        service.record_metric(
            ObservabilityMetric::counter("dock.skill_load.total", 1)
                .with_label(
                    "outcome",
                    if summary.production_ready {
                        "ok"
                    } else {
                        "warning"
                    },
                )
                .with_label("supplyChainStatus", summary.supply_chain_status),
        );
        service
    }

    pub fn skill(&self) -> &LoadedSkill {
        self.orchestrator.skill()
    }

    pub fn observability(&self) -> O {
        self.observability.clone()
    }

    pub fn metrics(&self) -> M {
        self.metrics.clone()
    }

    pub fn validate_skill(&self) -> RuntimeResponse<RuntimeValidateSkillResponse> {
        RuntimeResponse::ok(RuntimeValidateSkillResponse {
            skill: RuntimeSkillSummary::from_loaded(self.skill()),
        })
    }

    pub fn load_skill_response(&self) -> RuntimeResponse<RuntimeLoadSkillResponse> {
        RuntimeResponse::ok(RuntimeLoadSkillResponse {
            skill: RuntimeSkillSummary::from_loaded(self.skill()),
        })
    }

    pub fn host_contract(&self) -> RuntimeResponse<RuntimeHostContractResponse> {
        RuntimeResponse::ok(RuntimeHostContractResponse {
            contract: self.orchestrator.host_contract(),
        })
    }

    pub fn call_api(&self, request: RuntimeCallRequest) -> RuntimeResult<RuntimeCallResponse> {
        let api_name = request.api_name.clone();
        self.call_api_with_operation(
            request.session,
            request.api_name,
            request.arguments,
            request.capability_token,
            request.operation,
        )
        .map(|mut response| {
            response.data.api_name = api_name;
            response
        })
    }

    pub fn concurrency_policy(&self) -> RuntimeResponse<RuntimeConcurrencyPolicyResponse> {
        RuntimeResponse::ok(RuntimeConcurrencyPolicyResponse {
            policy: RuntimeConcurrencyPolicy::default(),
        })
    }

    fn call_api_with_operation(
        &self,
        session: RuntimeSessionContext,
        api_name: String,
        mut arguments: Value,
        capability_token: Option<String>,
        operation: Option<RuntimeOperationOptions>,
    ) -> RuntimeResult<RuntimeCallResponse> {
        let trace = trace_context_for(&operation);
        let trace_id = trace.trace_id.clone();
        let span = trace.child();
        let started = Instant::now();
        self.emit_session_event(
            ObservabilityEvent::new(ObservabilityEventKind::ApiCallStart)
                .with_trace_id(trace_id.clone())
                .with_api_name(api_name.clone())
                .with_outcome("started"),
            &session,
        );
        self.record_metric(
            ObservabilityMetric::counter("dock.api_call.total", 1)
                .with_label("phase", "start")
                .with_label("api", api_name.clone())
                .with_trace_id(trace_id.clone()),
        );
        let risk_level = self
            .orchestrator
            .api_risk_level(&api_name)
            .map_err(|error| {
                let latency = elapsed_ms(started);
                if error.code() == ErrorCode::ApiNotFound {
                    self.record_metric(
                        ObservabilityMetric::counter("dock.unsupported_api.total", 1)
                            .with_label("api", api_name.clone())
                            .with_label("outcome", error.code().as_str())
                            .with_trace_id(trace_id.clone()),
                    );
                }
                self.emit_session_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(api_name.clone())
                        .with_outcome(error.code().as_str())
                        .with_latency_ms(latency),
                    &session,
                );
                self.record_api_end(
                    &trace_id,
                    &span,
                    &api_name,
                    error.code().as_str(),
                    latency,
                    None,
                );
                RuntimeErrorResponse::from_core(error).boxed()
            })?;
        let mut guard = self
            .concurrency
            .begin(
                &session,
                &api_name,
                &mut arguments,
                operation.as_ref(),
                risk_level,
            )
            .map_err(|error| {
                let latency = elapsed_ms(started);
                self.emit_limit_if_needed(&session, &api_name, &trace_id, &error, started);
                self.emit_session_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(api_name.clone())
                        .with_outcome(error.code().as_str())
                        .with_latency_ms(latency),
                    &session,
                );
                self.record_api_end(
                    &trace_id,
                    &span,
                    &api_name,
                    error.code().as_str(),
                    latency,
                    Some(risk_level),
                );
                RuntimeErrorResponse::from_core(error).boxed()
            })?;
        if let Some(response) = guard.cached_response() {
            let latency = elapsed_ms(started);
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(api_name.clone())
                    .with_outcome("idempotent_replay")
                    .with_latency_ms(latency),
                &session,
            );
            self.record_api_end(
                &trace_id,
                &span,
                &api_name,
                "idempotent_replay",
                latency,
                Some(risk_level),
            );
            return Ok(RuntimeResponse::ok(response));
        }
        if risk_level.requires_consent() {
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::ConsentPrompt)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(api_name.clone())
                    .with_outcome("prompted")
                    .with_field("riskLevel", json!(risk_level.to_string())),
                &session,
            );
            self.record_metric(
                ObservabilityMetric::counter("dock.consent.total", 1)
                    .with_label("phase", "prompt")
                    .with_label("riskLevel", risk_level.to_string())
                    .with_trace_id(trace_id.clone()),
            );
        }
        guard.check_deadline().map_err(|error| {
            let latency = elapsed_ms(started);
            self.emit_limit_if_needed(&session, &api_name, &trace_id, &error, started);
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(api_name.clone())
                    .with_outcome(error.code().as_str())
                    .with_latency_ms(latency),
                &session,
            );
            self.record_api_end(
                &trace_id,
                &span,
                &api_name,
                error.code().as_str(),
                latency,
                Some(risk_level),
            );
            RuntimeErrorResponse::from_core(error).boxed()
        })?;
        let outcome = self
            .orchestrator
            .call_api(session.to_api_context(
                api_name.clone(),
                arguments,
                capability_token,
                Some(span.clone()),
            ))
            .map_err(|error| {
                if risk_level.requires_consent() {
                    self.emit_session_event(
                        ObservabilityEvent::new(ObservabilityEventKind::ConsentDecision)
                            .with_trace_id(trace_id.clone())
                            .with_api_name(api_name.clone())
                            .with_outcome(error.code().as_str())
                            .with_field("riskLevel", json!(risk_level.to_string())),
                        &session,
                    );
                    self.record_metric(
                        ObservabilityMetric::counter("dock.consent.total", 1)
                            .with_label("phase", "decision")
                            .with_label("outcome", error.code().as_str())
                            .with_label("riskLevel", risk_level.to_string())
                            .with_trace_id(trace_id.clone()),
                    );
                }
                self.emit_session_event(
                    ObservabilityEvent::new(ObservabilityEventKind::AuditRecordWritten)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(api_name.clone())
                        .with_outcome(error.code().as_str())
                        .with_field("riskLevel", json!(risk_level.to_string())),
                    &session,
                );
                self.record_metric(
                    ObservabilityMetric::counter("dock.audit_record.total", 1)
                        .with_label("outcome", error.code().as_str())
                        .with_label("riskLevel", risk_level.to_string())
                        .with_trace_id(trace_id.clone()),
                );
                self.record_audit_span(
                    &trace_id,
                    &span,
                    &api_name,
                    error.code().as_str(),
                    risk_level,
                    elapsed_ms(started),
                );
                let latency = elapsed_ms(started);
                self.emit_session_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(api_name.clone())
                        .with_outcome(error.code().as_str())
                        .with_latency_ms(latency),
                    &session,
                );
                self.record_api_end(
                    &trace_id,
                    &span,
                    &api_name,
                    error.code().as_str(),
                    latency,
                    Some(risk_level),
                );
                RuntimeErrorResponse::from_core(error).boxed()
            })?;
        let component_path = outcome
            .render
            .as_ref()
            .and_then(|render| render.component_path.clone());
        let fallback_reason = outcome
            .render
            .as_ref()
            .and_then(|render| render.fallback_reason.clone());
        guard.check_deadline().map_err(|error| {
            let latency = elapsed_ms(started);
            self.emit_limit_if_needed(&session, &api_name, &trace_id, &error, started);
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(api_name.clone())
                    .with_outcome(error.code().as_str())
                    .with_latency_ms(latency),
                &session,
            );
            self.record_api_end(
                &trace_id,
                &span,
                &api_name,
                error.code().as_str(),
                latency,
                Some(risk_level),
            );
            RuntimeErrorResponse::from_core(error).boxed()
        })?;
        let response = RuntimeCallResponse::from_outcome(api_name, outcome);
        guard.mark_completed(&response);
        if risk_level.requires_consent() {
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::ConsentDecision)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(response.api_name.clone())
                    .with_outcome("ok")
                    .with_field("riskLevel", json!(risk_level.to_string())),
                &session,
            );
            self.record_metric(
                ObservabilityMetric::counter("dock.consent.total", 1)
                    .with_label("phase", "decision")
                    .with_label("outcome", "ok")
                    .with_label("riskLevel", risk_level.to_string())
                    .with_trace_id(trace_id.clone()),
            );
        }
        let audit_outcome = if response.result.is_error {
            "error"
        } else {
            "ok"
        };
        self.emit_session_event(
            ObservabilityEvent::new(ObservabilityEventKind::AuditRecordWritten)
                .with_trace_id(trace_id.clone())
                .with_api_name(response.api_name.clone())
                .with_outcome(audit_outcome)
                .with_field("riskLevel", json!(risk_level.to_string())),
            &session,
        );
        self.record_metric(
            ObservabilityMetric::counter("dock.audit_record.total", 1)
                .with_label("outcome", audit_outcome)
                .with_label("riskLevel", risk_level.to_string())
                .with_trace_id(trace_id.clone()),
        );
        self.record_audit_span(
            &trace_id,
            &span,
            &response.api_name,
            audit_outcome,
            risk_level,
            elapsed_ms(started),
        );
        if let Some(reason) = fallback_reason {
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::FallbackUsed)
                    .with_trace_id(trace_id.clone())
                    .with_api_name(response.api_name.clone())
                    .with_outcome("fallback")
                    .with_latency_ms(elapsed_ms(started))
                    .with_field("reason", json!(reason)),
                &session,
            );
            self.record_metric(
                ObservabilityMetric::counter("dock.fallback.total", 1)
                    .with_label("reason", reason)
                    .with_trace_id(trace_id.clone()),
            );
        }
        let latency = elapsed_ms(started);
        let outcome = if response.result.is_error {
            "error"
        } else {
            "ok"
        };
        self.emit_session_event(
            ObservabilityEvent::new(ObservabilityEventKind::ApiCallEnd)
                .with_trace_id(trace_id.clone())
                .with_api_name(response.api_name.clone())
                .with_component_path(component_path.unwrap_or_default())
                .with_outcome(outcome)
                .with_latency_ms(latency),
            &session,
        );
        self.record_api_end(
            &trace_id,
            &span,
            &response.api_name,
            outcome,
            latency,
            Some(risk_level),
        );
        Ok(RuntimeResponse::ok(response))
    }

    pub fn render_component(
        &self,
        request: RuntimeRenderComponentRequest,
    ) -> RuntimeResult<RuntimeRenderComponentResponse> {
        let trace = TraceContext::root();
        let trace_id = trace.trace_id.clone();
        let span = trace.child();
        let started = Instant::now();
        let context = request.session.to_api_context(
            request.api_name.clone(),
            request.arguments.clone(),
            None,
            Some(span.clone()),
        );
        self.emit_context_event(
            ObservabilityEvent::new(ObservabilityEventKind::ComponentRenderStart)
                .with_trace_id(trace_id.clone())
                .with_api_name(request.api_name.clone())
                .with_component_path(request.component_path.clone())
                .with_render_ir_version(RUNTIME_RENDER_IR_SCHEMA_VERSION)
                .with_outcome("started"),
            &context,
        );
        let input = ComponentRenderInput {
            api_name: request.api_name,
            arguments: request.arguments,
            content: request.content,
            structured_content: request.structured_content,
            meta: request.meta,
            component_path: request.component_path,
        };
        self.orchestrator
            .render_component(&context, input)
            .map(|render| {
                let latency = elapsed_ms(started);
                self.emit_context_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ComponentRenderEnd)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(context.api_name.clone())
                        .with_component_path(render.component_path.clone().unwrap_or_default())
                        .with_render_ir_version(RUNTIME_RENDER_IR_SCHEMA_VERSION)
                        .with_outcome("ok")
                        .with_latency_ms(latency),
                    &context,
                );
                self.record_render_end(
                    &trace_id,
                    &span,
                    render.component_path.as_deref().unwrap_or("none"),
                    "ok",
                    latency,
                );
                RuntimeResponse::ok(RuntimeRenderComponentResponse { render })
            })
            .map_err(|error| {
                let latency = elapsed_ms(started);
                self.emit_context_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ComponentRenderEnd)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(context.api_name.clone())
                        .with_render_ir_version(RUNTIME_RENDER_IR_SCHEMA_VERSION)
                        .with_outcome(error.code().as_str())
                        .with_latency_ms(latency),
                    &context,
                );
                self.record_render_end(&trace_id, &span, "none", error.code().as_str(), latency);
                RuntimeErrorResponse::from_core(error).boxed()
            })
    }

    pub fn dispatch_component_action(
        &self,
        request: RuntimeDispatchComponentActionRequest,
    ) -> RuntimeResult<RuntimeDispatchComponentActionResponse> {
        let trace = trace_context_for(&request.operation);
        let base_context = request.session.to_api_context(
            request.source_api_name.clone(),
            request.source_arguments,
            request.capability_token,
            Some(trace.clone()),
        );
        match request.action {
            RuntimeComponentAction::ApiCall { name, arguments } => {
                self.emit_context_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ComponentEvent)
                        .with_trace_id(trace.trace_id.clone())
                        .with_api_name(request.source_api_name.clone())
                        .with_outcome("api_call")
                        .with_field("targetApiName", json!(name.clone())),
                    &base_context,
                );
                let call = self.call_api_with_operation(
                    request.session,
                    name,
                    arguments,
                    base_context.capability_token,
                    operation_with_trace(request.operation, &trace),
                )?;
                Ok(RuntimeResponse::ok(
                    RuntimeDispatchComponentActionResponse {
                        handled: true,
                        call: Some(call.data),
                        host_action: None,
                        boundary: "runtime-orchestrator".to_owned(),
                    },
                ))
            }
            RuntimeComponentAction::SendFollowUpMessage { content } => self.dispatch_host_action(
                &base_context,
                &request.session,
                request.operation,
                "sendFollowUpMessage",
                HostActionRequest::send_follow_up_message(request.source_api_name, &content),
            ),
            RuntimeComponentAction::OpenDetailPage { url } => {
                let canonical_url = canonicalize_open_detail_page_target(&url)
                    .map_err(|error| RuntimeErrorResponse::from_core(error).boxed())?;
                self.dispatch_host_action(
                    &base_context,
                    &request.session,
                    request.operation,
                    "openDetailPage",
                    HostActionRequest::open_detail_page(request.source_api_name, canonical_url),
                )
            }
            RuntimeComponentAction::ExpirePreviousCards {
                component_paths,
                match_policy,
            } => self.dispatch_host_action(
                &base_context,
                &request.session,
                request.operation,
                "expirePreviousCards",
                HostActionRequest::expire_previous_cards(
                    request.source_api_name,
                    component_paths,
                    match_policy,
                ),
            ),
        }
    }

    fn dispatch_host_action(
        &self,
        context: &ApiCallContext,
        session: &RuntimeSessionContext,
        operation: Option<RuntimeOperationOptions>,
        component_event: &str,
        request: HostActionRequest,
    ) -> RuntimeResult<RuntimeDispatchComponentActionResponse> {
        let trace = context
            .trace
            .as_ref()
            .map(TraceContext::child)
            .unwrap_or_else(|| trace_context_for(&operation));
        let trace_id = trace.trace_id.clone();
        let span = trace.child();
        let started = Instant::now();
        self.emit_context_event(
            ObservabilityEvent::new(ObservabilityEventKind::ComponentEvent)
                .with_trace_id(trace_id.clone())
                .with_api_name(context.api_name.clone())
                .with_outcome(component_event)
                .with_field("hostActionType", json!(request.action_type.clone())),
            context,
        );
        let mut arguments = context.arguments.clone();
        let guard = self
            .concurrency
            .begin(
                session,
                &context.api_name,
                &mut arguments,
                operation.as_ref(),
                RiskLevel::L2,
            )
            .map_err(|error| {
                self.emit_limit_if_needed(session, &context.api_name, &trace_id, &error, started);
                RuntimeErrorResponse::from_core(error).boxed()
            })?;
        guard.check_deadline().map_err(|error| {
            self.emit_limit_if_needed(session, &context.api_name, &trace_id, &error, started);
            RuntimeErrorResponse::from_core(error).boxed()
        })?;
        let outcome = self
            .orchestrator
            .handle_host_action(context, request)
            .map_err(|error| {
                let latency = elapsed_ms(started);
                self.emit_context_event(
                    ObservabilityEvent::new(ObservabilityEventKind::ComponentEvent)
                        .with_trace_id(trace_id.clone())
                        .with_api_name(context.api_name.clone())
                        .with_outcome(error.code().as_str())
                        .with_latency_ms(latency),
                    context,
                );
                self.record_component_action_end(
                    &trace_id,
                    &span,
                    component_event,
                    error.code().as_str(),
                    latency,
                );
                RuntimeErrorResponse::from_core(error).boxed()
            })?
            .redacted();
        guard.check_deadline().map_err(|error| {
            self.emit_limit_if_needed(session, &context.api_name, &trace_id, &error, started);
            RuntimeErrorResponse::from_core(error).boxed()
        })?;
        let handled = outcome.status == HostActionStatus::Accepted;
        let latency = elapsed_ms(started);
        let component_outcome = if handled { "ok" } else { "unsupported" };
        self.emit_context_event(
            ObservabilityEvent::new(ObservabilityEventKind::ComponentEvent)
                .with_trace_id(trace_id)
                .with_api_name(context.api_name.clone())
                .with_outcome(component_outcome)
                .with_latency_ms(latency)
                .with_field("hostActionStatus", json!(format!("{:?}", outcome.status))),
            context,
        );
        self.record_component_action_end(
            trace.trace_id.as_str(),
            &span,
            component_event,
            component_outcome,
            latency,
        );
        Ok(RuntimeResponse::ok(
            RuntimeDispatchComponentActionResponse {
                handled,
                call: None,
                boundary: outcome.boundary.clone(),
                host_action: Some(outcome),
            },
        ))
    }

    pub fn expire_cards(
        &self,
        _request: RuntimeExpireCardsRequest,
    ) -> RuntimeResult<RuntimeExpireCardsResponse> {
        Ok(RuntimeResponse::ok(RuntimeExpireCardsResponse {
            accepted: true,
            expired_count: 0,
            boundary: "host-managed-card-store".to_owned(),
        }))
    }

    pub fn get_audit_records(
        &self,
        request: RuntimeAuditRecordsRequest,
    ) -> RuntimeResult<RuntimeAuditRecordsResponse> {
        let records = self
            .audit_reader
            .runtime_audit_records()
            .map_err(|error| RuntimeErrorResponse::from_core(error).boxed())?
            .into_iter()
            .filter(|event| {
                request
                    .session_id
                    .as_ref()
                    .is_none_or(|session_id| &event.session_id == session_id)
                    && request
                        .skill_id
                        .as_ref()
                        .is_none_or(|skill_id| &event.skill_id == skill_id)
            })
            .map(RuntimeAuditEvent::from_event)
            .collect();
        Ok(RuntimeResponse::ok(RuntimeAuditRecordsResponse { records }))
    }

    pub fn cancel_operation(
        &self,
        request: RuntimeCancelOperationRequest,
    ) -> RuntimeResult<RuntimeCancelOperationResponse> {
        let cancelled = self
            .concurrency
            .cancel(&request.session, request.cancellation_token.clone())
            .map_err(|error| RuntimeErrorResponse::from_core(error).boxed())?;
        Ok(RuntimeResponse::ok(RuntimeCancelOperationResponse {
            session_id: request.session.session_id,
            cancellation_token: request.cancellation_token,
            cancelled,
            boundary: "runtime-cancellation-registry".to_owned(),
        }))
    }

    pub fn close_session(
        &self,
        request: RuntimeCloseSessionRequest,
    ) -> RuntimeResult<RuntimeCloseSessionResponse> {
        self.concurrency
            .close_session(&request.session)
            .map_err(|error| RuntimeErrorResponse::from_core(error).boxed())?;
        Ok(RuntimeResponse::ok(RuntimeCloseSessionResponse {
            session_id: request.session.session_id,
            closed: true,
            boundary: "runtime-session-manager".to_owned(),
        }))
    }

    fn emit_observability(&self, event: ObservabilityEvent) {
        self.observability.emit(event);
    }

    fn emit_session_event(&self, event: ObservabilityEvent, session: &RuntimeSessionContext) {
        self.emit_observability(observability_event_for_session(event, session));
    }

    fn emit_context_event(&self, event: ObservabilityEvent, context: &ApiCallContext) {
        self.emit_observability(observability_event_for_context(event, context));
    }

    fn record_metric(&self, metric: ObservabilityMetric) {
        self.metrics.record_metric(metric);
    }

    fn record_span(&self, span: TraceSpan) {
        self.metrics.record_span(span);
    }

    fn record_api_end(
        &self,
        trace_id: &str,
        span: &TraceContext,
        api_name: &str,
        outcome: &str,
        latency_ms: u64,
        risk_level: Option<RiskLevel>,
    ) {
        let mut latency = ObservabilityMetric::histogram_ms("dock.api_latency_ms", latency_ms)
            .with_label("api", api_name)
            .with_label("outcome", outcome)
            .with_trace_id(trace_id.to_owned());
        let mut total = ObservabilityMetric::counter("dock.api_call.total", 1)
            .with_label("phase", "end")
            .with_label("api", api_name)
            .with_label("outcome", outcome)
            .with_trace_id(trace_id.to_owned());
        if let Some(risk_level) = risk_level {
            latency = latency.with_label("riskLevel", risk_level.to_string());
            total = total.with_label("riskLevel", risk_level.to_string());
        }
        self.record_metric(latency);
        self.record_metric(total);
        self.record_span(
            TraceSpan::new(span.clone(), TraceSpanKind::ApiCall, "runtime.callApi")
                .with_attribute("api", api_name)
                .with_outcome(outcome)
                .with_latency_ms(latency_ms),
        );
    }

    fn record_render_end(
        &self,
        trace_id: &str,
        span: &TraceContext,
        component_path: &str,
        outcome: &str,
        latency_ms: u64,
    ) {
        self.record_metric(
            ObservabilityMetric::histogram_ms("dock.render_latency_ms", latency_ms)
                .with_label("component", component_path)
                .with_label("outcome", outcome)
                .with_trace_id(trace_id.to_owned()),
        );
        self.record_span(
            TraceSpan::new(
                span.clone(),
                TraceSpanKind::Render,
                "runtime.renderComponent",
            )
            .with_attribute("component", component_path)
            .with_outcome(outcome)
            .with_latency_ms(latency_ms),
        );
    }

    fn record_component_action_end(
        &self,
        trace_id: &str,
        span: &TraceContext,
        action: &str,
        outcome: &str,
        latency_ms: u64,
    ) {
        self.record_metric(
            ObservabilityMetric::counter("dock.component_event.total", 1)
                .with_label("action", action)
                .with_label("outcome", outcome)
                .with_trace_id(trace_id.to_owned()),
        );
        self.record_span(
            TraceSpan::new(
                span.clone(),
                TraceSpanKind::ComponentAction,
                "runtime.dispatchComponentAction",
            )
            .with_attribute("action", action)
            .with_outcome(outcome)
            .with_latency_ms(latency_ms),
        );
    }

    fn record_audit_span(
        &self,
        trace_id: &str,
        span: &TraceContext,
        api_name: &str,
        outcome: &str,
        risk_level: RiskLevel,
        latency_ms: u64,
    ) {
        self.record_span(
            TraceSpan::new(span.clone(), TraceSpanKind::Audit, "runtime.auditRecord")
                .with_attribute("api", api_name)
                .with_attribute("riskLevel", risk_level.to_string())
                .with_outcome(outcome)
                .with_latency_ms(latency_ms),
        );
        self.record_metric(
            ObservabilityMetric::histogram_ms("dock.audit_latency_ms", latency_ms)
                .with_label("api", api_name)
                .with_label("outcome", outcome)
                .with_label("riskLevel", risk_level.to_string())
                .with_trace_id(trace_id.to_owned()),
        );
    }

    fn emit_limit_if_needed(
        &self,
        session: &RuntimeSessionContext,
        api_name: &str,
        trace_id: &str,
        error: &DockCoreError,
        started: Instant,
    ) {
        if error.code() == ErrorCode::Timeout {
            self.emit_session_event(
                ObservabilityEvent::new(ObservabilityEventKind::SandboxLimitHit)
                    .with_trace_id(trace_id.to_owned())
                    .with_api_name(api_name.to_owned())
                    .with_outcome("timeout")
                    .with_latency_ms(elapsed_ms(started)),
                session,
            );
            self.record_metric(
                ObservabilityMetric::counter("dock.sandbox_limit.total", 1)
                    .with_label("limit", "timeout")
                    .with_label("api", api_name)
                    .with_trace_id(trace_id.to_owned()),
            );
        }
    }

    pub fn handle_ipc_request(&self, request: RuntimeIpcRequest) -> RuntimeIpcResponse {
        let started = Instant::now();
        let trace = TraceContext::root();
        if request.api_version != RUNTIME_API_VERSION {
            let response = RuntimeIpcResponse::error(
                request.request_id,
                request.method,
                RuntimeErrorResponse::new(
                    "unsupported_version",
                    "runtime API version is not supported",
                    None,
                ),
            );
            self.record_ipc_end(
                &trace,
                "runtime.unsupportedVersion",
                "unsupported_version",
                started,
            );
            return response;
        }

        let request_id = request.request_id;
        let method = request.method;
        let response = match method.as_str() {
            "runtime.negotiateVersion" => {
                match parse_ipc_params::<RuntimeIpcVersionParams>(&request.params) {
                    Ok(params) => runtime_ipc_response(
                        request_id,
                        method,
                        negotiate_runtime_version(params.requested_version.as_deref()),
                    ),
                    Err(error) => RuntimeIpcResponse::error(request_id, method, *error),
                }
            }
            "runtime.validateSkill" => {
                RuntimeIpcResponse::ok(request_id, method, self.validate_skill())
            }
            "runtime.loadSkill" => {
                RuntimeIpcResponse::ok(request_id, method, self.load_skill_response())
            }
            "runtime.hostContract" => {
                RuntimeIpcResponse::ok(request_id, method, self.host_contract())
            }
            "runtime.concurrencyPolicy" => {
                RuntimeIpcResponse::ok(request_id, method, self.concurrency_policy())
            }
            "runtime.callApi" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.call_api(params)),
            ),
            "runtime.renderComponent" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.render_component(params)),
            ),
            "runtime.dispatchComponentAction" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params)
                    .and_then(|params| self.dispatch_component_action(params)),
            ),
            "runtime.expireCards" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.expire_cards(params)),
            ),
            "runtime.getAuditRecords" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.get_audit_records(params)),
            ),
            "runtime.cancelOperation" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.cancel_operation(params)),
            ),
            "runtime.closeSession" => runtime_ipc_response(
                request_id,
                method,
                parse_ipc_params(&request.params).and_then(|params| self.close_session(params)),
            ),
            _ => RuntimeIpcResponse::error(
                request_id,
                method,
                RuntimeErrorResponse::invalid_method("runtime method is not supported"),
            ),
        };
        let method = response.method.clone();
        let outcome = response
            .error
            .as_ref()
            .map(|error| error.code.as_str())
            .unwrap_or(response.status.as_str());
        self.record_ipc_end(&trace, &method, outcome, started);
        response
    }

    fn record_ipc_end(&self, trace: &TraceContext, method: &str, outcome: &str, started: Instant) {
        let latency = elapsed_ms(started);
        self.record_metric(
            ObservabilityMetric::histogram_ms("dock.runtime_ipc_latency_ms", latency)
                .with_label("method", method)
                .with_label("outcome", outcome)
                .with_trace_id(trace.trace_id.clone()),
        );
        self.record_span(
            TraceSpan::new(trace.clone(), TraceSpanKind::RuntimeIpc, "runtime.ipc")
                .with_attribute("method", method)
                .with_outcome(outcome)
                .with_latency_ms(latency),
        );
    }
}

fn skill_id(skill: &LoadedSkill) -> String {
    skill
        .manifest
        .extra
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned()
}

fn package_ref(skill: &LoadedSkill) -> String {
    if skill.integrity.digest.value.is_empty() {
        "local-dev-package".to_owned()
    } else {
        format!("sha256:{}", skill.integrity.digest.value)
    }
}

fn redact_text(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for marker in [
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
        "key-",
        "/home/",
        "/users/",
        "\\users\\",
        "c:\\",
        "file:",
    ] {
        if lower.contains(marker) {
            return "[REDACTED]".to_owned();
        }
    }
    value.to_owned()
}

fn redacted_validation_report(report: &ValidationReport) -> ValidationReport {
    ValidationReport {
        errors: report
            .errors
            .iter()
            .map(redacted_validation_issue)
            .collect(),
        warnings: report
            .warnings
            .iter()
            .map(redacted_validation_issue)
            .collect(),
    }
}

fn redacted_validation_issue(issue: &ValidationIssue) -> ValidationIssue {
    ValidationIssue {
        level: issue.level,
        category: issue.category,
        path: redact_text(&issue.path),
        message: redact_text(&issue.message),
        suggestion: issue.suggestion.as_deref().map(redact_text),
    }
}

fn runtime_ipc_response<T: Serialize>(
    request_id: String,
    method: String,
    result: RuntimeResult<T>,
) -> RuntimeIpcResponse {
    match result {
        Ok(response) => RuntimeIpcResponse::ok(request_id, method, response),
        Err(error) => RuntimeIpcResponse::error(request_id, method, *error),
    }
}

fn parse_ipc_params<T: DeserializeOwned>(params: &Value) -> Result<T, RuntimeError> {
    let params = if params.is_null() {
        empty_object()
    } else {
        params.clone()
    };
    serde_json::from_value(params).map_err(|error| {
        RuntimeErrorResponse::invalid_params(format!("runtime IPC params are invalid: {error}"))
            .boxed()
    })
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn trace_id_for(_session: &RuntimeSessionContext, _api_name: &str) -> String {
    next_observability_id("trace")
}

fn trace_context_for(operation: &Option<RuntimeOperationOptions>) -> TraceContext {
    operation
        .as_ref()
        .and_then(|operation| operation.trace.as_ref())
        .map(|trace| {
            let context = TraceContext::new(trace.trace_id.clone(), next_observability_id("span"));
            match trace.parent_span_id.as_ref() {
                Some(parent_span_id) => context.with_parent_span_id(parent_span_id.clone()),
                None => context,
            }
        })
        .unwrap_or_else(TraceContext::root)
}

fn operation_with_trace(
    operation: Option<RuntimeOperationOptions>,
    trace: &TraceContext,
) -> Option<RuntimeOperationOptions> {
    Some(RuntimeOperationOptions {
        trace: Some(RuntimeTraceContext {
            trace_id: trace.trace_id.clone(),
            parent_span_id: Some(trace.span_id.clone()),
        }),
        ..operation.unwrap_or_default()
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    let millis = started.elapsed().as_millis();
    millis.min(u128::from(u64::MAX)) as u64
}

fn observability_event_for_session(
    event: ObservabilityEvent,
    session: &RuntimeSessionContext,
) -> ObservabilityEvent {
    let event = event
        .with_session_id(session.session_id.clone())
        .with_skill_id(session.skill_id.clone())
        .with_merchant_did(session.merchant_did.clone())
        .with_user_did_hash(session.user_did.as_deref());
    if event.trace_id.is_none() {
        let api_name = event.api_name.clone();
        event.with_trace_id(trace_id_for(session, api_name.as_deref().unwrap_or("")))
    } else {
        event
    }
}

fn observability_event_for_context(
    event: ObservabilityEvent,
    context: &ApiCallContext,
) -> ObservabilityEvent {
    let event = event
        .with_session_id(context.session_id.clone())
        .with_skill_id(context.skill_id.clone())
        .with_api_name(context.api_name.clone())
        .with_merchant_did(context.merchant_did.clone())
        .with_user_did_hash(context.user_did.as_deref());
    if event.trace_id.is_none() {
        event.with_trace_id(next_observability_id("trace"))
    } else {
        event
    }
}

#[allow(dead_code)]
fn core_error(code: ErrorCode, message: impl Into<String>) -> RuntimeErrorResponse {
    RuntimeErrorResponse::from_core(DockCoreError::core(code, message))
}
