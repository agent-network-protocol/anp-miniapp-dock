use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    #[serde(default)]
    pub apis: Vec<ApiDeclaration>,
    #[serde(default)]
    pub components: Vec<ComponentDeclaration>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<ManifestMeta>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SkillManifest {
    pub fn anp_meta(&self) -> Option<&Value> {
        self.meta.as_ref().and_then(|meta| meta.anp.as_ref())
    }

    pub fn supply_chain_meta(&self) -> Option<&Value> {
        self.anp_meta().and_then(|anp| anp.get("supplyChain"))
    }
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

    pub fn input_formats(&self) -> Vec<InputFormatField> {
        let mut fields = Vec::new();
        collect_input_formats("", &self.input_schema, &mut fields);
        fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputFormatField {
    pub path: String,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDeclaration {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<ComponentPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_page: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expirable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expired_text: Option<String>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<ManifestMeta>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ComponentDeclaration {
    pub fn dynamic_permission(&self) -> Option<&Value> {
        self.permissions
            .as_ref()
            .and_then(|permissions| permissions.scope_dynamic.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPermissions {
    #[serde(
        default,
        rename = "scope.dynamic",
        skip_serializing_if = "Option::is_none"
    )]
    pub scope_dynamic: Option<Value>,
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

fn collect_input_formats(prefix: &str, schema: &Value, fields: &mut Vec<InputFormatField>) {
    let Some(object) = schema.as_object() else {
        return;
    };

    if let Some(format) = object.get("format").and_then(Value::as_str) {
        if matches!(format, "image" | "file") {
            fields.push(InputFormatField {
                path: prefix.to_owned(),
                format: format.to_owned(),
            });
        }
    }

    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            let child_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            collect_input_formats(&child_path, child, fields);
        }
    }

    if let Some(items) = object.get("items") {
        let child_path = if prefix.is_empty() {
            "[]".to_owned()
        } else {
            format!("{prefix}[]")
        };
        collect_input_formats(&child_path, items, fields);
    }

    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(schemas) = object.get(keyword).and_then(Value::as_array) {
            for (index, child) in schemas.iter().enumerate() {
                let child_path = if prefix.is_empty() {
                    format!("{keyword}[{index}]")
                } else {
                    format!("{prefix}.{keyword}[{index}]")
                };
                collect_input_formats(&child_path, child, fields);
            }
        }
    }
}
