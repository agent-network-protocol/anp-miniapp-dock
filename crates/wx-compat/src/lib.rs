#![doc = "wx Compatibility Layer host capability and scoped storage crate."]

pub mod model_context;
pub mod permissions;
pub mod request;
pub mod storage;

pub use model_context::{
    CardEvent, CardEventSink, DeviceInfo, InMemoryCardEventSink, ModelContext, RelatedPage,
};
pub use permissions::{Capability, CapabilityProfile, PermissionDecision, WxEnvironmentKind};
pub use request::{
    unsupported_api, RequestBroker, UnsupportedRequestBroker, WxMethod, WxRequest, WxRequestError,
    WxResponse,
};
pub use storage::{InMemoryScopedStorage, ScopedStorage, StorageError, StorageScope};
