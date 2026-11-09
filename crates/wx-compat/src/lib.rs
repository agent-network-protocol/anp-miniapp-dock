#![doc = "wx Compatibility Layer host capability and scoped storage crate."]

pub mod high_risk;
pub mod model_context;
pub mod permissions;
pub mod request;
pub mod storage;
pub mod unsupported;

pub use high_risk::{
    call_high_risk_api_with_boundary, high_risk_api_spec, high_risk_audit_summary,
    high_risk_consent_required_json, high_risk_error_json, high_risk_success_json,
    redact_high_risk_value, DevOnlyHighRiskHostProvider, HighRiskApiError, HighRiskApiKind,
    HighRiskApiRequest, HighRiskApiSpec, HighRiskApiSuccess, HighRiskHostProvider, HighRiskLevel,
    UnavailableHighRiskHostProvider,
};
pub use model_context::{
    default_app_base_info_js_literal, default_device_info_js_literal, notification_type_js_literal,
    AppBaseInfo, CardEvent, CardEventSink, DeviceInfo, InMemoryCardEventSink, ModelContext,
    RelatedPage, NOTIFICATION_TYPE_EXPIRE, NOTIFICATION_TYPE_INPUT, NOTIFICATION_TYPE_OVERFLOW,
    NOTIFICATION_TYPE_RESULT,
};
pub use permissions::{
    Capability, CapabilityProfile, HostPermissionOverride, PermissionDecision,
    PermissionDecisionSummary, PermissionPolicyEngine, PermissionPolicyInput, PermissionReasonCode,
    PermissionSource, RuntimeProfile, WxEnvironmentKind,
};
pub use request::{
    RequestBroker, UnsupportedRequestBroker, WxMethod, WxRequest, WxRequestError, WxResponse,
};
pub use storage::{
    InMemoryScopedStorage, LocalFileScopedStorageBackend, PersistentScopedStorage,
    PersistentStorageEntry, ScopedStorage, ScopedStoragePersistenceBackend, StorageError,
    StoragePersistenceProfile, StoragePersistenceSnapshot, StorageRedaction,
    StorageRestoreRejection, StorageRestoreRejectionReason, StorageRestoreReport, StorageScope,
    StorageScopeSummary, DEFAULT_MAX_STORAGE_KEY_BYTES, DEFAULT_MAX_STORAGE_SCOPE_BYTES,
    DEFAULT_MAX_STORAGE_VALUE_BYTES, DEFAULT_STORAGE_NAMESPACE,
};
pub use unsupported::{
    unsupported_api, unsupported_api_registry, unsupported_api_registry_js_literal, UnsupportedApi,
    UnsupportedApiKind,
};
