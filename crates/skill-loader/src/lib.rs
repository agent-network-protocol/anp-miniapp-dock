#![doc = "MiniApp MCP Skill package loading and path resolution crate."]

pub mod integrity;
pub mod package;
pub mod registry;
pub mod resolver;

pub use integrity::{
    compute_package_digest, development_signature_value, validate_archive_entry_path,
    verify_package_integrity, PackageDigest, PackageIntegrityPolicy, PackageIntegrityProfile,
    PackageIntegrityReport, PackageIntegrityStatus, PackageSignature, PackageSupplyChainContract,
    DEVELOPMENT_SIGNATURE_ALGORITHM, PACKAGE_DIGEST_ALGORITHM,
};
pub use package::{
    load_skill, load_skill_with_integrity_policy, LoadedComponent, LoadedSkill, SourceFile,
};
pub use registry::{
    load_registry_skill, CachedSkill, CachedSkillMetadata, LocalSkillRegistry,
    PackageSourceSummary, RegistrySkillEntry, SkillCache, SkillCacheCleanupAction,
    SkillCacheCleanupEntry, SkillCacheCleanupPolicy, SkillCacheCleanupReport,
    SkillCacheCleanupScope, SkillCacheEntryMetadata, SkillCacheKey, SkillCacheReportRedaction,
    SkillReference, SkillReferenceKind, SkillRegistry, SkillVersionSelector,
};
pub use resolver::{
    resolve_api_module, resolve_component_path, resolve_package_path, resolve_skill_path,
    validate_inside_skill_root, SkillPackageError,
};
