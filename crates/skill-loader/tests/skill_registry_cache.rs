use skill_loader::{
    compute_package_digest, development_signature_value, load_registry_skill,
    load_skill_with_integrity_policy, CachedSkillMetadata, LocalSkillRegistry, PackageDigest,
    PackageIntegrityPolicy, PackageIntegrityStatus, RegistrySkillEntry, SkillCache,
    SkillCacheCleanupAction, SkillCacheCleanupPolicy, SkillCacheCleanupScope, SkillCacheKey,
    SkillPackageError, SkillReference, SkillReferenceKind, SkillVersionSelector,
    DEVELOPMENT_SIGNATURE_ALGORITHM,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn registry_latest_and_pinned_versions_resolve_to_digest_cache() {
    let publisher = "did:wba:trusted.example";
    let v1 = signed_skill("registry-latest-v1", "coffee", "1.0.0", publisher);
    let v2 = signed_skill("registry-latest-v2", "coffee", "1.1.0", publisher);
    let cache_dir = TestCacheDir::new("cache-latest");
    let registry = LocalSkillRegistry::new([
        registry_entry("coffee", "1.0.0", publisher, &v1),
        registry_entry("coffee", "1.1.0", publisher, &v2),
    ]);
    let mut cache = SkillCache::new(cache_dir.path());
    let policy = PackageIntegrityPolicy::production([publisher]);

    let latest = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let (loaded, latest_metadata) =
        load_registry_skill(&registry, &mut cache, &latest, &policy).expect("latest loads");

    assert_eq!(loaded.integrity.status, PackageIntegrityStatus::Verified);
    assert_eq!(latest_metadata.key.version, "1.1.0");
    assert!(latest_metadata.readonly);
    assert!(latest_metadata.package_ref.starts_with("sha256:"));
    assert!(latest_metadata
        .rootless_audit_summary()
        .contains("\"version\":\"1.1.0\""));
    assert!(!latest_metadata.rootless_audit_summary().contains("/tmp/"));
    assert!(!latest_metadata
        .rootless_audit_summary()
        .contains("\\Users\\"));

    let cached_root = cache.root().join(latest_metadata.key.directory_name());
    let pinned = SkillReference::registry_id(
        "coffee",
        SkillVersionSelector::Pinned("1.1.0".to_owned()),
        Some(publisher.to_owned()),
    );
    let (_, pinned_metadata) =
        load_registry_skill(&registry, &mut cache, &pinned, &policy).expect("pinned loads");

    assert_eq!(pinned_metadata.key, latest_metadata.key);
    assert_eq!(
        cached_root,
        cache.root().join(pinned_metadata.key.directory_name())
    );
    assert!(cached_root.join("mcp.json").exists());
}

#[test]
fn registry_rolls_back_to_previous_verified_version_and_preserves_pin_on_eviction() {
    let publisher = "did:wba:trusted.example";
    let v1 = signed_skill("registry-rollback-v1", "coffee", "1.0.0", publisher);
    let v2 = signed_skill("registry-rollback-v2", "coffee", "1.1.0", publisher);
    let cache_dir = TestCacheDir::new("cache-rollback");
    let registry = LocalSkillRegistry::new([
        registry_entry("coffee", "1.0.0", publisher, &v1),
        registry_entry("coffee", "1.1.0", publisher, &v2),
    ]);
    let mut cache = SkillCache::new(cache_dir.path());
    let policy = PackageIntegrityPolicy::production([publisher]);

    let latest = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let (_, latest_metadata) =
        load_registry_skill(&registry, &mut cache, &latest, &policy).expect("latest loads");
    let rollback = SkillReference::registry_id(
        "coffee",
        SkillVersionSelector::Rollback {
            before_version: "1.1.0".to_owned(),
        },
        Some(publisher.to_owned()),
    );
    let (_, rollback_metadata) =
        load_registry_skill(&registry, &mut cache, &rollback, &policy).expect("rollback loads");
    cache.pin_rollback(rollback_metadata.key.clone());

    assert_eq!(rollback_metadata.key.version, "1.0.0");
    assert_eq!(
        cache.rollback_pin(publisher, "coffee"),
        Some(&rollback_metadata.key)
    );

    let removed = cache.evict_unpinned([latest_metadata.key.clone()]);

    assert_eq!(
        removed, 0,
        "latest retain plus rollback pin keep both versions"
    );
    assert!(cache
        .root()
        .join(rollback_metadata.key.directory_name())
        .exists());
    assert!(cache
        .root()
        .join(latest_metadata.key.directory_name())
        .exists());

    let removed = cache.evict_unpinned([rollback_metadata.key.clone()]);

    assert_eq!(removed, 1, "unpinned latest can be evicted");
    assert!(cache
        .root()
        .join(rollback_metadata.key.directory_name())
        .exists());
    assert!(!cache
        .root()
        .join(latest_metadata.key.directory_name())
        .exists());
}

#[test]
fn registry_digest_mismatch_fails_closed_before_cache_reuse() {
    let publisher = "did:wba:trusted.example";
    let skill = signed_skill("registry-digest-mismatch", "coffee", "1.0.0", publisher);
    let cache_dir = TestCacheDir::new("cache-digest-mismatch");
    let mut entry = registry_entry("coffee", "1.0.0", publisher, &skill);
    entry.digest = "0".repeat(64);
    let registry = LocalSkillRegistry::new([entry]);
    let mut cache = SkillCache::new(cache_dir.path());
    let reference = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let policy = PackageIntegrityPolicy::production([publisher]);

    let error = load_registry_skill(&registry, &mut cache, &reference, &policy)
        .expect_err("digest mismatch must fail before caching");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "digest_mismatch"
    ));
    assert!(fs::read_dir(cache.root())
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true));
}

