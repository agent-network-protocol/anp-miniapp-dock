use crate::actions::CardAction;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardStatus {
    Normal,
    Error,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSpec {
    pub version: String,
    pub title: String,
    pub status: CardStatus,
    pub fallback_reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<CardSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<CardAction>,
}

impl CardSpec {
    pub fn new(
        title: impl Into<String>,
        status: CardStatus,
        fallback_reason: impl Into<String>,
    ) -> Self {
        Self {
            version: "card-spec/v0".to_owned(),
            title: title.into(),
            status,
            fallback_reason: fallback_reason.into(),
            sections: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn with_section(mut self, section: CardSection) -> Self {
        self.sections.push(section);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSection {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<CardItem>,
}

impl CardSection {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
        }
    }

    pub fn with_item(mut self, item: CardItem) -> Self {
        self.items.push(item);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CardItem {
    Text { text: String },
    Image { url: String, alt: Option<String> },
    Button { label: String, action: CardAction },
    Field { label: String, value: Value },
    List { items: Vec<CardItem> },
}

impl CardItem {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn field(label: impl Into<String>, value: Value) -> Self {
        Self::Field {
            label: label.into(),
            value,
        }
    }
}
