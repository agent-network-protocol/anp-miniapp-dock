use crate::error::DockCoreError;
use crate::orchestrator::{ApiCallContext, ComponentRenderInput};
use mcp_schema::AtomicApiResult;
use serde_json::Value;

pub trait RuntimeHost {
    fn check_permission(
        &self,
        context: &ApiCallContext,
    ) -> Result<PermissionDecision, DockCoreError>;
}

pub trait ConsentGate {
    fn check_consent(&self, context: &ApiCallContext) -> Result<ConsentDecision, DockCoreError>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub session_id: String,
    pub skill_id: String,
    pub api_name: String,
    pub outcome: String,
}