#[test]
fn registry_unknown_publisher_quarantines_package() {
    let publisher = "did:wba:unknown.example";
    let skill = signed_skill("registry-unknown-publisher", "coffee", "1.0.0", publisher);
    let cache_dir = TestCacheDir::new("cache-unknown-publisher");
    let registry = LocalSkillRegistry::new([registry_entry("coffee", "1.0.0", publisher, &skill)]);
    let mut cache = SkillCache::new(cache_dir.path());
    let reference = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let policy = PackageIntegrityPolicy::production(["did:wba:trusted.example"]);

    let error = load_registry_skill(&registry, &mut cache, &reference, &policy)
        .expect_err("unknown publisher must quarantine");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "unknown_publisher"
    ));
}

#[test]
fn registry_version_selection_uses_numeric_order_and_prerelease_policy() {
    let publisher = "did:wba:trusted.example";
    let v2 = signed_skill("registry-version-v2", "coffee", "1.2.0", publisher);
    let v10 = signed_skill("registry-version-v10", "coffee", "1.10.0", publisher);
    let beta = signed_skill("registry-version-beta", "coffee", "2.0.0-beta.1", publisher);
    let stable_cache_dir = TestCacheDir::new("cache-version-stable");
    let prerelease_cache_dir = TestCacheDir::new("cache-version-prerelease");
    let mut beta_entry = registry_entry("coffee", "2.0.0-beta.1", publisher, &beta);
    beta_entry.prerelease = true;
    let registry = LocalSkillRegistry::new([
        registry_entry("coffee", "1.2.0", publisher, &v2),
        registry_entry("coffee", "1.10.0", publisher, &v10),
        beta_entry,
    ]);
    let policy = PackageIntegrityPolicy::production([publisher]);

    let mut stable_cache = SkillCache::new(stable_cache_dir.path());
    let stable_latest = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let (_, stable_metadata) =
        load_registry_skill(&registry, &mut stable_cache, &stable_latest, &policy)
            .expect("stable latest loads");
    assert_eq!(stable_metadata.key.version, "1.10.0");

    let mut prerelease_cache = SkillCache::new(prerelease_cache_dir.path());
    let prerelease_latest = SkillReference::registry_id_with_prerelease(
        "coffee",
        SkillVersionSelector::Latest,
        None,
        true,
    );
    let (_, prerelease_metadata) = load_registry_skill(
        &registry,
        &mut prerelease_cache,
        &prerelease_latest,
        &policy,
    )
    .expect("prerelease latest loads");
    assert_eq!(prerelease_metadata.key.version, "2.0.0-beta.1");
}

