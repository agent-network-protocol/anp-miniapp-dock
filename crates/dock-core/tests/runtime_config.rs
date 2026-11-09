use dock_core::{
    ConfigReference, HostProviderConfig, NetworkAllowlistReference, PathReference,
    ProviderReference, RuntimeAllowlistConfig, RuntimeConfig, RuntimeDataBackendConfig,
    RuntimeDataBackendKind, RuntimeIdentityConfig, RuntimeMockProviderFlags,
    RuntimeObservabilityLevel, RuntimeProfile, RuntimeResolverConfig, RuntimeTokenIssuerConfig,
    SecretReference, RUNTIME_CONFIG_LOAD_PRIORITY, RUNTIME_CONFIG_REDACTION_MARKER,
    RUNTIME_CONFIG_SCHEMA_VERSION,
};
use serde_json::json;

#[test]
fn runtime_config_defaults_to_development_profile() {
    let config = RuntimeConfig::default();

    assert_eq!(config.schema_version, RUNTIME_CONFIG_SCHEMA_VERSION);
    assert_eq!(config.profile, RuntimeProfile::Development);
    assert_eq!(config.storage.backend, RuntimeDataBackendKind::InMemory);
    assert_eq!(config.observability.level, RuntimeObservabilityLevel::Info);
    assert!(config.observability.structured_events);
    assert_eq!(RUNTIME_CONFIG_LOAD_PRIORITY.len(), 5);

    let validation = config.validate();
    assert!(validation.valid);
    assert!(!validation.release_blocked);
    assert!(validation.issues.is_empty());
}

#[test]
fn runtime_config_loader_rejects_unknown_fields() {
    let error = RuntimeConfig::from_json_str(
        r#"{
            "schemaVersion": "dock.runtime.config.v1",
            "profile": "demo",
            "inlineSecret": "should-not-exist"
        }"#,
    )
    .expect_err("unknown fields are rejected");

    assert!(error.to_string().contains("runtime config JSON is invalid"));
    assert!(!error.to_string().contains("should-not-exist"));
}

#[test]
fn runtime_config_loader_accepts_only_structured_secret_references() {
    let config = RuntimeConfig::from_json_str(
        r#"{
            "schemaVersion": "dock.runtime.config.v1",
            "profile": "demo",
            "identity": {
                "credential": {
                    "kind": "hostCredentialProvider",
                    "handle": "did-signing-key"
                }
            },
            "tokenIssuer": {
                "issuer": "did:wba:issuer.example",
                "secretRef": {
                    "kind": "secretStore",
                    "key": "runtime/token-issuer"
                }
            }
        }"#,
    )
    .expect("structured secret references load");

    assert_eq!(
        config.identity.credential,
        Some(SecretReference::HostCredentialProvider {
            handle: "did-signing-key".to_owned()
        })
    );
    assert_eq!(
        config.token_issuer.secret_ref,
        Some(SecretReference::SecretStore {
            key: "runtime/token-issuer".to_owned()
        })
    );

    let error = RuntimeConfig::from_json_str(
        r#"{
            "schemaVersion": "dock.runtime.config.v1",
            "profile": "demo",
            "tokenIssuer": {
                "secretRef": "Authorization Bearer raw-token-value"
            }
        }"#,
    )
    .expect_err("inline secret string is not a valid secret reference shape");

    assert!(!error.to_string().contains("raw-token-value"));
    assert!(!error.to_string().contains("Authorization Bearer"));
}

#[test]
fn runtime_config_production_profile_reports_required_release_blockers() {
    let config = RuntimeConfig {
        profile: RuntimeProfile::Production,
        ..RuntimeConfig::default()
    };

    let validation = config.validate();

    assert!(validation.valid);
    assert!(validation.release_blocked);
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_identity_provider"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_resolver_provider"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_network_allowlist"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_token_issuer_secret_ref"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "in_memory_storage_backend"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_host_render_provider"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "missing_host_consent_provider"));
}

