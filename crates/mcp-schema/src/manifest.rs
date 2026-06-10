use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    #[serde(default)]
    pub apis: Vec<ApiDeclaration>,
    #[serde(default)]
    pub components: Vec<ComponentDeclaration>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDeclaration {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<ManifestMeta>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ApiDeclaration {
    pub fn component_path(&self) -> Option<&str> {
        self.meta
            .as_ref()
            .and_then(|meta| meta.ui.as_ref())
            .and_then(|ui| ui.component_path.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDeclaration {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_page: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<ManifestMeta>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anp: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_path: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