#[test]
fn cache_audit_summary_redacts_package_url_secrets() {
    let publisher = "did:wba:trusted.example";
    let skill = signed_skill("registry-url-redaction", "coffee", "1.0.0", publisher);
    let cache_dir = TestCacheDir::new("cache-url-redaction");
    let mut entry = registry_entry("coffee", "1.0.0", publisher, &skill);
    entry.package_url = Some(
        "https://registry.example.invalid/coffee.zip?token=capability-secret-token".to_owned(),
    );
    let registry = LocalSkillRegistry::new([entry]);
    let mut cache = SkillCache::new(cache_dir.path());
    let reference = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let policy = PackageIntegrityPolicy::production([publisher]);

    let (_, metadata) =
        load_registry_skill(&registry, &mut cache, &reference, &policy).expect("skill loads");

    let rendered = metadata.audit_summary().to_string();
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("?token="));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn cache_cleanup_dry_run_reports_scope_without_paths_or_url_secrets() {
    let publisher = "did:wba:trusted.example";
    let merchant = "did:wba:merchant.example";
    let skill = signed_skill("cache-cleanup-dry-run", "coffee", "1.0.0", publisher);
    let cache_dir = TestCacheDir::new("cache-cleanup-dry-run");
    let mut entry = registry_entry("coffee", "1.0.0", publisher, &skill);
    entry.merchant_did = Some(merchant.to_owned());
    entry.package_url = Some(
        "https://registry.example.invalid/coffee.zip?token=capability-secret-token".to_owned(),
    );
    let registry = LocalSkillRegistry::new([entry]);
    let mut cache = SkillCache::new(cache_dir.path());
    let reference = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let policy = PackageIntegrityPolicy::production([publisher]);
    let (_, metadata) =
        load_registry_skill(&registry, &mut cache, &reference, &policy).expect("skill loads");

    let report = cache
        .cleanup(SkillCacheCleanupPolicy::dry_run(
            SkillCacheCleanupScope::all()
                .publisher(publisher)
                .merchant(merchant)
                .skill("coffee"),
        ))
        .expect("dry-run cleanup report");

    assert!(report.dry_run);
    assert_eq!(report.scanned_count, 1);
    assert_eq!(report.matched_count, 1);
    assert_eq!(report.removed_count, 0);
    assert_eq!(report.entries[0].action, SkillCacheCleanupAction::Remove);
    assert_eq!(report.entries[0].key, Some(metadata.key.clone()));
    assert_eq!(report.entries[0].merchant_did.as_deref(), Some(merchant));
    assert!(cache.root().join(metadata.key.directory_name()).exists());
    let rendered = serde_json::to_string(&report).expect("report serializes");
    assert!(!rendered.contains(cache.root().to_string_lossy().as_ref()));
    assert!(!rendered.contains("capability-secret-token"));
    assert!(!rendered.contains("?token="));
    assert!(rendered.contains("\"rootPathVisible\":false"));
}