#[test]
fn runtime_config_production_profile_accepts_secret_refs_and_provider_handles_without_inline_secrets(
) {
    let config = RuntimeConfig {
        profile: RuntimeProfile::Production,
        identity: RuntimeIdentityConfig {
            provider: Some(ProviderReference {
                handle: "host-identity".to_owned(),
            }),
            did_document: Some(ConfigReference::HostProvider {
                handle: "host-identity".to_owned(),
            }),
            credential: Some(SecretReference::HostCredentialProvider {
                handle: "did-signing-key".to_owned(),
            }),
        },
        resolver: RuntimeResolverConfig {
            provider: Some(ProviderReference {
                handle: "trusted-did-resolver".to_owned(),
            }),
            trust_anchor: Some(ConfigReference::RuntimeData {
                path: "trust/anchors.json".to_owned(),
            }),
            ..RuntimeResolverConfig::default()
        },
        allowlist: RuntimeAllowlistConfig {
            network_rules: vec![NetworkAllowlistReference {
                name: "merchant-api".to_owned(),
                scope: Some("coffee".to_owned()),
                source: ConfigReference::RuntimeData {
                    path: "policy/network-allowlist.json".to_owned(),
                },
            }],
        },
        token_issuer: RuntimeTokenIssuerConfig {
            issuer: Some("did:wba:issuer.example".to_owned()),
            secret_ref: Some(SecretReference::SecretStore {
                key: "runtime/token-issuer".to_owned(),
            }),
        },
        storage: RuntimeDataBackendConfig {
            backend: RuntimeDataBackendKind::EncryptedSqlite,
            path_ref: Some(PathReference::RuntimeData {
                path: "state/storage.sqlite3".to_owned(),
            }),
            quota_bytes: Some(8 * 1024 * 1024),
            ..RuntimeDataBackendConfig::default()
        },
        audit: RuntimeDataBackendConfig {
            backend: RuntimeDataBackendKind::File,
            path_ref: Some(PathReference::RuntimeData {
                path: "audit/runtime.jsonl".to_owned(),
            }),
            retention_days: Some(30),
            ..RuntimeDataBackendConfig::default()
        },
        cache: RuntimeDataBackendConfig {
            backend: RuntimeDataBackendKind::HostProvider,
            provider: Some(ProviderReference {
                handle: "host-skill-cache".to_owned(),
            }),
            ..RuntimeDataBackendConfig::default()
        },
        host_providers: vec![
            HostProviderConfig {
                handle: "host-renderer".to_owned(),
                capabilities: vec!["render".to_owned()],
                dev_only: false,
                mock: false,
            },
            HostProviderConfig {
                handle: "host-consent".to_owned(),
                capabilities: vec!["consent".to_owned()],
                dev_only: false,
                mock: false,
            },
        ],
        ..RuntimeConfig::default()
    };

    let validation = config.validate();

    assert!(validation.valid);
    assert!(!validation.release_blocked);
    assert!(validation.release_blockers.is_empty());

    let diagnostics = config.redacted_diagnostics();
    assert_eq!(diagnostics["profile"], json!("production"));
    assert_eq!(diagnostics["identity"]["credential"], json!("[CONFIGURED]"));
    assert_eq!(
        diagnostics["tokenIssuer"]["secretRef"],
        json!("[CONFIGURED]")
    );
    assert!(!diagnostics.to_string().contains("runtime/token-issuer"));
    assert!(!diagnostics.to_string().contains("state/storage.sqlite3"));
}

#[test]
fn runtime_config_inline_secret_material_is_rejected_and_redacted_in_diagnostics() {
    let config = RuntimeConfig {
        schema_version: "Authorization Bearer raw-token-value".to_owned(),
        profile: RuntimeProfile::Demo,
        identity: RuntimeIdentityConfig {
            credential: Some(SecretReference::Env {
                name: "Authorization Bearer raw-token-value".to_owned(),
            }),
            ..RuntimeIdentityConfig::default()
        },
        token_issuer: RuntimeTokenIssuerConfig {
            issuer: Some("merchant secret should not be inline".to_owned()),
            secret_ref: Some(SecretReference::Env {
                name: "token=raw-token-value".to_owned(),
            }),
        },
        ..RuntimeConfig::default()
    };

    let validation = config.validate();

    assert!(!validation.valid);
    assert!(validation
        .issues
        .iter()
        .any(|issue| issue.code == "inline_secret_rejected"));
    let diagnostics = config.redacted_diagnostics().to_string();
    assert!(!diagnostics.contains("raw-token-value"));
    assert!(!diagnostics.contains("merchant secret"));
    assert!(diagnostics.contains(RUNTIME_CONFIG_REDACTION_MARKER));
}

#[test]
fn runtime_config_local_private_paths_are_rejected_and_not_reported_in_diagnostics() {
    let config = RuntimeConfig {
        profile: RuntimeProfile::Demo,
        storage: RuntimeDataBackendConfig {
            backend: RuntimeDataBackendKind::File,
            path_ref: Some(PathReference::RuntimeData {
                path: "/home/user/private/key-1-private.pem".to_owned(),
            }),
            ..RuntimeDataBackendConfig::default()
        },
        ..RuntimeConfig::default()
    };

    let validation = config.validate();

    assert!(!validation.valid);
    assert!(validation
        .issues
        .iter()
        .any(|issue| issue.code == "local_private_path_rejected"));
    assert!(validation
        .issues
        .iter()
        .any(|issue| issue.code == "unsafe_path_ref"));
    let diagnostics = config.redacted_diagnostics().to_string();
    assert!(!diagnostics.contains("/home/user/private/key-1-private.pem"));
}

#[test]
fn runtime_config_production_profile_blocks_mock_provider_flags_and_dev_host_providers() {
    let config = RuntimeConfig {
        profile: RuntimeProfile::Production,
        mock_providers: RuntimeMockProviderFlags {
            identity: true,
            ..RuntimeMockProviderFlags::default()
        },
        host_providers: vec![HostProviderConfig {
            handle: "dev-renderer".to_owned(),
            capabilities: vec!["render".to_owned(), "consent".to_owned()],
            dev_only: true,
            mock: true,
        }],
        ..RuntimeConfig::default()
    };

    let validation = config.validate();

    assert!(validation.release_blocked);
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "mock_provider_enabled"));
    assert!(validation
        .release_blockers
        .iter()
        .any(|blocker| blocker.code == "mock_or_dev_host_provider"));
}
