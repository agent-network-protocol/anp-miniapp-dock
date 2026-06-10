#![doc = "MiniApp MCP Skill package loading and path resolution crate."]

pub mod package;
pub mod resolver;

pub use package::{load_skill, LoadedComponent, LoadedSkill, SourceFile};
pub use resolver::{
    resolve_api_module, resolve_component_path, resolve_package_path, resolve_skill_path,
    validate_inside_skill_root, SkillPackageError,
};
