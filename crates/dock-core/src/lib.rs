#![doc = "Core orchestrator, API registry, host boundary, and shared error crate."]

pub mod api_registry;
pub mod error;
pub mod host;
pub mod orchestrator;

pub use api_registry::{ApiRegistry, RegisteredApi};
pub use error::{DockCoreError, ErrorCode};
pub use host::{
    ApiExecutor, AuditEvent, AuditSink, ConsentDecision, ConsentGate, PermissionDecision,
    RenderOutcome, RenderRouter, RuntimeHost,
};
pub use orchestrator::{
    ApiCallContext, CallOutcome, ComponentAction, ComponentRenderInput, Orchestrator,
};
