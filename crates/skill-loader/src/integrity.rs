use crate::resolver::{resolve_skill_path, validate_inside_skill_root, SkillPackageError};
use mcp_schema::SkillManifest;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const PACKAGE_DIGEST_ALGORITHM: &str = "sha256";
pub const DEVELOPMENT_SIGNATURE_ALGORITHM: &str = "dock.package.dev-sha256.v1";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDigest {
    pub algorithm: String,
    pub value: String,
}

impl PackageDigest {
    pub fn sha256(value: impl Into<String>) -> Self {
        Self {
            algorithm: PACKAGE_DIGEST_ALGORITHM.to_owned(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSignature {
    pub algorithm: String,
    pub value: String,
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSupplyChainContract {
    pub publisher_did: String,
    pub digest: PackageDigest,
    pub signature: PackageSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageIntegrityStatus {
    Verified,
    DemoUnsigned,
    Quarantined,
}

impl PackageIntegrityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::DemoUnsigned => "demo-unsigned",
            Self::Quarantined => "quarantined",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIntegrityReport {
    pub digest: PackageDigest,
    pub status: PackageIntegrityStatus,
    pub publisher_did: Option<String>,
    pub signature_algorithm: Option<String>,
    pub signature_key_id: Option<String>,
    pub trusted_publisher: bool,
    pub quarantine: bool,
    pub production_ready: bool,
    pub issue_codes: Vec<String>,
    pub warnings: Vec<String>,
}

impl PackageIntegrityReport {
    pub fn development_unsigned(digest: PackageDigest) -> Self {
        Self {
            digest,
            status: PackageIntegrityStatus::DemoUnsigned,
            publisher_did: None,
            signature_algorithm: None,
            signature_key_id: None,
            trusted_publisher: false,
            quarantine: false,
            production_ready: false,
            issue_codes: vec!["unsigned_dev_only".to_owned()],
            warnings: vec![
                "Skill package is unsigned; local loading is allowed only for dev/demo profiles."
                    .to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageIntegrityProfile {
    Development,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIntegrityPolicy {
    pub profile: PackageIntegrityProfile,
    pub trusted_publishers: BTreeSet<String>,
}

impl PackageIntegrityPolicy {
    pub fn development() -> Self {
        Self {
            profile: PackageIntegrityProfile::Development,
            trusted_publishers: BTreeSet::new(),
        }
    }

    pub fn production<I, S>(trusted_publishers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            profile: PackageIntegrityProfile::Production,
            trusted_publishers: trusted_publishers.into_iter().map(Into::into).collect(),
        }
    }

    pub fn is_production(&self) -> bool {
        self.profile == PackageIntegrityProfile::Production
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSupplyChainContract {
    publisher_did: Option<String>,
    digest: Option<PackageDigest>,
    signature: Option<PackageSignature>,
}

pub fn compute_package_digest(
    skill_root: impl AsRef<Path>,
) -> Result<PackageDigest, SkillPackageError> {
    let root = resolve_skill_path(skill_root)?;
    let mut files = Vec::new();
    collect_package_files(&root, &root, &mut files)?;
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let mut hasher = Sha256::new();
    for file in files {
        let relative_path = path_to_package_string(&file.relative_path)?;
        let bytes = read_digest_bytes(&file.absolute_path, &file.relative_path)?;
        hasher.update(b"file\0");
        hasher.update(relative_path.as_bytes());
        hasher.update(b"\0");
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(b"\0");
        hasher.update(bytes);
        hasher.update(b"\0");
    }

    Ok(PackageDigest::sha256(hex_lower(&hasher.finalize())))
}

pub fn verify_package_integrity(
    skill_root: impl AsRef<Path>,
    manifest: &SkillManifest,
    policy: &PackageIntegrityPolicy,
) -> Result<PackageIntegrityReport, SkillPackageError> {
    let digest = compute_package_digest(skill_root)?;
    let contract = parse_supply_chain_contract(manifest);

    let report = match contract {
        None => unsigned_report(digest, policy),
        Some(Err(message)) => quarantined_report(
            digest,
            None,
            None,
            None,
            false,
            "invalid_supply_chain_contract",
            message,
        ),
        Some(Ok(contract)) => verify_contract(digest, contract, policy),
    };

    Ok(report)
}

pub fn validate_archive_entry_path(entry_path: &str) -> Result<PathBuf, SkillPackageError> {
    if entry_path.trim().is_empty()
        || entry_path.contains('\0')
        || entry_path.contains("://")
        || entry_path.contains('\\')
    {
        return Err(SkillPackageError::ZipSlipPath {
            path: entry_path.to_owned(),
        });
    }

    let path = Path::new(entry_path);
    if path.is_absolute() {
        return Err(SkillPackageError::AbsolutePath {
            path: path.to_path_buf(),
        });
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(SkillPackageError::ZipSlipPath {
                    path: entry_path.to_owned(),
                });
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(SkillPackageError::ZipSlipPath {
            path: entry_path.to_owned(),
        });
    }

    Ok(normalized)
}

pub fn development_signature_value(
    publisher_did: &str,
    key_id: &str,
    digest: &PackageDigest,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DEVELOPMENT_SIGNATURE_ALGORITHM.as_bytes());
    hasher.update(b"\n");
    hasher.update(publisher_did.as_bytes());
    hasher.update(b"\n");
    hasher.update(key_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(digest.algorithm.as_bytes());
    hasher.update(b"\n");
    hasher.update(digest.value.as_bytes());
    hex_lower(&hasher.finalize())
}

fn unsigned_report(
    digest: PackageDigest,
    policy: &PackageIntegrityPolicy,
) -> PackageIntegrityReport {
    let mut report = PackageIntegrityReport::development_unsigned(digest);
    if policy.is_production() {
        report.status = PackageIntegrityStatus::Quarantined;
        report.quarantine = true;
        report.issue_codes = vec!["missing_supply_chain_metadata".to_owned()];
        report.warnings = vec![
            "Production profile requires publisher DID, digest, signature, and trusted publisher allowlist."
                .to_owned(),
        ];
    }
    report
}

fn verify_contract(
    digest: PackageDigest,
    contract: PackageSupplyChainContract,
    policy: &PackageIntegrityPolicy,
) -> PackageIntegrityReport {
    let publisher_did = Some(contract.publisher_did.clone());
    let signature_algorithm = Some(contract.signature.algorithm.clone());
    let signature_key_id = Some(contract.signature.key_id.clone());
    let trusted_publisher = policy.trusted_publishers.contains(&contract.publisher_did);

    if contract.digest.algorithm != PACKAGE_DIGEST_ALGORITHM {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            trusted_publisher,
            "unsupported_digest_algorithm",
            "Package digest algorithm must be sha256.",
        );
    }

    if !is_sha256_hex(&contract.digest.value) || contract.digest.value != digest.value {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            trusted_publisher,
            "digest_mismatch",
            "Package digest does not match the normalized package content.",
        );
    }

    if !contract.publisher_did.starts_with("did:") {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            trusted_publisher,
            "invalid_publisher_did",
            "Package publisherDid must be a DID.",
        );
    }

    if contract.signature.algorithm != DEVELOPMENT_SIGNATURE_ALGORITHM {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            trusted_publisher,
            "unsupported_signature_algorithm",
            "No verifier is registered for the package signature algorithm.",
        );
    }

    let expected =
        development_signature_value(&contract.publisher_did, &contract.signature.key_id, &digest);
    if contract.signature.value != expected {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            trusted_publisher,
            "signature_mismatch",
            "Package signature does not verify against the normalized package digest.",
        );
    }

    if !trusted_publisher {
        return quarantined_report(
            digest,
            publisher_did,
            signature_algorithm,
            signature_key_id,
            false,
            "unknown_publisher",
            "Publisher DID is not in the trusted publisher allowlist.",
        );
    }

    PackageIntegrityReport {
        digest,
        status: PackageIntegrityStatus::Verified,
        publisher_did,
        signature_algorithm,
        signature_key_id,
        trusted_publisher: true,
        quarantine: false,
        production_ready: true,
        issue_codes: Vec::new(),
        warnings: Vec::new(),
    }
}

fn quarantined_report(
    digest: PackageDigest,
    publisher_did: Option<String>,
    signature_algorithm: Option<String>,
    signature_key_id: Option<String>,
    trusted_publisher: bool,
    issue_code: &str,
    warning: impl Into<String>,
) -> PackageIntegrityReport {
    PackageIntegrityReport {
        digest,
        status: PackageIntegrityStatus::Quarantined,
        publisher_did,
        signature_algorithm,
        signature_key_id,
        trusted_publisher,
        quarantine: true,
        production_ready: false,
        issue_codes: vec![issue_code.to_owned()],
        warnings: vec![warning.into()],
    }
}

fn parse_supply_chain_contract(
    manifest: &SkillManifest,
) -> Option<Result<PackageSupplyChainContract, String>> {
    let value = manifest.supply_chain_meta()?;
    let raw: RawSupplyChainContract = match serde_json::from_value(value.clone()) {
        Ok(raw) => raw,
        Err(_) => return Some(Err("supplyChain metadata has an invalid shape.".to_owned())),
    };

    let publisher_did = match raw.publisher_did {
        Some(publisher_did) if !publisher_did.trim().is_empty() => publisher_did,
        _ => return Some(Err("supplyChain.publisherDid is required.".to_owned())),
    };
    let Some(digest) = raw.digest else {
        return Some(Err("supplyChain.digest is required.".to_owned()));
    };
    let Some(signature) = raw.signature else {
        return Some(Err("supplyChain.signature is required.".to_owned()));
    };

    Some(Ok(PackageSupplyChainContract {
        publisher_did,
        digest,
        signature,
    }))
}

#[derive(Debug)]
struct PackageFile {
    relative_path: PathBuf,
    absolute_path: PathBuf,
}

fn collect_package_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PackageFile>,
) -> Result<(), SkillPackageError> {
    for entry in fs::read_dir(directory).map_err(|source| SkillPackageError::ReadFile {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillPackageError::ReadFile {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| SkillPackageError::ReadFile {
                path: path.clone(),
                source,
            })?;

        if metadata.file_type().is_symlink() {
            let canonical = path
                .canonicalize()
                .map_err(|source| SkillPackageError::ReadFile {
                    path: path.clone(),
                    source,
                })?;
            validate_inside_skill_root(root, &canonical)?;
            let target_metadata =
                fs::metadata(&canonical).map_err(|source| SkillPackageError::ReadFile {
                    path: canonical.clone(),
                    source,
                })?;
            if target_metadata.is_dir() {
                return Err(SkillPackageError::InvalidPackageEntry {
                    path,
                    reason: "symlinked directories are not supported in Skill packages".to_owned(),
                });
            }
        } else if metadata.is_dir() {
            collect_package_files(root, &path, files)?;
            continue;
        } else if !metadata.is_file() {
            return Err(SkillPackageError::InvalidPackageEntry {
                path,
                reason: "only regular files and in-package file symlinks are allowed".to_owned(),
            });
        }

        let relative_path =
            path.strip_prefix(root)
                .map_err(|_| SkillPackageError::PathEscapesSkillRoot {
                    root: root.to_path_buf(),
                    path: path.clone(),
                })?;
        validate_archive_entry_path(&path_to_package_string(relative_path)?)?;
        files.push(PackageFile {
            relative_path: relative_path.to_path_buf(),
            absolute_path: path,
        });
    }

    Ok(())
}

fn read_digest_bytes(
    absolute_path: &Path,
    relative_path: &Path,
) -> Result<Vec<u8>, SkillPackageError> {
    let bytes = fs::read(absolute_path).map_err(|source| SkillPackageError::ReadFile {
        path: absolute_path.to_path_buf(),
        source,
    })?;

    if relative_path == Path::new("mcp.json") {
        Ok(normalize_manifest_bytes_for_digest(&bytes))
    } else {
        Ok(bytes)
    }
}

fn normalize_manifest_bytes_for_digest(bytes: &[u8]) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<Value>(bytes) else {
        return bytes.to_vec();
    };
    remove_supply_chain_metadata(&mut value);
    serde_json::to_vec(&value).unwrap_or_else(|_| bytes.to_vec())
}

fn remove_supply_chain_metadata(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(meta) = root.get_mut("_meta").and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(anp) = meta.get_mut("anp").and_then(Value::as_object_mut) {
        anp.remove("supplyChain");
        if anp.is_empty() {
            meta.remove("anp");
        }
    }
    if meta.is_empty() {
        root.remove("_meta");
    }
}

fn path_to_package_string(path: &Path) -> Result<String, SkillPackageError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let Some(part) = part.to_str() else {
                    return Err(SkillPackageError::InvalidPackageEntry {
                        path: path.to_path_buf(),
                        reason: "package path is not valid UTF-8".to_owned(),
                    });
                };
                parts.push(part.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(SkillPackageError::ZipSlipPath {
                    path: path.display().to_string(),
                });
            }
        }
    }
    Ok(parts.join("/"))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