#[test]
fn cache_cleanup_delete_scope_removes_matching_cache_and_metadata_only() {
    let publisher = "did:wba:trusted.example";
    let other_publisher = "did:wba:other.example";
    let merchant = "did:wba:merchant.example";
    let skill_a = signed_skill("cache-cleanup-delete-a", "coffee", "1.0.0", publisher);
    let skill_b = signed_skill("cache-cleanup-delete-b", "tea", "1.0.0", other_publisher);
    let cache_dir = TestCacheDir::new("cache-cleanup-delete");
    let mut entry_a = registry_entry("coffee", "1.0.0", publisher, &skill_a);
    entry_a.merchant_did = Some(merchant.to_owned());
    let registry = LocalSkillRegistry::new([
        entry_a,
        registry_entry("tea", "1.0.0", other_publisher, &skill_b),
    ]);
    let mut cache = SkillCache::new(cache_dir.path());
    let policy = PackageIntegrityPolicy::production([publisher, other_publisher]);
    let (_, coffee_metadata) = load_registry_skill(
        &registry,
        &mut cache,
        &SkillReference::registry_id(
            "coffee",
            SkillVersionSelector::Latest,
            Some(publisher.to_owned()),
        ),
        &policy,
    )
    .expect("coffee loads");
    let (_, tea_metadata) = load_registry_skill(
        &registry,
        &mut cache,
        &SkillReference::registry_id(
            "tea",
            SkillVersionSelector::Latest,
            Some(other_publisher.to_owned()),
        ),
        &policy,
    )
    .expect("tea loads");

    let report = cache
        .cleanup(SkillCacheCleanupPolicy::delete_scope(
            SkillCacheCleanupScope::all().merchant(merchant),
        ))
        .expect("cleanup removes merchant scope");

    assert_eq!(report.matched_count, 1);
    assert_eq!(report.removed_count, 1);
    assert!(!cache
        .root()
        .join(coffee_metadata.key.directory_name())
        .exists());
    assert!(!cache
        .root()
        .join(format!(
            "{}.dock-cache.json",
            coffee_metadata.key.directory_name()
        ))
        .exists());
    assert!(cache
        .root()
        .join(tea_metadata.key.directory_name())
        .exists());
}

#[test]
fn cache_cleanup_preserves_rollback_pin_and_active_retain() {
    let publisher = "did:wba:trusted.example";
    let v1 = signed_skill("cache-cleanup-pin-v1", "coffee", "1.0.0", publisher);
    let v2 = signed_skill("cache-cleanup-pin-v2", "coffee", "1.1.0", publisher);
    let v3 = signed_skill("cache-cleanup-pin-v3", "coffee", "1.2.0", publisher);
    let cache_dir = TestCacheDir::new("cache-cleanup-pin");
    let registry = LocalSkillRegistry::new([
        registry_entry("coffee", "1.0.0", publisher, &v1),
        registry_entry("coffee", "1.1.0", publisher, &v2),
        registry_entry("coffee", "1.2.0", publisher, &v3),
    ]);
    let mut cache = SkillCache::new(cache_dir.path());
    let policy = PackageIntegrityPolicy::production([publisher]);
    let (_, pinned_metadata) = load_registry_skill(
        &registry,
        &mut cache,
        &SkillReference::registry_id(
            "coffee",
            SkillVersionSelector::Pinned("1.0.0".to_owned()),
            Some(publisher.to_owned()),
        ),
        &policy,
    )
    .expect("pinned loads");
    let (_, retained_metadata) = load_registry_skill(
        &registry,
        &mut cache,
        &SkillReference::registry_id(
            "coffee",
            SkillVersionSelector::Pinned("1.1.0".to_owned()),
            Some(publisher.to_owned()),
        ),
        &policy,
    )
    .expect("retained loads");
    let (_, stale_metadata) = load_registry_skill(
        &registry,
        &mut cache,
        &SkillReference::registry_id(
            "coffee",
            SkillVersionSelector::Pinned("1.2.0".to_owned()),
            Some(publisher.to_owned()),
        ),
        &policy,
    )
    .expect("stale loads");
    cache.pin_rollback(pinned_metadata.key.clone());

    let report = cache
        .cleanup(
            SkillCacheCleanupPolicy::delete_scope(SkillCacheCleanupScope::all().skill("coffee"))
                .retain(retained_metadata.key.clone()),
        )
        .expect("cleanup preserves pins");

    assert_eq!(report.matched_count, 3);
    assert_eq!(report.retained_count, 2);
    assert_eq!(report.removed_count, 1);
    assert!(cache
        .root()
        .join(pinned_metadata.key.directory_name())
        .exists());
    assert!(cache
        .root()
        .join(retained_metadata.key.directory_name())
        .exists());
    assert!(!cache
        .root()
        .join(stale_metadata.key.directory_name())
        .exists());
    assert!(report
        .entries
        .iter()
        .any(|entry| entry.retained_by_rollback_pin));
    assert!(report
        .entries
        .iter()
        .any(|entry| entry.retained_by_active_retain));
}

