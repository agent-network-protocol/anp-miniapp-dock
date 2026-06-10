use crate::schema::{CardItem, CardSection, CardSpec, CardStatus};
use mcp_schema::AtomicApiResult;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    NoComponentPath,
    ComponentLoadFailed,
    ComponentRenderFailed,
    WxmlUnsupported,
    RendererUnavailable,
    ApiError,
    EmptyStructuredContent,
}

impl FallbackReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoComponentPath => "no_component_path",
            Self::ComponentLoadFailed => "component_load_failed",
            Self::ComponentRenderFailed => "component_render_failed",
            Self::WxmlUnsupported => "wxml_unsupported",
            Self::RendererUnavailable => "renderer_unavailable",
            Self::ApiError => "api_error",
            Self::EmptyStructuredContent => "empty_structured_content",
        }
    }
}

pub fn fallback_from_result(result: &AtomicApiResult, reason: FallbackReason) -> CardSpec {
    if result.is_error {
        return text_card(result, CardStatus::Error, FallbackReason::ApiError);
    }

    if let Some(structured_content) = &result.structured_content {
        if !structured_content.is_empty() {
            return structured_card(structured_content, reason);
        }
    }

    if result.content.is_empty() {
        CardSpec::new("No content", CardStatus::Empty, reason.as_str()).with_section(
            CardSection::new("Fallback").with_item(CardItem::text("No displayable content")),
        )
    } else {
        text_card(result, CardStatus::Normal, reason)
    }
}

fn structured_card(structured_content: &Map<String, Value>, reason: FallbackReason) -> CardSpec {
    let mut section = CardSection::new("Structured content");
    for (key, value) in structured_content {
        section = section.with_item(CardItem::field(key.clone(), value.clone()));
    }

    CardSpec::new("Response", CardStatus::Normal, reason.as_str()).with_section(section)
}

fn text_card(result: &AtomicApiResult, status: CardStatus, reason: FallbackReason) -> CardSpec {
    let mut section = CardSection::new("Message");
    for content in &result.content {
        section = section.with_item(CardItem::text(content.text.clone()));
    }

    CardSpec::new(
        if status == CardStatus::Error {
            "Error"
        } else {
            "Response"
        },
        status,
        reason.as_str(),
    )
    .with_section(section)
}
