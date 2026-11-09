use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionSource {
    RuntimeProfile,
    Manifest,
    MetaAnp,
    XAnp,
    ComponentDynamicScope,
    HostOverride,
    MerchantTrustPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfile {
    Production,
    Development,
    Headless,
}

impl RuntimeProfile {
    pub fn allows_dev_only(self) -> bool {
        matches!(self, Self::Development | Self::Headless)
    }
}

impl Default for RuntimeProfile {
    fn default() -> Self {
        Self::Production
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostPermissionOverride {
    Allow,
    Deny,
    Prompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionReasonCode {
    RuntimeProfileAllowed,
    ManifestPermissionAllowed,
    MetaAnpPermissionAllowed,
    XAnpPermissionAllowed,
    DynamicScopeAllowed,
    HostOverrideAllow,
    HostOverrideDeny,
    HostOverridePrompt,
    MerchantTrustDenied,
    CapabilityNotDeclared,
    MockDevOnlyAllowed,
    MockProductionDenied,
    NetworkAllowlistAllowed,
    NetworkAllowlistDenied,
}

impl PermissionReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeProfileAllowed => "runtime_profile_allowed",
            Self::ManifestPermissionAllowed => "manifest_permission_allowed",
            Self::MetaAnpPermissionAllowed => "meta_anp_permission_allowed",
            Self::XAnpPermissionAllowed => "x_anp_permission_allowed",
            Self::DynamicScopeAllowed => "dynamic_scope_allowed",
            Self::HostOverrideAllow => "host_override_allow",
            Self::HostOverrideDeny => "host_override_deny",
            Self::HostOverridePrompt => "host_override_prompt",
            Self::MerchantTrustDenied => "merchant_trust_denied",
            Self::CapabilityNotDeclared => "capability_not_declared",
            Self::MockDevOnlyAllowed => "mock_dev_only_allowed",
            Self::MockProductionDenied => "mock_production_denied",
            Self::NetworkAllowlistAllowed => "network_allowlist_allowed",
            Self::NetworkAllowlistDenied => "network_allowlist_denied",
        }
    }
}

impl fmt::Display for PermissionReasonCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
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
        reason_code: PermissionReasonCode,
        reason: String,
    },
    Prompt {
        capability: Capability,
        reason_code: PermissionReasonCode,
        reason: String,
    },
    MockAllowed {
        capability: Capability,
        reason_code: PermissionReasonCode,
        reason: String,
        dev_only: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecisionSummary {
    pub decision: String,
    pub capability: Capability,
    pub reason_code: String,
    pub reason: String,
    pub dev_only: bool,
}

impl PermissionDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow | Self::MockAllowed { .. })
    }

    pub fn deny(
        capability: Capability,
        reason_code: PermissionReasonCode,
        reason: impl Into<String>,
    ) -> Self {
        Self::Deny {
            capability,
            reason_code,
            reason: reason.into(),
        }
    }

    pub fn prompt(
        capability: Capability,
        reason_code: PermissionReasonCode,
        reason: impl Into<String>,
    ) -> Self {
        Self::Prompt {
            capability,
            reason_code,
            reason: reason.into(),
        }
    }

    pub fn mock_allowed(
        capability: Capability,
        reason_code: PermissionReasonCode,
        reason: impl Into<String>,
    ) -> Self {
        Self::MockAllowed {
            capability,
            reason_code,
            reason: reason.into(),
            dev_only: true,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allow => None,
            Self::Deny { reason, .. }
            | Self::Prompt { reason, .. }
            | Self::MockAllowed { reason, .. } => Some(reason.as_str()),
        }
    }

    pub fn summary(&self, capability: Capability) -> PermissionDecisionSummary {
        match self {
            Self::Allow => PermissionDecisionSummary {
                decision: "allow".to_owned(),
                capability,
                reason_code: PermissionReasonCode::RuntimeProfileAllowed
                    .as_str()
                    .to_owned(),
                reason: "capability allowed".to_owned(),
                dev_only: false,
            },
            Self::Deny {
                capability,
                reason_code,
                reason,
            } => PermissionDecisionSummary {
                decision: "deny".to_owned(),
                capability: *capability,
                reason_code: reason_code.as_str().to_owned(),
                reason: reason.clone(),
                dev_only: false,
            },
            Self::Prompt {
                capability,
                reason_code,
                reason,
            } => PermissionDecisionSummary {
                decision: "prompt".to_owned(),
                capability: *capability,
                reason_code: reason_code.as_str().to_owned(),
                reason: reason.clone(),
                dev_only: false,
            },
            Self::MockAllowed {
                capability,
                reason_code,
                reason,
                dev_only,
            } => PermissionDecisionSummary {
                decision: "mock_allowed".to_owned(),
                capability: *capability,
                reason_code: reason_code.as_str().to_owned(),
                reason: reason.clone(),
                dev_only: *dev_only,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    environment: WxEnvironmentKind,
    grants: BTreeMap<Capability, BTreeSet<PermissionSource>>,
    runtime_profile: RuntimeProfile,
    merchant_trusted: bool,
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
            self.grant(Capability::Request, PermissionSource::ComponentDynamicScope);
        }
        self
    }

    pub fn with_dynamic_component_timer(mut self) -> Self {
        if self.environment == WxEnvironmentKind::Component {
            self.grant(Capability::Timer, PermissionSource::ComponentDynamicScope);
        }
        self
    }

    pub fn with_manifest_permission(mut self, capability: Capability) -> Self {
        self.grant(capability, PermissionSource::Manifest);
        self
    }

    pub fn with_meta_anp_permission(mut self, capability: Capability) -> Self {
        self.grant(capability, PermissionSource::MetaAnp);
        self
    }

    pub fn with_x_anp_permission(mut self, capability: Capability) -> Self {
        self.grant(capability, PermissionSource::XAnp);
        self
    }

    pub fn with_runtime_profile(mut self, runtime_profile: RuntimeProfile) -> Self {
        self.runtime_profile = runtime_profile;
        self
    }

    pub fn with_merchant_trust(mut self, merchant_trusted: bool) -> Self {
        self.merchant_trusted = merchant_trusted;
        self
    }

    pub fn new(
        environment: WxEnvironmentKind,
        allowed: impl IntoIterator<Item = Capability>,
    ) -> Self {
        let mut profile = Self {
            environment,
            grants: BTreeMap::new(),
            runtime_profile: RuntimeProfile::Production,
            merchant_trusted: true,
        };
        for capability in allowed {
            profile.grant(capability, PermissionSource::RuntimeProfile);
        }
        profile
    }

    fn grant(&mut self, capability: Capability, source: PermissionSource) {
        self.grants.entry(capability).or_default().insert(source);
    }

    pub fn has_grant(&self, capability: Capability, source: PermissionSource) -> bool {
        self.grants
            .get(&capability)
            .is_some_and(|sources| sources.contains(&source))
    }

    pub fn sources_for(&self, capability: Capability) -> BTreeSet<PermissionSource> {
        self.grants.get(&capability).cloned().unwrap_or_default()
    }

    pub fn policy_input(&self, capability: Capability) -> PermissionPolicyInput {
        let sources = self.sources_for(capability);
        self.to_policy_input(capability, sources)
    }

    pub fn environment(&self) -> WxEnvironmentKind {
        self.environment
    }

    pub fn check(&self, capability: Capability) -> PermissionDecision {
        PermissionPolicyEngine.decide(self.policy_input(capability))
    }

    pub fn ensure(&self, capability: Capability) -> Result<(), PermissionDecision> {
        match self.check(capability) {
            PermissionDecision::Allow => Ok(()),
            denial => Err(denial),
        }
    }

    pub fn allowed_capabilities(&self) -> impl Iterator<Item = Capability> + '_ {
        self.grants.keys().copied()
    }

    fn to_policy_input(
        &self,
        capability: Capability,
        sources: BTreeSet<PermissionSource>,
    ) -> PermissionPolicyInput {
        PermissionPolicyInput {
            capability,
            environment: self.environment,
            manifest_declared: sources.contains(&PermissionSource::Manifest)
                || sources.contains(&PermissionSource::RuntimeProfile),
            meta_anp_declared: sources.contains(&PermissionSource::MetaAnp),
            x_anp_declared: sources.contains(&PermissionSource::XAnp),
            dynamic_scope_declared: sources.contains(&PermissionSource::ComponentDynamicScope),
            host_override: None,
            runtime_profile: self.runtime_profile,
            mock_provider: false,
            merchant_trusted: self.merchant_trusted,
            requires_prompt: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPolicyInput {
    pub capability: Capability,
    pub environment: WxEnvironmentKind,
    pub manifest_declared: bool,
    pub meta_anp_declared: bool,
    pub x_anp_declared: bool,
    pub dynamic_scope_declared: bool,
    pub host_override: Option<HostPermissionOverride>,
    pub runtime_profile: RuntimeProfile,
    pub mock_provider: bool,
    pub merchant_trusted: bool,
    pub requires_prompt: bool,
}

impl PermissionPolicyInput {
    pub fn new(capability: Capability, environment: WxEnvironmentKind) -> Self {
        Self {
            capability,
            environment,
            manifest_declared: false,
            meta_anp_declared: false,
            x_anp_declared: false,
            dynamic_scope_declared: false,
            host_override: None,
            runtime_profile: RuntimeProfile::Production,
            mock_provider: false,
            merchant_trusted: true,
            requires_prompt: false,
        }
    }

    pub fn with_manifest_declared(mut self, declared: bool) -> Self {
        self.manifest_declared = declared;
        self
    }

    pub fn with_meta_anp_declared(mut self, declared: bool) -> Self {
        self.meta_anp_declared = declared;
        self
    }

    pub fn with_x_anp_declared(mut self, declared: bool) -> Self {
        self.x_anp_declared = declared;
        self
    }

    pub fn with_dynamic_scope_declared(mut self, declared: bool) -> Self {
        self.dynamic_scope_declared = declared;
        self
    }

    pub fn with_host_override(mut self, override_decision: HostPermissionOverride) -> Self {
        self.host_override = Some(override_decision);
        self
    }

    pub fn with_runtime_profile(mut self, runtime_profile: RuntimeProfile) -> Self {
        self.runtime_profile = runtime_profile;
        self
    }

    pub fn with_mock_provider(mut self, mock_provider: bool) -> Self {
        self.mock_provider = mock_provider;
        self
    }

    pub fn with_merchant_trust(mut self, merchant_trusted: bool) -> Self {
        self.merchant_trusted = merchant_trusted;
        self
    }

    pub fn with_prompt_required(mut self, requires_prompt: bool) -> Self {
        self.requires_prompt = requires_prompt;
        self
    }

    pub fn with_manifest_permissions_value(mut self, permissions: &Value) -> Self {
        self.manifest_declared |= value_declares_capability(permissions, self.capability);
        self
    }

    pub fn with_meta_anp_value(mut self, meta_anp: &Value) -> Self {
        self.meta_anp_declared |= value_declares_capability(meta_anp, self.capability);
        self
    }

    pub fn with_x_anp_value(mut self, x_anp: &Value) -> Self {
        self.x_anp_declared |= value_declares_capability(x_anp, self.capability);
        self
    }

    pub fn with_component_dynamic_scope_value(mut self, dynamic_scope: &Value) -> Self {
        self.dynamic_scope_declared |= value_declares_dynamic_scope(dynamic_scope);
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PermissionPolicyEngine;

impl PermissionPolicyEngine {
    pub fn decide(&self, input: PermissionPolicyInput) -> PermissionDecision {
        if matches!(input.host_override, Some(HostPermissionOverride::Deny)) {
            return PermissionDecision::deny(
                input.capability,
                PermissionReasonCode::HostOverrideDeny,
                "Host policy denied this capability",
            );
        }

        if !input.merchant_trusted {
            return PermissionDecision::deny(
                input.capability,
                PermissionReasonCode::MerchantTrustDenied,
                "merchant trust policy denied this capability",
            );
        }

        if input.mock_provider {
            if input.runtime_profile.allows_dev_only() && input.has_declared_permission() {
                return PermissionDecision::mock_allowed(
                    input.capability,
                    PermissionReasonCode::MockDevOnlyAllowed,
                    "mock provider allowed only in explicit dev/headless runtime profile",
                );
            }
            return PermissionDecision::deny(
                input.capability,
                PermissionReasonCode::MockProductionDenied,
                "mock provider is denied outside explicit dev/headless runtime profile",
            );
        }

        let Some(reason_code) = declared_reason_code(&input) else {
            return PermissionDecision::deny(
                input.capability,
                PermissionReasonCode::CapabilityNotDeclared,
                format!(
                    "{} is not declared for {:?} environment",
                    input.capability, input.environment
                ),
            );
        };

        match input.host_override {
            Some(HostPermissionOverride::Allow) => return PermissionDecision::Allow,
            Some(HostPermissionOverride::Prompt) => {
                return PermissionDecision::prompt(
                    input.capability,
                    PermissionReasonCode::HostOverridePrompt,
                    "Host policy requires explicit prompt before this capability can run",
                );
            }
            Some(HostPermissionOverride::Deny) => unreachable!("handled above"),
            None => {}
        }

        if input.requires_prompt {
            return PermissionDecision::prompt(
                input.capability,
                reason_code,
                "declared capability requires explicit prompt before execution",
            );
        }

        PermissionDecision::Allow
    }
}

impl PermissionPolicyInput {
    fn has_declared_permission(&self) -> bool {
        declared_reason_code(self).is_some()
    }
}

fn declared_reason_code(input: &PermissionPolicyInput) -> Option<PermissionReasonCode> {
    if input.environment == WxEnvironmentKind::Component
        && matches!(input.capability, Capability::Request | Capability::Timer)
    {
        return input
            .dynamic_scope_declared
            .then_some(PermissionReasonCode::DynamicScopeAllowed);
    }

    if input.manifest_declared {
        return Some(PermissionReasonCode::ManifestPermissionAllowed);
    }
    if input.meta_anp_declared {
        return Some(PermissionReasonCode::MetaAnpPermissionAllowed);
    }
    if input.x_anp_declared {
        return Some(PermissionReasonCode::XAnpPermissionAllowed);
    }
    None
}

fn value_declares_capability(value: &Value, capability: Capability) -> bool {
    match value {
        Value::Bool(_) => false,
        Value::String(label) => label_declares_capability(label, capability),
        Value::Array(items) => items
            .iter()
            .any(|item| value_declares_capability(item, capability)),
        Value::Object(map) => {
            capability_label_keys(capability).iter().any(|key| {
                map.get(*key)
                    .is_some_and(|value| !matches!(value, Value::Bool(false) | Value::Null))
            }) || [
                "permissions",
                "capabilities",
                "capability",
                "wx",
                "apis",
                "scopes",
            ]
            .iter()
            .any(|key| {
                map.get(*key)
                    .is_some_and(|value| value_declares_capability(value, capability))
            })
        }
        Value::Null | Value::Number(_) => false,
    }
}

fn value_declares_dynamic_scope(value: &Value) -> bool {
    match value {
        Value::Bool(enabled) => *enabled,
        Value::Object(map) => map.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        _ => false,
    }
}

fn label_declares_capability(label: &str, capability: Capability) -> bool {
    let normalized = normalize_permission_label(label);
    capability_label_keys(capability)
        .iter()
        .any(|key| normalize_permission_label(key) == normalized)
}

fn capability_label_keys(capability: Capability) -> &'static [&'static str] {
    match capability {
        Capability::ModelContext => &["model_context", "modelContext", "wx.modelContext"],
        Capability::Storage => &["storage", "wx.storage", "wx.getStorage", "wx.setStorage"],
        Capability::Request => &["request", "wx.request", "network"],
        Capability::Timer => &["timer", "timers", "setTimeout", "setInterval"],
        Capability::DeviceInfo => &["device_info", "deviceInfo", "wx.getDeviceInfo"],
        Capability::AppBaseInfo => &["app_base_info", "appBaseInfo", "wx.getAppBaseInfo"],
        Capability::Login => &["login", "wx.login", "wx.checkSession"],
        Capability::Payment => &["payment", "pay", "wx.requestPayment"],
    }
}

fn normalize_permission_label(label: &str) -> String {
    label
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', '.', ' '], "")
}
