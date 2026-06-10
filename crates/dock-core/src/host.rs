use crate::error::DockCoreError;
use crate::orchestrator::{ApiCallContext, ComponentRenderInput};
use consent_audit::{ConsentProof, ConsentRequest, RiskLevel};
use mcp_schema::AtomicApiResult;
use serde_json::Value;

pub trait RuntimeHost {
    fn check_permission(
        &self,
        context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError>;
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
    fn record(&self, event: AuditEvent) -> Result<(), DockCoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentDecision {
    Approved,
    Required(String),
}

#[derive(Debug, Clone, PartialEq)]
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
    pub consent_proof: Option<ConsentProof>,
    pub outcome: String,
}
