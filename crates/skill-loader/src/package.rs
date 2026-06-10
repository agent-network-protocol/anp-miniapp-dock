use crate::resolver::{
    resolve_component_path, resolve_package_path, resolve_skill_path, SkillPackageError,
};
use mcp_schema::{validate_manifest_with_component_paths, SkillManifest, ValidationReport};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub root: PathBuf,
    pub skill_md: SourceFile,
    pub manifest: SkillManifest,
    pub entry_js: SourceFile,
    pub api_modules: BTreeMap<String, SourceFile>,
    pub components: BTreeMap<String, LoadedComponent>,
    pub component_routes: BTreeMap<String, String>,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone)]
pub struct LoadedComponent {
    pub route: String,
    pub directory: PathBuf,
    pub index_js: Option<SourceFile>,
    pub index_wxml: Option<SourceFile>,
    pub index_wxss: Option<SourceFile>,
    pub index_json: Option<SourceFile>,
}

pub fn load_skill(skill_root: impl AsRef<Path>) -> Result<LoadedSkill, SkillPackageError> {
    let root = resolve_skill_path(skill_root)?;
    let skill_md = read_required_file(&root, "SKILL.md")?;
    let manifest_file = read_required_file(&root, "mcp.json")?;
    let manifest: SkillManifest =
        serde_json::from_str(&manifest_file.source).map_err(|source| {
            SkillPackageError::ParseManifest {
                path: manifest_file.relative_path.clone(),
                source,
            }
        })?;
    let entry_js = read_required_file(&root, "index.js")?;
    let api_modules = discover_api_modules(&root)?;
    let discovered_component_paths = discover_component_paths(&root)?;
    let validation_component_paths = component_validation_paths(&discovered_component_paths);
    let validation = validate_manifest_with_component_paths(
        &manifest,
        validation_component_paths.iter().map(String::as_str),
    );

    if !validation.is_valid() {
        return Err(SkillPackageError::InvalidManifest {
            error_count: validation.errors.len(),
            warning_count: validation.warnings.len(),
        });
    }

    let components = load_components(&root, &manifest, &discovered_component_paths)?;
    let component_routes = manifest
        .apis
        .iter()
        .filter_map(|api| {
            api.component_path()
                .map(|component_path| (api.name.clone(), component_path.to_owned()))
        })
        .collect();

    Ok(LoadedSkill {
        root,
        skill_md,
        manifest,
        entry_js,
        api_modules,
        components,
        component_routes,
        validation,
    })
}

fn read_required_file(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<SourceFile, SkillPackageError> {
    let relative_path = relative_path.as_ref();
    let absolute_path = resolve_package_path(root, relative_path)?;
    let source =
        fs::read_to_string(&absolute_path).map_err(|source| SkillPackageError::ReadFile {
            path: absolute_path.clone(),
            source,
        })?;

    Ok(SourceFile {
        relative_path: relative_path.to_path_buf(),
        absolute_path,
        source,
    })
}

fn read_optional_file(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<Option<SourceFile>, SkillPackageError> {
    let relative_path = relative_path.as_ref();
    if !root.join(relative_path).exists() {
        return Ok(None);
    }

    read_required_file(root, relative_path).map(Some)
}

fn discover_api_modules(root: &Path) -> Result<BTreeMap<String, SourceFile>, SkillPackageError> {
    let api_dir = root.join("apis");
    if !api_dir.exists() {
        return Ok(BTreeMap::new());
    }

    let mut modules = BTreeMap::new();
    for entry in fs::read_dir(&api_dir).map_err(|source| SkillPackageError::ReadFile {
        path: api_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillPackageError::ReadFile {
            path: api_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "js") {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let relative_path = Path::new("apis").join(format!("{stem}.js"));
            modules.insert(stem.to_owned(), read_required_file(root, relative_path)?);
        }
    }

    Ok(modules)
}

fn discover_component_paths(root: &Path) -> Result<Vec<String>, SkillPackageError> {
    let components_dir = root.join("components");
    if !components_dir.exists() {
        return Ok(Vec::new());
    }

    let mut component_paths = Vec::new();
    for entry in fs::read_dir(&components_dir).map_err(|source| SkillPackageError::ReadFile {
        path: components_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillPackageError::ReadFile {
            path: components_dir.clone(),
            source,
        })?;
        if entry.path().is_dir() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            component_paths.push(format!("components/{name}/index"));
        }
    }

    Ok(component_paths)
}

fn component_validation_paths(component_paths: &[String]) -> Vec<String> {
    let mut validation_paths = Vec::with_capacity(component_paths.len() * 2);
    for component_path in component_paths {
        validation_paths.push(component_path.clone());
        if let Some(component_dir) = component_path.strip_suffix("/index") {
            validation_paths.push(component_dir.to_owned());
        }
    }
    validation_paths
}

fn load_components(
    root: &Path,
    manifest: &SkillManifest,
    discovered_component_paths: &[String],
) -> Result<BTreeMap<String, LoadedComponent>, SkillPackageError> {
    let mut route_paths: Vec<String> = manifest
        .components
        .iter()
        .map(|component| component.path.clone())
        .collect();
    route_paths.extend(discovered_component_paths.iter().cloned());
    route_paths.sort();
    route_paths.dedup();

    let mut components = BTreeMap::new();
    for route in route_paths {
        let directory = resolve_component_path(root, &route)?;
        let relative_dir = directory
            .strip_prefix(root)
            .unwrap_or(directory.as_path())
            .to_path_buf();

        let component = LoadedComponent {
            route: route.clone(),
            directory,
            index_js: read_optional_file(root, relative_dir.join("index.js"))?,
            index_wxml: read_optional_file(root, relative_dir.join("index.wxml"))?,
            index_wxss: read_optional_file(root, relative_dir.join("index.wxss"))?,
            index_json: read_optional_file(root, relative_dir.join("index.json"))?,
        };

        components.insert(route, component);
    }

    Ok(components)
}