#[test]
fn cache_quarantine_blocks_reload_and_cleanup_can_purge_it() {
    let publisher = "did:wba:trusted.example";
    let skill = signed_skill("cache-quarantine", "coffee", "1.0.0", publisher);
    let cache_dir = TestCacheDir::new("cache-quarantine");
    let entry = registry_entry("coffee", "1.0.0", publisher, &skill);
    let key = entry.cache_key();
    let registry = LocalSkillRegistry::new([entry]);
    let mut cache = SkillCache::new(cache_dir.path());
    let reference = SkillReference::registry_id("coffee", SkillVersionSelector::Latest, None);
    let policy = PackageIntegrityPolicy::production([publisher]);
    load_registry_skill(&registry, &mut cache, &reference, &policy).expect("initial load caches");

    let quarantined = cache
        .quarantine(&key, "Authorization Bearer secret-token")
        .expect("cache quarantine writes metadata");
    assert!(quarantined.quarantined);
    assert_eq!(quarantined.quarantine_reason.as_deref(), Some("[REDACTED]"));

    let error = load_registry_skill(&registry, &mut cache, &reference, &policy)
        .expect_err("quarantined cache cannot reload");
    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "[REDACTED]"
    ));

    let report = cache
        .cleanup(SkillCacheCleanupPolicy::delete_scope(
            SkillCacheCleanupScope::all().digest(key.digest.clone()),
        ))
        .expect("cleanup purges quarantined cache");
    assert_eq!(report.quarantined_count, 1);
    assert_eq!(report.removed_count, 1);
    assert!(!cache.root().join(key.directory_name()).exists());
}

#[test]
fn skill_reference_supports_local_path_and_package_url_shapes() {
    let publisher = "did:wba:trusted.example";
    let skill = signed_skill("registry-ref-shapes", "coffee", "1.0.0", publisher);
    let digest = compute_package_digest(skill.path()).expect("digest");
    let local = SkillReference::local_path(skill.path());
    let package_url = SkillReference::package_url(
        "coffee",
        "https://registry.example.invalid/coffee-1.0.0.zip",
        publisher,
        "1.0.0",
        digest.value.clone(),
    );

    assert!(matches!(local.kind, SkillReferenceKind::LocalPath(_)));
    assert!(matches!(
        package_url.kind,
        SkillReferenceKind::PackageUrl(_)
    ));
    assert_eq!(package_url.publisher_did.as_deref(), Some(publisher));
    assert_eq!(package_url.version.as_deref(), Some("1.0.0"));
    assert_eq!(package_url.digest.as_deref(), Some(digest.value.as_str()));
}

trait MetadataTestExt {
    fn rootless_audit_summary(&self) -> String;
}

impl MetadataTestExt for CachedSkillMetadata {
    fn rootless_audit_summary(&self) -> String {
        self.audit_summary().to_string()
    }
}

struct TestSkillDir {
    path: PathBuf,
}

impl TestSkillDir {
    fn new(name: &str) -> Self {
        let path = test_root(&format!("skill-{name}"));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale test dir");
        }
        fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative_path: &str, source: &str) {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, source).expect("write test file");
    }
}

