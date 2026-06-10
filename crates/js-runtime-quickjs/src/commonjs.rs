use crate::api_vm::ApiVmError;
use serde_json::{Map, Value};
use skill_loader::{LoadedSkill, SourceFile};
use std::collections::BTreeMap;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonJsModule {
    pub id: String,
    pub filename: String,
    pub dirname: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonJsModules {
    modules: BTreeMap<String, CommonJsModule>,
}

impl CommonJsModules {
    pub fn from_skill(skill: &LoadedSkill) -> Result<Self, ApiVmError> {
        let mut modules = BTreeMap::new();
        insert_source(&mut modules, &skill.entry_js)?;
        for source in skill.api_modules.values() {
            insert_source(&mut modules, source)?;
        }

        Ok(Self { modules })
    }

    pub fn module_ids(&self) -> Vec<String> {
        self.modules.keys().cloned().collect()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.modules.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<&CommonJsModule> {
        self.modules.get(id)
    }

    pub fn to_json_value(&self) -> Value {
        let modules = self
            .modules
            .iter()
            .map(|(id, module)| {
                let mut value = Map::new();
                value.insert("id".to_owned(), Value::String(module.id.clone()));
                value.insert(
                    "filename".to_owned(),
                    Value::String(module.filename.clone()),
                );
                value.insert("dirname".to_owned(), Value::String(module.dirname.clone()));
                value.insert("source".to_owned(), Value::String(module.source.clone()));
                (id.clone(), Value::Object(value))
            })
            .collect();

        Value::Object(modules)
    }
}

fn insert_source(
    modules: &mut BTreeMap<String, CommonJsModule>,
    source: &SourceFile,
) -> Result<(), ApiVmError> {
    let id = module_id(&source.relative_path)?;
    modules.insert(
        id.clone(),
        CommonJsModule {
            dirname: module_dirname(&id),
            filename: id.clone(),
            id,
            source: source.source.clone(),
        },
    );
    Ok(())
}

fn module_id(path: &Path) -> Result<String, ApiVmError> {
    if path.is_absolute() {
        return Err(ApiVmError::UnsafeRequire(
            "absolute module paths are not allowed".to_owned(),
        ));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let Some(part) = part.to_str() else {
                    return Err(ApiVmError::UnsafeRequire(
                        "module path is not valid UTF-8".to_owned(),
                    ));
                };
                parts.push(part.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ApiVmError::UnsafeRequire(
                    "module path cannot contain parent segments".to_owned(),
                ));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(ApiVmError::UnsafeRequire(
                    "absolute module paths are not allowed".to_owned(),
                ));
            }
        }
    }

    let Some(last) = parts.last_mut() else {
        return Err(ApiVmError::UnsafeRequire("empty module path".to_owned()));
    };
    if last.ends_with(".js") {
        last.truncate(last.len() - 3);
    }

    Ok(parts.join("/"))
}

fn module_dirname(id: &str) -> String {
    id.rsplit_once('/')
        .map(|(dirname, _)| dirname.to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_segments() {
        let source = SourceFile {
            relative_path: Path::new("apis").join("../secret.js"),
            absolute_path: Path::new("/tmp/skill/secret.js").to_path_buf(),
            source: String::new(),
        };

        assert!(matches!(
            module_id(&source.relative_path),
            Err(ApiVmError::UnsafeRequire(_))
        ));
    }

    #[test]
    fn canonicalizes_js_extension() {
        assert_eq!(
            module_id(Path::new("apis/searchDrinks.js")).unwrap(),
            "apis/searchDrinks"
        );
    }
}
