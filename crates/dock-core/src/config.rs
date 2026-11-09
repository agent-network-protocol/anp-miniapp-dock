use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const RUNTIME_CONFIG_SCHEMA_VERSION: &str = "dock.runtime.config.v1";
pub const RUNTIME_CONFIG_REDACTION_POLICY: &str = "dock.runtime.config-redaction.v1";
pub const RUNTIME_CONFIG_REDACTION_MARKER: &str = "[REDACTED]";

pub const RUNTIME_CONFIG_LOAD_PRIORITY: [RuntimeConfigSource; 5] = [
    RuntimeConfigSource::BuiltInDefault,
    RuntimeConfigSource::ConfigFile,
    RuntimeConfigSource::Environment,
    RuntimeConfigSource::CliArgument,
    RuntimeConfigSource::HostOverride,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeConfigSource {
    BuiltInDefault,
    ConfigFile,
    Environment,
    CliArgument,
    HostOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub profile: RuntimeProfile,
    #[serde(default)]
    pub identity: RuntimeIdentityConfig,
    #[serde(default)]
    pub resolver: RuntimeResolverConfig,
    #[serde(default)]
    pub allowlist: RuntimeAllowlistConfig,
    #[serde(default)]
    pub token_issuer: RuntimeTokenIssuerConfig,
    #[serde(default)]
    pub storage: RuntimeDataBackendConfig,
    #[serde(default)]
    pub audit: RuntimeDataBackendConfig,
    #[serde(default)]
    pub cache: RuntimeDataBackendConfig,
    #[serde(default)]
    pub host_providers: Vec<HostProviderConfig>,
    #[serde(default)]
    pub mock_providers: RuntimeMockProviderFlags,
    #[serde(default)]
    pub observability: RuntimeObservabilityConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            profile: RuntimeProfile::default(),
            identity: RuntimeIdentityConfig::default(),
            resolver: RuntimeResolverConfig::default(),
            allowlist: RuntimeAllowlistConfig::default(),
            token_issuer: RuntimeTokenIssuerConfig::default(),
            storage: RuntimeDataBackendConfig::default(),
            audit: RuntimeDataBackendConfig::default(),
            cache: RuntimeDataBackendConfig::default(),
            host_providers: Vec::new(),
            mock_providers: RuntimeMockProviderFlags::default(),
            observability: RuntimeObservabilityConfig::default(),
        }
    }
}

impl RuntimeConfig {
    pub fn from_json_str(input: &str) -> Result<Self, RuntimeConfigLoadError> {
        serde_json::from_str(input).map_err(|error| RuntimeConfigLoadError {
            message: redact_runtime_config_text(&error.to_string()),
        })
    }

    pub fn validate(&self) -> RuntimeConfigValidation {
        let mut validation = RuntimeConfigValidation::new(self.profile);

        if self.schema_version != RUNTIME_CONFIG_SCHEMA_VERSION {
            validation.error(
                "unsupported_config_schema",
                "schemaVersion",
                "runtime config schemaVersion is not supported",
            );
            if self.profile == RuntimeProfile::Production {
                validation.blocker(
                    "unsupported_config_schema",
                    "schemaVersion",
                    "production profile requires the current runtime config schema",
                );
            }
        }

        validate_identity(&self.identity, &mut validation);
        validate_resolver(&self.resolver, &mut validation);
        validate_allowlist(&self.allowlist, &mut validation);
        validate_token_issuer(&self.token_issuer, &mut validation);
        validate_data_backend("storage", &self.storage, &mut validation);
        validate_data_backend("audit", &self.audit, &mut validation);
        validate_data_backend("cache", &self.cache, &mut validation);
        validate_host_providers(&self.host_providers, &mut validation);
        validate_mock_flags(&self.mock_providers, &mut validation);
        validate_observability(&self.observability, &mut validation);

        if self.profile == RuntimeProfile::Production {
            validate_production_config(self, &mut validation);
        }

        validation
    }