impl Drop for TestSkillDir {
    fn drop(&mut self) {
        let _ = make_writable_recursive(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct TestCacheDir {
    path: PathBuf,
}

impl TestCacheDir {
    fn new(name: &str) -> Self {
        Self {
            path: test_root(name),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestCacheDir {
    fn drop(&mut self) {
        let _ = make_writable_recursive(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn signed_skill(name: &str, skill_id: &str, version: &str, publisher: &str) -> TestSkillDir {
    let skill = TestSkillDir::new(name);
    skill.write("SKILL.md", &format!("# {skill_id} {version}"));
    skill.write(
        "index.js",
        "const skill = wx.modelContext.createSkill(__dirname)\nmodule.exports = skill\n",
    );
    skill.write(
        "mcp.json",
        &manifest_json(skill_id, version, publisher, None, None),
    );
    let digest = compute_package_digest(skill.path()).expect("digest unsigned package");
    let key_id = format!("{publisher}#package-key-1");
    let signature = development_signature_value(publisher, &key_id, &digest);
    skill.write(
        "mcp.json",
        &manifest_json(
            skill_id,
            version,
            publisher,
            Some(&digest),
            Some(&signature),
        ),
    );
    load_skill_with_integrity_policy(
        skill.path(),
        &PackageIntegrityPolicy::production([publisher]),
    )
    .expect("signed fixture loads");
    skill
}

fn registry_entry(
    skill_id: &str,
    version: &str,
    publisher: &str,
    skill: &TestSkillDir,
) -> RegistrySkillEntry {
    let digest = compute_package_digest(skill.path()).expect("digest");
    RegistrySkillEntry {
        skill_id: skill_id.to_owned(),
        publisher_did: publisher.to_owned(),
        merchant_did: None,
        version: version.to_owned(),
        digest: digest.value,
        package_path: skill.path().to_path_buf(),
        package_url: Some(format!(
            "https://registry.example.invalid/{skill_id}-{version}.zip"
        )),
        prerelease: false,
    }
}

fn manifest_json(
    skill_id: &str,
    version: &str,
    publisher: &str,
    digest: Option<&PackageDigest>,
    signature: Option<&str>,
) -> String {
    let key_id = format!("{publisher}#package-key-1");
    let supply_chain = match (digest, signature) {
        (Some(digest), Some(signature)) => format!(
            r#","supplyChain": {{
                "publisherDid": "{publisher}",
                "digest": {{
                  "algorithm": "{}",
                  "value": "{}"
                }},
                "signature": {{
                  "algorithm": "{}",
                  "keyId": "{key_id}",
                  "value": "{signature}"
                }}
              }}"#,
            digest.algorithm, digest.value, DEVELOPMENT_SIGNATURE_ALGORITHM
        ),
        _ => String::new(),
    };

    format!(
        r#"{{
          "_meta": {{
            "anp": {{
              "skillId": "{skill_id}",
              "version": "{version}"
              {supply_chain}
            }}
          }},
          "apis": [],
          "components": []
        }}"#
    )
}

fn test_root(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "anp-miniapp-dock-skill-registry-{name}-{}",
        std::process::id()
    ));
    if path.exists() {
        let _ = make_writable_recursive(&path);
        fs::remove_dir_all(&path).expect("remove stale path");
    }
    path
}

fn make_writable_recursive(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(path)?;
    make_path_writable(path, &metadata)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            make_writable_recursive(&entry?.path())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_path_writable(path: &Path, metadata: &fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = metadata.permissions();
    let write_bits = if metadata.is_dir() { 0o700 } else { 0o600 };
    permissions.set_mode(permissions.mode() | write_bits);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_path_writable(path: &Path, metadata: &fs::Metadata) -> std::io::Result<()> {
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

#[allow(dead_code)]
fn cache_key(version: &str, digest: &str) -> SkillCacheKey {
    SkillCacheKey {
        publisher_did: "did:wba:trusted.example".to_owned(),
        skill_id: "coffee".to_owned(),
        version: version.to_owned(),
        digest: digest.to_owned(),
    }
}
