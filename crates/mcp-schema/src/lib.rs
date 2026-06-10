#![doc = "MiniApp MCP manifest, result, and validation contract crate."]

pub mod manifest;
pub mod result;
pub mod validation;

pub use manifest::{ApiDeclaration, ComponentDeclaration, ManifestMeta, SkillManifest, UiMeta};
pub use result::{AtomicApiResult, ModelVisibleApiResult, TextContent};
pub use validation::{
    validate_api_result, validate_input, validate_manifest, validate_manifest_with_component_paths,
    validate_output_warning, ValidationIssue, ValidationIssueLevel, ValidationReport,
};