    pub fn redacted_diagnostics(&self) -> Value {
        json!({
            "schemaVersion": redact_runtime_config_text(&self.schema_version),
            "profile": self.profile,
            "identity": {
                "provider": configured(self.identity.provider.is_some()),
                "didDocument": configured(self.identity.did_document.is_some()),
                "credential": configured(self.identity.credential.is_some()),
            },
            "resolver": {
                "provider": configured(self.resolver.provider.is_some()),
                "trustAnchor": configured(self.resolver.trust_anchor.is_some()),
                "cacheTtlSeconds": self.resolver.cache_ttl_seconds,
            },
            "allowlist": {
                "networkRuleCount": self.allowlist.network_rules.len(),
            },
            "tokenIssuer": {
                "issuer": self.token_issuer.issuer.as_deref().map(redact_runtime_config_text),
                "secretRef": configured(self.token_issuer.secret_ref.is_some()),
            },
            "storage": data_backend_diagnostics(&self.storage),
            "audit": data_backend_diagnostics(&self.audit),
            "cache": data_backend_diagnostics(&self.cache),
            "hostProviders": {
                "count": self.host_providers.len(),
                "capabilities": self.host_providers
                    .iter()
                    .flat_map(|provider| {
                        provider
                            .capabilities
                            .iter()
                            .map(|capability| redact_runtime_config_text(capability))
                    })
                    .collect::<Vec<_>>(),
                "hasMockOrDevOnly": self.host_providers
                    .iter()
                    .any(|provider| provider.mock || provider.dev_only),
            },
            "mockProviders": self.mock_providers,
            "observability": {
                "level": self.observability.level,
                "structuredEvents": self.observability.structured_events,
                "redactionPolicy": RUNTIME_CONFIG_REDACTION_POLICY,
            },
            "redaction": {
                "marker": RUNTIME_CONFIG_REDACTION_MARKER,
                "policy": RUNTIME_CONFIG_REDACTION_POLICY,
                "appliedByDefault": true,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProfile {
    Development,
    Demo,
    Production,
}

impl Default for RuntimeProfile {
    fn default() -> Self {
        Self::Development
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeIdentityConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub did_document: Option<ConfigReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<SecretReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeResolverConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_anchor: Option<ConfigReference>,
    #[serde(default = "default_resolver_cache_ttl_seconds")]
    pub cache_ttl_seconds: u64,
}

impl Default for RuntimeResolverConfig {
    fn default() -> Self {
        Self {
            provider: None,
            trust_anchor: None,
            cache_ttl_seconds: default_resolver_cache_ttl_seconds(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeAllowlistConfig {
    #[serde(default)]
    pub network_rules: Vec<NetworkAllowlistReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkAllowlistReference {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub source: ConfigReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeTokenIssuerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<SecretReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDataBackendConfig {
    #[serde(default)]
    pub backend: RuntimeDataBackendKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_ref: Option<PathReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

impl Default for RuntimeDataBackendConfig {
    fn default() -> Self {
        Self {
            backend: RuntimeDataBackendKind::InMemory,
            path_ref: None,
            provider: None,
            quota_bytes: None,
            retention_days: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeDataBackendKind {
    InMemory,
    File,
    Sqlite,
    EncryptedSqlite,
    HostProvider,
}

impl Default for RuntimeDataBackendKind {
    fn default() -> Self {
        Self::InMemory
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostProviderConfig {
    pub handle: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub mock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeMockProviderFlags {
    #[serde(default)]
    pub identity: bool,
    #[serde(default)]
    pub resolver: bool,
    #[serde(default)]
    pub consent: bool,
    #[serde(default)]
    pub storage: bool,
    #[serde(default)]
    pub audit: bool,
    #[serde(default)]
    pub cache: bool,
    #[serde(default)]
    pub network: bool,
}

impl RuntimeMockProviderFlags {
    pub fn any_enabled(&self) -> bool {
        self.identity
            || self.resolver
            || self.consent
            || self.storage
            || self.audit
            || self.cache
            || self.network
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeObservabilityConfig {
    #[serde(default)]
    pub level: RuntimeObservabilityLevel,
    #[serde(default = "default_structured_events")]
    pub structured_events: bool,
}

impl Default for RuntimeObservabilityConfig {
    fn default() -> Self {
        Self {
            level: RuntimeObservabilityLevel::Info,
            structured_events: default_structured_events(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeObservabilityLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
}

impl Default for RuntimeObservabilityLevel {
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderReference {
    pub handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ConfigReference {
    RuntimeData { path: String },
    Url { url: String },
    HostProvider { handle: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PathReference {
    RuntimeData { path: String },
    HostProvider { handle: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SecretReference {
    Env { name: String },
    SecretStore { key: String },
    HostCredentialProvider { handle: String },
}

impl SecretReference {
    pub fn kind_name(&self) -> &'static str {
        match self {
            SecretReference::Env { .. } => "env",
            SecretReference::SecretStore { .. } => "secretStore",
            SecretReference::HostCredentialProvider { .. } => "hostCredentialProvider",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigValidation {
    pub profile: RuntimeProfile,
    pub valid: bool,
    pub release_blocked: bool,
    pub issues: Vec<RuntimeConfigIssue>,
    pub release_blockers: Vec<RuntimeConfigReleaseBlocker>,
}

impl RuntimeConfigValidation {
    fn new(profile: RuntimeProfile) -> Self {
        Self {
            profile,
            valid: true,
            release_blocked: false,
            issues: Vec::new(),
            release_blockers: Vec::new(),
        }
    }

    fn issue(
        &mut self,
        severity: RuntimeConfigIssueSeverity,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        if severity == RuntimeConfigIssueSeverity::Error {
            self.valid = false;
        }
        self.issues.push(RuntimeConfigIssue {
            severity,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        });
    }

    fn error(
        &mut self,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issue(RuntimeConfigIssueSeverity::Error, code, path, message);
    }

    fn blocker(
        &mut self,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.release_blocked = true;
        self.release_blockers.push(RuntimeConfigReleaseBlocker {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeConfigIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigIssue {
    pub severity: RuntimeConfigIssueSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigReleaseBlocker {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("runtime config JSON is invalid: {message}")]
pub struct RuntimeConfigLoadError {
    pub message: String,
}

pub fn redact_runtime_config_text(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for marker in [
        "authorization",
        "signature",
        "capabilitytoken",
        "capability_token",
        "bearer ",
        "token=",
        "secret",
        "secret=",
        "private",
        "password",
        "credential",
        "cookie",
        "-----begin",
        "/home/",
        "/users/",
        "\\users\\",
        "c:\\",
        "file:",
    ] {
        if lower.contains(marker) {
            return RUNTIME_CONFIG_REDACTION_MARKER.to_owned();
        }
    }
    value.to_owned()
}

fn validate_identity(identity: &RuntimeIdentityConfig, validation: &mut RuntimeConfigValidation) {
    validate_provider("identity.provider", identity.provider.as_ref(), validation);
    validate_config_ref(
        "identity.didDocument",
        identity.did_document.as_ref(),
        validation,
    );
    validate_secret_ref(
        "identity.credential",
        identity.credential.as_ref(),
        validation,
    );
}

fn validate_resolver(resolver: &RuntimeResolverConfig, validation: &mut RuntimeConfigValidation) {
    validate_provider("resolver.provider", resolver.provider.as_ref(), validation);
    validate_config_ref(
        "resolver.trustAnchor",
        resolver.trust_anchor.as_ref(),
        validation,
    );
    if resolver.cache_ttl_seconds == 0 {
        validation.error(
            "invalid_resolver_cache_ttl",
            "resolver.cacheTtlSeconds",
            "resolver cacheTtlSeconds must be greater than zero",
        );
    }
}

fn validate_allowlist(
    allowlist: &RuntimeAllowlistConfig,
    validation: &mut RuntimeConfigValidation,
) {
    for (index, rule) in allowlist.network_rules.iter().enumerate() {
        let path = format!("allowlist.networkRules[{index}]");
        check_sensitive_string(&format!("{path}.name"), &rule.name, validation);
        if let Some(scope) = &rule.scope {
            check_sensitive_string(&format!("{path}.scope"), scope, validation);
        }
        validate_config_ref(&format!("{path}.source"), Some(&rule.source), validation);
    }
}

fn validate_token_issuer(
    token_issuer: &RuntimeTokenIssuerConfig,
    validation: &mut RuntimeConfigValidation,
) {
    if let Some(issuer) = &token_issuer.issuer {
        check_sensitive_string("tokenIssuer.issuer", issuer, validation);
    }
    validate_secret_ref(
        "tokenIssuer.secretRef",
        token_issuer.secret_ref.as_ref(),
        validation,
    );
}

fn validate_data_backend(
    prefix: &str,
    backend: &RuntimeDataBackendConfig,
    validation: &mut RuntimeConfigValidation,
) {
    validate_path_ref(
        &format!("{prefix}.pathRef"),
        backend.path_ref.as_ref(),
        validation,
    );
    validate_provider(
        &format!("{prefix}.provider"),
        backend.provider.as_ref(),
        validation,
    );
}

fn validate_host_providers(
    providers: &[HostProviderConfig],
    validation: &mut RuntimeConfigValidation,
) {
    for (index, provider) in providers.iter().enumerate() {
        let path = format!("hostProviders[{index}]");
        check_sensitive_string(&format!("{path}.handle"), &provider.handle, validation);
        if provider.handle.trim().is_empty() {
            validation.error(
                "empty_provider_handle",
                format!("{path}.handle"),
                "host provider handle must not be empty",
            );
        }
        for (capability_index, capability) in provider.capabilities.iter().enumerate() {
            if capability.trim().is_empty() {
                validation.error(
                    "empty_provider_capability",
                    format!("{path}.capabilities[{capability_index}]"),
                    "host provider capability must not be empty",
                );
            }
            check_sensitive_string(
                &format!("{path}.capabilities[{capability_index}]"),
                capability,
                validation,
            );
        }
    }
}

fn validate_mock_flags(
    _flags: &RuntimeMockProviderFlags,
    _validation: &mut RuntimeConfigValidation,
) {
}

fn validate_observability(
    _observability: &RuntimeObservabilityConfig,
    _validation: &mut RuntimeConfigValidation,
) {
}

fn validate_production_config(config: &RuntimeConfig, validation: &mut RuntimeConfigValidation) {
    if config.identity.provider.is_none() && config.identity.credential.is_none() {
        validation.blocker(
            "missing_identity_provider",
            "identity",
            "production profile requires an identity provider or credential secret reference",
        );
    }

    if config.resolver.provider.is_none() && config.resolver.trust_anchor.is_none() {
        validation.blocker(
            "missing_resolver_provider",
            "resolver",
            "production profile requires a resolver provider or trust anchor reference",
        );
    }

    if config.allowlist.network_rules.is_empty() {
        validation.blocker(
            "missing_network_allowlist",
            "allowlist.networkRules",
            "production profile requires an explicit network allowlist source",
        );
    }

    if config.token_issuer.secret_ref.is_none() {
        validation.blocker(
            "missing_token_issuer_secret_ref",
            "tokenIssuer.secretRef",
            "production profile requires token issuer secretRef",
        );
    }

    production_backend_gate("storage", &config.storage, validation);
    production_backend_gate("audit", &config.audit, validation);
    production_backend_gate("cache", &config.cache, validation);

    if !has_host_capability(&config.host_providers, "render") {
        validation.blocker(
            "missing_host_render_provider",
            "hostProviders",
            "production profile requires a Host render provider capability",
        );
    }

    if !has_host_capability(&config.host_providers, "consent") {
        validation.blocker(
            "missing_host_consent_provider",
            "hostProviders",
            "production profile requires a Host consent provider capability",
        );
    }

    if config.mock_providers.any_enabled() {
        validation.blocker(
            "mock_provider_enabled",
            "mockProviders",
            "production profile must not enable mock provider flags",
        );
    }

    for (index, provider) in config.host_providers.iter().enumerate() {
        if provider.mock || provider.dev_only {
            validation.blocker(
                "mock_or_dev_host_provider",
                format!("hostProviders[{index}]"),
                "production profile must not use mock or dev-only Host providers",
            );
        }
    }
}

fn production_backend_gate(
    prefix: &str,
    backend: &RuntimeDataBackendConfig,
    validation: &mut RuntimeConfigValidation,
) {
    if backend.backend == RuntimeDataBackendKind::InMemory {
        validation.blocker(
            format!("in_memory_{prefix}_backend"),
            prefix,
            "production profile must not use in-memory runtime backend",
        );
    }
    if backend.path_ref.is_none() && backend.provider.is_none() {
        validation.blocker(
            format!("missing_{prefix}_backend_reference"),
            prefix,
            "production profile requires a pathRef or Host provider reference",
        );
    }
}

fn validate_provider(
    path: &str,
    provider: Option<&ProviderReference>,
    validation: &mut RuntimeConfigValidation,
) {
    if let Some(provider) = provider {
        if provider.handle.trim().is_empty() {
            validation.error(
                "empty_provider_handle",
                path,
                "provider handle must not be empty",
            );
        }
        check_sensitive_string(&format!("{path}.handle"), &provider.handle, validation);
    }
}

fn validate_config_ref(
    path: &str,
    config_ref: Option<&ConfigReference>,
    validation: &mut RuntimeConfigValidation,
) {
    if let Some(config_ref) = config_ref {
        match config_ref {
            ConfigReference::RuntimeData { path: value } => {
                validate_relative_runtime_path(path, value, validation);
            }
            ConfigReference::Url { url } => {
                check_sensitive_string(path, url, validation);
            }
            ConfigReference::HostProvider { handle } => {
                check_sensitive_string(path, handle, validation);
            }
        }
    }
}

fn validate_path_ref(
    path: &str,
    path_ref: Option<&PathReference>,
    validation: &mut RuntimeConfigValidation,
) {
    if let Some(path_ref) = path_ref {
        match path_ref {
            PathReference::RuntimeData { path: value } => {
                validate_relative_runtime_path(path, value, validation);
            }
            PathReference::HostProvider { handle } => {
                check_sensitive_string(path, handle, validation);
            }
        }
    }
}

fn validate_secret_ref(
    path: &str,
    secret_ref: Option<&SecretReference>,
    validation: &mut RuntimeConfigValidation,
) {
    if let Some(secret_ref) = secret_ref {
        match secret_ref {
            SecretReference::Env { name } => {
                if name.trim().is_empty() {
                    validation.error("empty_secret_ref", path, "env secret reference is empty");
                }
                if looks_like_inline_secret(name) {
                    validation.error(
                        "inline_secret_rejected",
                        path,
                        "secret reference appears to contain inline secret material",
                    );
                }
            }
            SecretReference::SecretStore { key } => {
                if key.trim().is_empty() {
                    validation.error(
                        "empty_secret_ref",
                        path,
                        "secret store key reference is empty",
                    );
                }
                if looks_like_inline_secret(key) {
                    validation.error(
                        "inline_secret_rejected",
                        path,
                        "secret reference appears to contain inline secret material",
                    );
                }
            }
            SecretReference::HostCredentialProvider { handle } => {
                if handle.trim().is_empty() {
                    validation.error(
                        "empty_secret_ref",
                        path,
                        "Host credential provider handle is empty",
                    );
                }
                if looks_like_inline_secret(handle) {
                    validation.error(
                        "inline_secret_rejected",
                        path,
                        "secret reference appears to contain inline secret material",
                    );
                }
            }
        }
    }
}

fn validate_relative_runtime_path(
    path: &str,
    value: &str,
    validation: &mut RuntimeConfigValidation,
) {
    check_sensitive_string(path, value, validation);
    let lower = value.to_ascii_lowercase();
    if value.trim().is_empty() {
        validation.error(
            "empty_path_ref",
            path,
            "runtime path reference must not be empty",
        );
    }
    if value.starts_with('/')
        || value.starts_with('\\')
        || lower.contains(":\\")
        || lower.contains("../")
        || lower.contains("..\\")
    {
        validation.error(
            "unsafe_path_ref",
            path,
            "runtime path reference must be relative to the runtime data root and stay in scope",
        );
    }
}

fn check_sensitive_string(path: &str, value: &str, validation: &mut RuntimeConfigValidation) {
    if looks_like_inline_secret(value) {
        validation.error(
            "inline_secret_rejected",
            path,
            "config value appears to contain inline secret material and was rejected",
        );
    } else if looks_like_local_private_path(value) {
        validation.error(
            "local_private_path_rejected",
            path,
            "config value appears to contain a local private path and was rejected",
        );
    }
}

fn looks_like_inline_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "-----begin ",
        "authorization:",
        "authorization bearer",
        "bearer ",
        "signature:",
        "signature-input:",
        "capabilitytoken",
        "capability_token",
        "token=",
        "secret=",
        "merchant secret",
        "raw token",
        "private key",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn looks_like_local_private_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.contains("/home/") || lower.contains("/users/") || lower.contains("\\users\\"))
        && (lower.contains("private") || lower.contains("key-") || lower.contains("secret"))
}

fn has_host_capability(providers: &[HostProviderConfig], capability: &str) -> bool {
    providers.iter().any(|provider| {
        provider
            .capabilities
            .iter()
            .any(|candidate| candidate == capability)
    })
}

fn data_backend_diagnostics(backend: &RuntimeDataBackendConfig) -> Value {
    json!({
        "backend": backend.backend,
        "pathRef": configured(backend.path_ref.is_some()),
        "provider": configured(backend.provider.is_some()),
        "quotaBytes": backend.quota_bytes,
        "retentionDays": backend.retention_days,
    })
}

fn configured(is_configured: bool) -> Option<&'static str> {
    is_configured.then_some("[CONFIGURED]")
}

fn default_schema_version() -> String {
    RUNTIME_CONFIG_SCHEMA_VERSION.to_owned()
}

fn default_resolver_cache_ttl_seconds() -> u64 {
    300
}

fn default_structured_events() -> bool {
    true
}
