use crate::error::{DockCoreError, ErrorCode};
use mcp_schema::{ApiDeclaration, SkillManifest};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct RegisteredApi {
    pub declaration: ApiDeclaration,
}

#[derive(Debug, Clone, Default)]
pub struct ApiRegistry {
    apis: BTreeMap<String, RegisteredApi>,
}

impl ApiRegistry {
    pub fn from_manifest(manifest: &SkillManifest) -> Self {
        let apis = manifest
            .apis
            .iter()
            .map(|api| {
                (
                    api.name.clone(),
                    RegisteredApi {
                        declaration: api.clone(),
                    },
                )
            })
            .collect();

        Self { apis }
    }

    pub fn get(&self, api_name: &str) -> Result<&RegisteredApi, DockCoreError> {
        self.apis.get(api_name).ok_or_else(|| {
            DockCoreError::core(
                ErrorCode::ApiNotFound,
                format!("API `{api_name}` is not registered"),
            )
        })
    }

    pub fn contains(&self, api_name: &str) -> bool {
        self.apis.contains_key(api_name)
    }

    pub fn len(&self) -> usize {
        self.apis.len()
    }

    pub fn is_empty(&self) -> bool {
        self.apis.is_empty()
    }
}
