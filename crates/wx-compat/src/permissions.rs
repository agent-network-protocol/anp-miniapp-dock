use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WxEnvironmentKind {
    AtomicApi,
    Component,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Capability {
    ModelContext,
    Storage,
    Request,
    Timer,
    DeviceInfo,
    AppBaseInfo,
    Login,
    Payment,
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ModelContext => "model_context",
            Self::Storage => "storage",
            Self::Request => "request",
            Self::Timer => "timer",
            Self::DeviceInfo => "device_info",
            Self::AppBaseInfo => "app_base_info",
            Self::Login => "login",
            Self::Payment => "payment",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny {
        capability: Capability,
        reason: String,
    },
}

impl PermissionDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    environment: WxEnvironmentKind,
    allowed: BTreeSet<Capability>,
}

impl CapabilityProfile {
    pub fn atomic_api() -> Self {
        Self::new(
            WxEnvironmentKind::AtomicApi,
            [
                Capability::ModelContext,
                Capability::Storage,
                Capability::Request,
                Capability::DeviceInfo,
                Capability::AppBaseInfo,
                Capability::Login,
            ],
        )
    }

    pub fn component() -> Self {
        Self::new(
            WxEnvironmentKind::Component,
            [
                Capability::ModelContext,
                Capability::Storage,
                Capability::DeviceInfo,
                Capability::AppBaseInfo,
            ],
        )
    }

    pub fn with_dynamic_component_request(mut self) -> Self {
        if self.environment == WxEnvironmentKind::Component {
            self.allowed.insert(Capability::Request);
        }
        self
    }

    pub fn new(
        environment: WxEnvironmentKind,
        allowed: impl IntoIterator<Item = Capability>,
    ) -> Self {
        Self {
            environment,
            allowed: allowed.into_iter().collect(),
        }
    }

    pub fn environment(&self) -> WxEnvironmentKind {
        self.environment
    }

    pub fn check(&self, capability: Capability) -> PermissionDecision {
        if self.allowed.contains(&capability) {
            PermissionDecision::Allow
        } else {
            PermissionDecision::Deny {
                capability,
                reason: format!(
                    "{capability} is not allowed in {:?} environment",
                    self.environment
                ),
            }
        }
    }

    pub fn ensure(&self, capability: Capability) -> Result<(), PermissionDecision> {
        match self.check(capability) {
            PermissionDecision::Allow => Ok(()),
            denial => Err(denial),
        }
    }

    pub fn allowed_capabilities(&self) -> impl Iterator<Item = Capability> + '_ {
        self.allowed.iter().copied()
    }
}
