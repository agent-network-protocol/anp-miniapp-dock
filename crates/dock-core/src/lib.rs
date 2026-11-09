#![doc = "Core orchestrator, API registry, host boundary, and shared error crate."]

pub mod api_registry;
pub mod config;
pub mod error;
pub mod host;
pub mod observability;
pub mod orchestrator;
pub mod runtime;

pub use api_registry::{ApiRegistry, RegisteredApi};
pub use config::{
    redact_runtime_config_text, ConfigReference, HostProviderConfig, NetworkAllowlistReference,
    PathReference, ProviderReference, RuntimeAllowlistConfig, RuntimeConfig, RuntimeConfigIssue,
    RuntimeConfigIssueSeverity, RuntimeConfigLoadError, RuntimeConfigReleaseBlocker,
    RuntimeConfigSource, RuntimeConfigValidation, RuntimeDataBackendConfig, RuntimeDataBackendKind,
    RuntimeIdentityConfig, RuntimeMockProviderFlags, RuntimeObservabilityConfig,
    RuntimeObservabilityLevel, RuntimeProfile, RuntimeResolverConfig, RuntimeTokenIssuerConfig,
    SecretReference, RUNTIME_CONFIG_LOAD_PRIORITY, RUNTIME_CONFIG_REDACTION_MARKER,
    RUNTIME_CONFIG_REDACTION_POLICY, RUNTIME_CONFIG_SCHEMA_VERSION,
};
pub use error::{DockCoreError, ErrorCode};
pub use host::{
    canonicalize_open_detail_page_target, ApiExecutor, AuditEvent, AuditSink, ConsentDecision,
    ConsentGate, HeadlessHostAdapter, HostActionOutcome, HostActionRedaction, HostActionRequest,
    HostActionRoute, HostActionStatus, HostAdapterContract, HostCapabilityDeclaration,
    HostCapabilityRequirement, HostCapabilityStatus, HostConsentGateAdapter, PermissionDecision,
    PermissionDecisionSummary, RenderOutcome, RenderRouter, RuntimeHost,
    HOST_ACTION_REDACTION_POLICY, HOST_ADAPTER_CONTRACT_VERSION,
};
pub use observability::{
    hash_user_did, low_cardinality_label, next_observability_id, redact_observability_text,
    redact_observability_value, InMemoryMetricsSink, InMemoryObservabilitySink, MetricKind,
    MetricsSink, NoopMetricsSink, NoopObservabilitySink, ObservabilityEvent,
    ObservabilityEventKind, ObservabilityMetric, ObservabilityRedaction, ObservabilitySeverity,
    ObservabilitySink, TraceContext, TraceSpan, TraceSpanKind, DEFAULT_RUNTIME_VERSION,
    OBSERVABILITY_EVENT_SCHEMA_VERSION, OBSERVABILITY_METRIC_SCHEMA_VERSION,
    OBSERVABILITY_REDACTION_MARKER, OBSERVABILITY_REDACTION_POLICY,
    OBSERVABILITY_TRACE_SCHEMA_VERSION,
};
pub use orchestrator::{
    ApiCallContext, CallOutcome, ComponentAction, ComponentRenderInput, Orchestrator,
};
pub use runtime::{
    load_skill_path, negotiate_runtime_version, validate_skill_path, RuntimeAuditEvent,
    RuntimeAuditReader, RuntimeAuditRecordsRequest, RuntimeAuditRecordsResponse,
    RuntimeCallRequest, RuntimeCallResponse, RuntimeCancelOperationRequest,
    RuntimeCancelOperationResponse, RuntimeCloseSessionRequest, RuntimeCloseSessionResponse,
    RuntimeComponentAction, RuntimeConcurrencyPolicy, RuntimeConcurrencyPolicyResponse,
    RuntimeDispatchComponentActionRequest, RuntimeDispatchComponentActionResponse, RuntimeErrorDto,
    RuntimeErrorResponse, RuntimeExpireCardsRequest, RuntimeExpireCardsResponse,
    RuntimeHostContractResponse, RuntimeIdempotencyPolicy, RuntimeIpcRedaction, RuntimeIpcRequest,
    RuntimeIpcResponse, RuntimeIpcTransport, RuntimeIpcVersionParams, RuntimeLoadSkillResponse,
    RuntimeOperationOptions, RuntimePersistentAuditSink, RuntimeRenderComponentRequest,
    RuntimeRenderComponentResponse, RuntimeResponse, RuntimeResult, RuntimeRetryPolicy,
    RuntimeService, RuntimeServiceObservabilityParts, RuntimeServiceParts, RuntimeSessionContext,
    RuntimeSkillSummary, RuntimeTraceContext, RuntimeValidateSkillResponse, RuntimeVersion,
    RUNTIME_API_VERSION, RUNTIME_IPC_BINDING, RUNTIME_IPC_TRANSPORT,
};
