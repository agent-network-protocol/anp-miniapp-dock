use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicApiResult {
    #[serde(default)]
    pub is_error: bool,
    pub content: Vec<TextContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Map<String, Value>>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Map<String, Value>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl AtomicApiResult {
    pub fn model_visible(&self) -> ModelVisibleApiResult {
        ModelVisibleApiResult {
            is_error: self.is_error,
            content: self.content.clone(),
            structured_content: self.structured_content.clone(),
        }
    }

    pub fn should_render_component(&self) -> bool {
        !self.is_error
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVisibleApiResult {
    pub is_error: bool,
    pub content: Vec<TextContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: TextContentType,
    pub text: String,
}

impl TextContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: TextContentType::Text,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextContentType {
    Text,
}
