use crate::integrity::{compute_package_digest, PackageIntegrityPolicy, PackageIntegrityReport};
use crate::package::{load_skill_with_integrity_policy, LoadedSkill};
use crate::resolver::{resolve_skill_path, SkillPackageError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_METADATA_SUFFIX: &str = ".dock-cache.json";
const CACHE_REDACTION_POLICY: &str = "dock.skill-cache.redaction.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillReference {
    pub kind: SkillReferenceKind,
    pub skill_id: String,
    pub publisher_did: Option<String>,
    pub version: Option<String>,
    pub digest: Option<String>,
}

impl SkillReference {
    pub fn local_path(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: SkillReferenceKind::LocalPath(path.into()),
            skill_id: "local".to_owned(),
            publisher_did: None,
            version: None,
            digest: None,
        }
    }

    pub fn registry_id(
        skill_id: impl Into<String>,
        selector: SkillVersionSelector,
        publisher_did: Option<String>,
    ) -> Self {
        Self {
            kind: SkillReferenceKind::RegistryId {
                selector,
                prerelease: false,
            },
            skill_id: skill_id.into(),
            publisher_did,
            version: None,
            digest: None,
        }
    }

    pub fn registry_id_with_prerelease(
        skill_id: impl Into<String>,
        selector: SkillVersionSelector,
        publisher_did: Option<String>,
        allow_prerelease: bool,
    ) -> Self {
        Self {
            kind: SkillReferenceKind::RegistryId {
                selector,
                prerelease: allow_prerelease,
            },
            skill_id: skill_id.into(),
            publisher_did,
            version: None,
            digest: None,
        }
    }

    pub fn package_url(
        skill_id: impl Into<String>,
        package_url: impl Into<String>,
        publisher_did: impl Into<String>,
        version: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            kind: SkillReferenceKind::PackageUrl(package_url.into()),
            skill_id: skill_id.into(),
            publisher_did: Some(publisher_did.into()),
            version: Some(version.into()),
            digest: Some(digest.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillReferenceKind {
    LocalPath(PathBuf),
    PackageUrl(String),
    RegistryId {
        selector: SkillVersionSelector,
        prerelease: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillVersionSelector {
    Latest,
    Pinned(String),
    Rollback { before_version: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrySkillEntry {
    pub skill_id: String,
    pub publisher_did: String,
    pub merchant_did: Option<String>,
    pub version: String,
    pub digest: String,
    pub package_path: PathBuf,
    pub package_url: Option<String>,
    pub prerelease: bool,
}

impl RegistrySkillEntry {
    pub fn cache_key(&self) -> SkillCacheKey {
        SkillCacheKey {
            publisher_did: self.publisher_did.clone(),
            skill_id: self.skill_id.clone(),
            version: self.version.clone(),
            digest: self.digest.clone(),
        }
    }

    pub fn reference(&self) -> SkillReference {
        SkillReference {
            kind: match &self.package_url {
                Some(url) => SkillReferenceKind::PackageUrl(url.clone()),
                None => SkillReferenceKind::LocalPath(self.package_path.clone()),
            },
            skill_id: self.skill_id.clone(),
            publisher_did: Some(self.publisher_did.clone()),
            version: Some(self.version.clone()),
            digest: Some(self.digest.clone()),
        }
    }
}

pub trait SkillRegistry {
    fn resolve_skill(
        &self,
        reference: &SkillReference,
    ) -> Result<RegistrySkillEntry, SkillPackageError>;
}

#[derive(Debug, Clone, Default)]
pub struct LocalSkillRegistry {
    entries: Vec<RegistrySkillEntry>,
}

impl LocalSkillRegistry {
    pub fn new(entries: impl IntoIterator<Item = RegistrySkillEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    pub fn entries(&self) -> &[RegistrySkillEntry] {
        &self.entries
    }
}

impl SkillRegistry for LocalSkillRegistry {
    fn resolve_skill(
        &self,
        reference: &SkillReference,
    ) -> Result<RegistrySkillEntry, SkillPackageError> {
        match &reference.kind {
            SkillReferenceKind::LocalPath(path) => registry_entry_from_local_path(reference, path),
            SkillReferenceKind::PackageUrl(url) => self
                .entries
                .iter()
                .find(|entry| entry.package_url.as_deref() == Some(url.as_str()))
                .filter(|entry| entry_matches_reference(entry, reference))
                .cloned()
                .ok_or_else(|| registry_not_found(reference)),
            SkillReferenceKind::RegistryId {
                selector,
                prerelease,
            } => select_registry_entry(&self.entries, reference, selector, *prerelease),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCacheKey {
    pub publisher_did: String,
    pub skill_id: String,
    pub version: String,
    pub digest: String,
}

impl SkillCacheKey {
    pub fn directory_name(&self) -> String {
        format!(
            "{}__{}__{}__{}",
            sanitize_cache_part(&self.publisher_did),
            sanitize_cache_part(&self.skill_id),
            sanitize_cache_part(&self.version),
            sanitize_cache_part(&self.digest)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSkillMetadata {
    pub key: SkillCacheKey,
    pub merchant_did: Option<String>,
    pub package_source: PackageSourceSummary,
    pub package_ref: String,
    pub integrity: PackageIntegrityReport,
    pub readonly: bool,
    pub quarantined: bool,
    pub quarantine_reason: Option<String>,
    pub last_used_at_ms: u64,
}

impl CachedSkillMetadata {
    pub fn audit_summary(&self) -> Value {
        json!({
            "packageSource": self.package_source,
            "packageRef": self.package_ref,
            "publisherDid": self.key.publisher_did,
            "merchantDid": self.merchant_did,
            "skillId": self.key.skill_id,
            "version": self.key.version,
            "digest": {
                "algorithm": self.integrity.digest.algorithm,
                "value": self.integrity.digest.value,
            },
            "supplyChain": {
                "status": self.integrity.status.as_str(),
                "trustedPublisher": self.integrity.trusted_publisher,
                "quarantine": self.integrity.quarantine,
                "productionReady": self.integrity.production_ready,
                "issueCodes": self.integrity.issue_codes,
            },
            "cache": {
                "readonly": self.readonly,
                "quarantined": self.quarantined,
                "quarantineReason": self.quarantine_reason,
                "lastUsedAtMs": self.last_used_at_ms,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSkill {
    pub root: PathBuf,
    pub metadata: CachedSkillMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSourceSummary {
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillCacheCleanupScope {
    pub publisher_did: Option<String>,
    pub merchant_did: Option<String>,
    pub skill_id: Option<String>,
    pub version: Option<String>,
    pub digest: Option<String>,
}

impl SkillCacheCleanupScope {
    pub fn all() -> Self {
        Self::default()
    }

    pub fn publisher(mut self, publisher_did: impl Into<String>) -> Self {
        self.publisher_did = Some(publisher_did.into());
        self
    }

    pub fn merchant(mut self, merchant_did: impl Into<String>) -> Self {
        self.merchant_did = Some(merchant_did.into());
        self
    }

    pub fn skill(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn digest(mut self, digest: impl Into<String>) -> Self {
        self.digest = Some(digest.into());
        self
    }

    fn matches(&self, metadata: Option<&SkillCacheEntryMetadata>) -> bool {
        let Some(metadata) = metadata else {
            return self.publisher_did.is_none()
                && self.merchant_did.is_none()
                && self.skill_id.is_none()
                && self.version.is_none()
                && self.digest.is_none();
        };
        self.publisher_did
            .as_ref()
            .is_none_or(|publisher_did| &metadata.key.publisher_did == publisher_did)
            && self
                .merchant_did
                .as_ref()
                .is_none_or(|merchant_did| metadata.merchant_did.as_ref() == Some(merchant_did))
            && self
                .skill_id
                .as_ref()
                .is_none_or(|skill_id| &metadata.key.skill_id == skill_id)
            && self
                .version
                .as_ref()
                .is_none_or(|version| &metadata.key.version == version)
            && self
                .digest
                .as_ref()
                .is_none_or(|digest| &metadata.key.digest == digest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCacheCleanupPolicy {
    pub scope: SkillCacheCleanupScope,
    pub retain: BTreeSet<SkillCacheKey>,
    pub dry_run: bool,
    pub purge_quarantined: bool,
}

impl SkillCacheCleanupPolicy {
    pub fn dry_run(scope: SkillCacheCleanupScope) -> Self {
        Self {
            scope,
            retain: BTreeSet::new(),
            dry_run: true,
            purge_quarantined: true,
        }
    }

    pub fn delete_scope(scope: SkillCacheCleanupScope) -> Self {
        Self {
            scope,
            retain: BTreeSet::new(),
            dry_run: false,
            purge_quarantined: true,
        }
    }

    pub fn retain(mut self, key: SkillCacheKey) -> Self {
        self.retain.insert(key);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCacheCleanupReport {
    pub dry_run: bool,
    pub scanned_count: usize,
    pub matched_count: usize,
    pub retained_count: usize,
    pub removed_count: usize,
    pub skipped_count: usize,
    pub quarantined_count: usize,
    pub entries: Vec<SkillCacheCleanupEntry>,
    pub redaction: SkillCacheReportRedaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCacheCleanupEntry {
    pub cache_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<SkillCacheKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_did: Option<String>,
    pub action: SkillCacheCleanupAction,
    pub reason: String,
    pub retained_by_active_retain: bool,
    pub retained_by_rollback_pin: bool,
    pub quarantined: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillCacheCleanupAction {
    Retain,
    Remove,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCacheReportRedaction {
    pub marker: String,
    pub policy: String,
    pub root_path_visible: bool,
    pub package_url_secrets_visible: bool,
}

impl Default for SkillCacheReportRedaction {
    fn default() -> Self {
        Self {
            marker: "[REDACTED]".to_owned(),
            policy: CACHE_REDACTION_POLICY.to_owned(),
            root_path_visible: false,
            package_url_secrets_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCacheEntryMetadata {
    pub key: SkillCacheKey,
    pub merchant_did: Option<String>,
    pub package_source: PackageSourceSummary,
    pub package_ref: String,
    pub production_ready: bool,
    pub quarantined: bool,
    pub quarantine_reason: Option<String>,
    pub last_used_at_ms: u64,
}

impl SkillCacheEntryMetadata {
    fn minimal(key: SkillCacheKey, merchant_did: Option<String>, last_used_at_ms: u64) -> Self {
        Self {
            package_source: PackageSourceSummary {
                source_type: "unknown".to_owned(),
                url: None,
            },
            package_ref: format!("sha256:{}", key.digest),
            production_ready: false,
            quarantined: false,
            quarantine_reason: None,
            key,
            merchant_did,
            last_used_at_ms,
        }
    }
}

#[derive(Debug, Clone)]
struct ScannedSkillCacheEntry {
    cache_ref: String,
    cache_dir: PathBuf,
    metadata_path: PathBuf,
    metadata: Option<SkillCacheEntryMetadata>,
    has_cache_dir: bool,
}

pub struct SkillCache {
    root: PathBuf,
    rollback_pins: BTreeMap<String, SkillCacheKey>,
}

impl SkillCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            rollback_pins: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_or_insert(
        &mut self,
        entry: &RegistrySkillEntry,
        policy: &PackageIntegrityPolicy,
    ) -> Result<CachedSkill, SkillPackageError> {
        let key = entry.cache_key();
        if let Some(metadata) = self.read_metadata(&key)? {
            if metadata.quarantined {
                return Err(SkillPackageError::PackageQuarantined {
                    reason: metadata
                        .quarantine_reason
                        .unwrap_or_else(|| "cache_quarantined".to_owned()),
                });
            }
        }

        let source_root = resolve_skill_path(&entry.package_path)?;
        let actual_digest = compute_package_digest(&source_root)?;
        if actual_digest.value != entry.digest {
            return Err(SkillPackageError::PackageQuarantined {
                reason: "digest_mismatch".to_owned(),
            });
        }
        load_skill_with_integrity_policy(&source_root, policy)?;

        let cache_dir = self.root.join(key.directory_name());
        let root = if cache_dir.exists() {
            set_readonly_recursive(&cache_dir)?;
            cache_dir
        } else {
            fs::create_dir_all(&cache_dir).map_err(|source| SkillPackageError::ReadFile {
                path: cache_dir.clone(),
                source,
            })?;
            copy_package_dir(&source_root, &cache_dir)?;
            set_readonly_recursive(&cache_dir)?;
            cache_dir
        };

        let loaded = load_skill_with_integrity_policy(&root, policy)?;
        if loaded.integrity.digest.value != entry.digest {
            return Err(SkillPackageError::PackageQuarantined {
                reason: "digest_mismatch".to_owned(),
            });
        }
        let metadata = CachedSkillMetadata {
            key,
            merchant_did: entry.merchant_did.clone(),
            package_source: PackageSourceSummary {
                source_type: if entry.package_url.is_some() {
                    "package-url".to_owned()
                } else {
                    "registry-local-path".to_owned()
                },
                url: entry.package_url.as_deref().map(redacted_package_url),
            },
            package_ref: format!("sha256:{}", loaded.integrity.digest.value),
            integrity: loaded.integrity,
            readonly: is_readonly_package(&root)?,
            quarantined: false,
            quarantine_reason: None,
            last_used_at_ms: now_ms(),
        };
        self.write_metadata(&metadata)?;
        Ok(CachedSkill { root, metadata })
    }

    pub fn load_skill(
        &mut self,
        entry: &RegistrySkillEntry,
        policy: &PackageIntegrityPolicy,
    ) -> Result<(LoadedSkill, CachedSkillMetadata), SkillPackageError> {
        let cached = self.load_or_insert(entry, policy)?;
        let loaded = load_skill_with_integrity_policy(&cached.root, policy)?;
        Ok((loaded, cached.metadata))
    }

    pub fn pin_rollback(&mut self, key: SkillCacheKey) {
        self.rollback_pins
            .insert(rollback_scope(&key.publisher_did, &key.skill_id), key);
    }

    pub fn rollback_pin(&self, publisher_did: &str, skill_id: &str) -> Option<&SkillCacheKey> {
        self.rollback_pins
            .get(&rollback_scope(publisher_did, skill_id))
    }

    pub fn quarantine(
        &self,
        key: &SkillCacheKey,
        reason: impl Into<String>,
    ) -> Result<SkillCacheEntryMetadata, SkillPackageError> {
        let metadata = self
            .read_metadata(key)?
            .unwrap_or_else(|| SkillCacheEntryMetadata::minimal(key.clone(), None, now_ms()));
        let metadata = SkillCacheEntryMetadata {
            quarantined: true,
            quarantine_reason: Some(redact_cache_text(&reason.into())),
            production_ready: false,
            last_used_at_ms: now_ms(),
            ..metadata
        };
        self.write_entry_metadata(&metadata)?;
        Ok(metadata)
    }

    pub fn cleanup(
        &self,
        policy: SkillCacheCleanupPolicy,
    ) -> Result<SkillCacheCleanupReport, SkillPackageError> {
        let scanned = self.scan_entries()?;
        let rollback_pins = self
            .rollback_pins
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut report = SkillCacheCleanupReport {
            dry_run: policy.dry_run,
            scanned_count: scanned.len(),
            matched_count: 0,
            retained_count: 0,
            removed_count: 0,
            skipped_count: 0,
            quarantined_count: 0,
            entries: Vec::new(),
            redaction: SkillCacheReportRedaction::default(),
        };

        for entry in scanned {
            let key = entry.metadata.as_ref().map(|metadata| metadata.key.clone());
            let merchant_did = entry
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.merchant_did.clone());
            let cache_ref = entry.cache_ref.clone();
            let quarantined = entry
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.quarantined);
            if quarantined {
                report.quarantined_count += 1;
            }

            if !policy.scope.matches(entry.metadata.as_ref()) {
                report.skipped_count += 1;
                report.entries.push(SkillCacheCleanupEntry {
                    cache_ref,
                    key,
                    merchant_did,
                    action: SkillCacheCleanupAction::Skip,
                    reason: "scope_mismatch".to_owned(),
                    retained_by_active_retain: false,
                    retained_by_rollback_pin: false,
                    quarantined,
                });
                continue;
            }
            report.matched_count += 1;

            let retained_by_active_retain =
                key.as_ref().is_some_and(|key| policy.retain.contains(key));
            let retained_by_rollback_pin =
                key.as_ref().is_some_and(|key| rollback_pins.contains(key));
            if retained_by_active_retain || retained_by_rollback_pin {
                report.retained_count += 1;
                report.entries.push(SkillCacheCleanupEntry {
                    cache_ref,
                    key,
                    merchant_did,
                    action: SkillCacheCleanupAction::Retain,
                    reason: if retained_by_rollback_pin {
                        "rollback_pin".to_owned()
                    } else {
                        "active_retain".to_owned()
                    },
                    retained_by_active_retain,
                    retained_by_rollback_pin,
                    quarantined,
                });
                continue;
            }

            if quarantined && !policy.purge_quarantined {
                report.retained_count += 1;
                report.entries.push(SkillCacheCleanupEntry {
                    cache_ref,
                    key,
                    merchant_did,
                    action: SkillCacheCleanupAction::Retain,
                    reason: "quarantine_retained_by_policy".to_owned(),
                    retained_by_active_retain: false,
                    retained_by_rollback_pin: false,
                    quarantined,
                });
                continue;
            }

            if !policy.dry_run {
                self.remove_scanned_entry(&entry)?;
                report.removed_count += 1;
            }
            report.entries.push(SkillCacheCleanupEntry {
                cache_ref,
                key,
                merchant_did,
                action: SkillCacheCleanupAction::Remove,
                reason: if quarantined {
                    "quarantine_purge".to_owned()
                } else {
                    "matched_scope".to_owned()
                },
                retained_by_active_retain: false,
                retained_by_rollback_pin: false,
                quarantined,
            });
        }

        Ok(report)
    }

    pub fn evict_unpinned(&self, retain: impl IntoIterator<Item = SkillCacheKey>) -> usize {
        let policy = retain.into_iter().fold(
            SkillCacheCleanupPolicy::delete_scope(SkillCacheCleanupScope::all()),
            |policy, key| policy.retain(key),
        );
        self.cleanup(policy)
            .map(|report| report.removed_count)
            .unwrap_or(0)
    }

    fn read_metadata(
        &self,
        key: &SkillCacheKey,
    ) -> Result<Option<SkillCacheEntryMetadata>, SkillPackageError> {
        self.read_metadata_path(&metadata_path_for_key(&self.root, key))
    }

    fn read_metadata_path(
        &self,
        path: &Path,
    ) -> Result<Option<SkillCacheEntryMetadata>, SkillPackageError> {
        if !path.exists() {
            return Ok(None);
        }
        let source = fs::read_to_string(path).map_err(|source| SkillPackageError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&source).map(Some).map_err(|error| {
            SkillPackageError::InvalidPackageEntry {
                path: path.to_path_buf(),
                reason: format!("cache metadata is invalid: {error}"),
            }
        })
    }

    fn write_metadata(&self, metadata: &CachedSkillMetadata) -> Result<(), SkillPackageError> {
        self.write_entry_metadata(&SkillCacheEntryMetadata {
            key: metadata.key.clone(),
            merchant_did: metadata.merchant_did.clone(),
            package_source: metadata.package_source.clone(),
            package_ref: metadata.package_ref.clone(),
            production_ready: metadata.integrity.production_ready,
            quarantined: metadata.quarantined,
            quarantine_reason: metadata.quarantine_reason.clone(),
            last_used_at_ms: metadata.last_used_at_ms,
        })
    }

    fn write_entry_metadata(
        &self,
        metadata: &SkillCacheEntryMetadata,
    ) -> Result<(), SkillPackageError> {
        fs::create_dir_all(&self.root).map_err(|source| SkillPackageError::ReadFile {
            path: self.root.clone(),
            source,
        })?;
        let path = metadata_path_for_key(&self.root, &metadata.key);
        let tmp_path = path.with_extension("json.tmp");
        let source = serde_json::to_string(metadata).map_err(|error| {
            SkillPackageError::InvalidPackageEntry {
                path: path.clone(),
                reason: format!("cache metadata serialization failed: {error}"),
            }
        })?;
        fs::write(&tmp_path, source).map_err(|source| SkillPackageError::ReadFile {
            path: tmp_path.clone(),
            source,
        })?;
        fs::rename(&tmp_path, &path).map_err(|source| SkillPackageError::ReadFile { path, source })
    }

    fn scan_entries(&self) -> Result<Vec<ScannedSkillCacheEntry>, SkillPackageError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.root).map_err(|source| SkillPackageError::ReadFile {
            path: self.root.clone(),
            source,
        })?;
        let mut scanned = BTreeMap::<String, ScannedSkillCacheEntry>::new();
        for entry in entries {
            let entry = entry.map_err(|source| SkillPackageError::ReadFile {
                path: self.root.clone(),
                source,
            })?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if path.is_dir() {
                scanned
                    .entry(file_name.to_owned())
                    .or_insert_with(|| ScannedSkillCacheEntry {
                        cache_ref: file_name.to_owned(),
                        cache_dir: path.clone(),
                        metadata_path: self
                            .root
                            .join(format!("{file_name}{CACHE_METADATA_SUFFIX}")),
                        metadata: None,
                        has_cache_dir: true,
                    })
                    .has_cache_dir = true;
            } else if let Some(cache_ref) = file_name.strip_suffix(CACHE_METADATA_SUFFIX) {
                let metadata = self.read_metadata_path(&path)?;
                scanned
                    .entry(cache_ref.to_owned())
                    .or_insert_with(|| ScannedSkillCacheEntry {
                        cache_ref: cache_ref.to_owned(),
                        cache_dir: self.root.join(cache_ref),
                        metadata_path: path.clone(),
                        metadata: None,
                        has_cache_dir: false,
                    })
                    .metadata = metadata;
            }
        }
        for entry in scanned.values_mut() {
            if entry.metadata.is_none() {
                entry.metadata = self.read_metadata_path(&entry.metadata_path)?;
            }
        }
        Ok(scanned.into_values().collect())
    }

    fn remove_scanned_entry(
        &self,
        entry: &ScannedSkillCacheEntry,
    ) -> Result<(), SkillPackageError> {
        if entry.has_cache_dir && entry.cache_dir.exists() {
            make_writable_recursive(&entry.cache_dir)?;
            fs::remove_dir_all(&entry.cache_dir).map_err(|source| SkillPackageError::ReadFile {
                path: entry.cache_dir.clone(),
                source,
            })?;
        }
        if entry.metadata_path.exists() {
            fs::remove_file(&entry.metadata_path).map_err(|source| {
                SkillPackageError::ReadFile {
                    path: entry.metadata_path.clone(),
                    source,
                }
            })?;
        }
        Ok(())
    }
}

pub fn load_registry_skill<R: SkillRegistry>(
    registry: &R,
    cache: &mut SkillCache,
    reference: &SkillReference,
    policy: &PackageIntegrityPolicy,
) -> Result<(LoadedSkill, CachedSkillMetadata), SkillPackageError> {
    let entry = registry.resolve_skill(reference)?;
    cache.load_skill(&entry, policy)
}

fn registry_entry_from_local_path(
    reference: &SkillReference,
    path: &Path,
) -> Result<RegistrySkillEntry, SkillPackageError> {
    let root = resolve_skill_path(path)?;
    let digest = compute_package_digest(&root)?;
    Ok(RegistrySkillEntry {
        skill_id: reference.skill_id.clone(),
        publisher_did: reference
            .publisher_did
            .clone()
            .unwrap_or_else(|| "did:wba:local-dev.example".to_owned()),
        merchant_did: None,
        version: reference
            .version
            .clone()
            .unwrap_or_else(|| "0.0.0-local".to_owned()),
        digest: reference.digest.clone().unwrap_or(digest.value),
        package_path: root,
        package_url: None,
        prerelease: true,
    })
}

fn select_registry_entry(
    entries: &[RegistrySkillEntry],
    reference: &SkillReference,
    selector: &SkillVersionSelector,
    allow_prerelease: bool,
) -> Result<RegistrySkillEntry, SkillPackageError> {
    let mut candidates = entries
        .iter()
        .filter(|entry| entry_matches_reference(entry, reference))
        .filter(|entry| allow_prerelease || !entry.prerelease)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| compare_versions(&left.version, &right.version));

    match selector {
        SkillVersionSelector::Latest => candidates
            .last()
            .map(|entry| (*entry).clone())
            .ok_or_else(|| registry_not_found(reference)),
        SkillVersionSelector::Pinned(version) => candidates
            .into_iter()
            .find(|entry| &entry.version == version)
            .cloned()
            .ok_or_else(|| registry_not_found(reference)),
        SkillVersionSelector::Rollback { before_version } => candidates
            .into_iter()
            .filter(|entry| compare_versions(&entry.version, before_version) == Ordering::Less)
            .next_back()
            .cloned()
            .ok_or_else(|| registry_not_found(reference)),
    }
}

fn entry_matches_reference(entry: &RegistrySkillEntry, reference: &SkillReference) -> bool {
    if entry.skill_id != reference.skill_id {
        return false;
    }
    if reference
        .publisher_did
        .as_ref()
        .is_some_and(|publisher_did| publisher_did != &entry.publisher_did)
    {
        return false;
    }
    if reference
        .version
        .as_ref()
        .is_some_and(|version| version != &entry.version)
    {
        return false;
    }
    if reference
        .digest
        .as_ref()
        .is_some_and(|digest| digest != &entry.digest)
    {
        return false;
    }
    true
}

fn registry_not_found(reference: &SkillReference) -> SkillPackageError {
    SkillPackageError::PackageQuarantined {
        reason: format!("registry_entry_not_found:{}", reference.skill_id),
    }
}

fn copy_package_dir(source: &Path, target: &Path) -> Result<(), SkillPackageError> {
    for entry in fs::read_dir(source).map_err(|source_error| SkillPackageError::ReadFile {
        path: source.to_path_buf(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| SkillPackageError::ReadFile {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(|source_error| {
            SkillPackageError::ReadFile {
                path: source_path.clone(),
                source: source_error,
            }
        })?;
        if metadata.file_type().is_symlink() {
            let canonical =
                source_path
                    .canonicalize()
                    .map_err(|source_error| SkillPackageError::ReadFile {
                        path: source_path.clone(),
                        source: source_error,
                    })?;
            fs::copy(&canonical, &target_path).map_err(|source_error| {
                SkillPackageError::ReadFile {
                    path: canonical,
                    source: source_error,
                }
            })?;
        } else if metadata.is_dir() {
            fs::create_dir_all(&target_path).map_err(|source_error| {
                SkillPackageError::ReadFile {
                    path: target_path.clone(),
                    source: source_error,
                }
            })?;
            copy_package_dir(&source_path, &target_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).map_err(|source_error| {
                SkillPackageError::ReadFile {
                    path: source_path.clone(),
                    source: source_error,
                }
            })?;
        }
    }
    Ok(())
}

fn set_readonly_recursive(path: &Path) -> Result<(), SkillPackageError> {
    let metadata = fs::metadata(path).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|source| SkillPackageError::ReadFile {
            path: path.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| SkillPackageError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?;
            set_readonly_recursive(&entry.path())?;
        }
    }
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn make_writable_recursive(path: &Path) -> Result<(), SkillPackageError> {
    let metadata = fs::metadata(path).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    make_path_writable(path, &metadata)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|source| SkillPackageError::ReadFile {
            path: path.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| SkillPackageError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?;
            make_writable_recursive(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_path_writable(path: &Path, metadata: &fs::Metadata) -> Result<(), SkillPackageError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = metadata.permissions();
    let write_bits = if metadata.is_dir() { 0o700 } else { 0o600 };
    permissions.set_mode(permissions.mode() | write_bits);
    fs::set_permissions(path, permissions).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn make_path_writable(path: &Path, metadata: &fs::Metadata) -> Result<(), SkillPackageError> {
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn is_readonly_package(path: &Path) -> Result<bool, SkillPackageError> {
    let metadata = fs::metadata(path).map_err(|source| SkillPackageError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.permissions().readonly() {
        return Ok(false);
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|source| SkillPackageError::ReadFile {
            path: path.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| SkillPackageError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?;
            if !is_readonly_package(&entry.path())? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn sanitize_cache_part(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

fn rollback_scope(publisher_did: &str, skill_id: &str) -> String {
    format!("{publisher_did}/{skill_id}")
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let (left_main, left_pre) = split_prerelease(left);
    let (right_main, right_pre) = split_prerelease(right);
    let left_parts = version_parts(left_main);
    let right_parts = version_parts(right_main);
    let max_len = left_parts.len().max(right_parts.len());
    for index in 0..max_len {
        let left_part = left_parts.get(index).copied().unwrap_or(0);
        let right_part = right_parts.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    match (left_pre, right_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left_pre), Some(right_pre)) => left_pre.cmp(right_pre),
    }
}

fn split_prerelease(version: &str) -> (&str, Option<&str>) {
    match version.split_once('-') {
        Some((main, prerelease)) => (main, Some(prerelease)),
        None => (version, None),
    }
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn redacted_package_url(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if [
        "authorization",
        "signature",
        "token",
        "secret",
        "credential",
        "private",
        "password",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return "[REDACTED]".to_owned();
    }
    url.split(['?', '#']).next().unwrap_or(url).to_owned()
}

fn metadata_path_for_key(root: &Path, key: &SkillCacheKey) -> PathBuf {
    root.join(format!("{}{}", key.directory_name(), CACHE_METADATA_SUFFIX))
}

fn redact_cache_text(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if [
        "authorization",
        "signature",
        "token",
        "secret",
        "credential",
        "private",
        "password",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || lower.contains("/tmp/")
        || lower.contains("\\users\\")
        || lower.contains("/home/")
    {
        return "[REDACTED]".to_owned();
    }
    text.to_owned()
}

fn now_ms() -> u64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
