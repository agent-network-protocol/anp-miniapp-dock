use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillPackageError {
    #[error("skill root `{path}` is not a directory")]
    SkillRootNotDirectory { path: PathBuf },

    #[error("required file `{path}` is missing")]
    MissingRequiredFile { path: PathBuf },

    #[error("path `{path}` is not inside skill root `{root}`")]
    PathEscapesSkillRoot { root: PathBuf, path: PathBuf },

    #[error("absolute paths are not allowed in skill packages: `{path}`")]
    AbsolutePath { path: PathBuf },

    #[error("failed to read `{path}`: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse mcp.json `{path}`: {source}")]
    ParseManifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "mcp.json validation failed with {error_count} error(s) and {warning_count} warning(s)"
    )]
    InvalidManifest {
        error_count: usize,
        warning_count: usize,
    },
}

pub fn resolve_skill_path(path: impl AsRef<Path>) -> Result<PathBuf, SkillPackageError> {
    let path = path.as_ref();
    let root = path
        .canonicalize()
        .map_err(|_| SkillPackageError::SkillRootNotDirectory {
            path: path.to_path_buf(),
        })?;

    if root.is_dir() {
        Ok(root)
    } else {
        Err(SkillPackageError::SkillRootNotDirectory { path: root })
    }
}

pub fn resolve_package_path(
    skill_root: impl AsRef<Path>,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, SkillPackageError> {
    let skill_root = skill_root.as_ref();
    let relative_path = relative_path.as_ref();

    if relative_path.is_absolute() {
        return Err(SkillPackageError::AbsolutePath {
            path: relative_path.to_path_buf(),
        });
    }

    if relative_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        let canonical_root =
            skill_root
                .canonicalize()
                .map_err(|_| SkillPackageError::SkillRootNotDirectory {
                    path: skill_root.to_path_buf(),
                })?;
        let candidate = skill_root.join(relative_path);
        let path = candidate.canonicalize().unwrap_or(candidate);
        return Err(SkillPackageError::PathEscapesSkillRoot {
            root: canonical_root,
            path,
        });
    }

    let candidate = skill_root.join(relative_path);
    let canonical =
        candidate
            .canonicalize()
            .map_err(|_| SkillPackageError::MissingRequiredFile {
                path: candidate.clone(),
            })?;

    validate_inside_skill_root(skill_root, canonical)
}

pub fn resolve_api_module(
    skill_root: impl AsRef<Path>,
    api_name: &str,
) -> Result<PathBuf, SkillPackageError> {
    resolve_package_path(skill_root, Path::new("apis").join(format!("{api_name}.js")))
}

pub fn resolve_component_path(
    skill_root: impl AsRef<Path>,
    component_path: &str,
) -> Result<PathBuf, SkillPackageError> {
    let skill_root = skill_root.as_ref();
    let relative_path = Path::new(component_path);

    if relative_path.is_absolute() {
        return Err(SkillPackageError::AbsolutePath {
            path: relative_path.to_path_buf(),
        });
    }

    let candidate = skill_root.join(relative_path);
    if candidate.is_dir() {
        return validate_inside_skill_root(
            skill_root,
            candidate
                .canonicalize()
                .map_err(|_| SkillPackageError::MissingRequiredFile { path: candidate })?,
        );
    }

    let component_dir = if relative_path
        .file_name()
        .is_some_and(|name| name == "index")
    {
        relative_path.parent().unwrap_or(relative_path)
    } else {
        relative_path
    };

    resolve_package_path(skill_root, component_dir)
}

pub fn validate_inside_skill_root(
    skill_root: impl AsRef<Path>,
    path: impl AsRef<Path>,
) -> Result<PathBuf, SkillPackageError> {
    let skill_root = skill_root.as_ref();
    let path = path.as_ref();
    let canonical_root =
        skill_root
            .canonicalize()
            .map_err(|_| SkillPackageError::SkillRootNotDirectory {
                path: skill_root.to_path_buf(),
            })?;
    let canonical_path =
        path.canonicalize()
            .map_err(|_| SkillPackageError::MissingRequiredFile {
                path: path.to_path_buf(),
            })?;

    if canonical_path.starts_with(&canonical_root) {
        Ok(canonical_path)
    } else {
        Err(SkillPackageError::PathEscapesSkillRoot {
            root: canonical_root,
            path: canonical_path,
        })
    }
}
