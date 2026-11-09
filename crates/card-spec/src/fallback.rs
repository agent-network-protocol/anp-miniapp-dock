use crate::schema::{CardItem, CardSection, CardSpec, CardStatus};
use mcp_schema::AtomicApiResult;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    NoComponentPath,
    ComponentMissing,
    ComponentLoadFailed,
    ComponentVmFailed,
    WxmlParseFailed,
    WxssParseWarningThreshold,
    UnsupportedNodeKind,
    HostRendererUnavailable,
    ApiError,
    EmptyStructuredContent,
}

impl FallbackReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoComponentPath => "no_component_path",
            Self::ComponentMissing => "component_missing",
            Self::ComponentLoadFailed => "component_load_failed",
            Self::ComponentVmFailed => "component_vm_failed",
            Self::WxmlParseFailed => "wxml_parse_failed",
            Self::WxssParseWarningThreshold => "wxss_parse_warning_threshold",
            Self::UnsupportedNodeKind => "unsupported_node_kind",
            Self::HostRendererUnavailable => "host_renderer_unavailable",
            Self::ApiError => "api_error",
            Self::EmptyStructuredContent => "empty_structured_content",
        }
    }

    pub fn from_stable_str(reason: &str) -> Option<Self> {
        match reason {
            "no_component_path" => Some(Self::NoComponentPath),
            "component_missing" => Some(Self::ComponentMissing),
            "component_load_failed" => Some(Self::ComponentLoadFailed),
            "component_vm_failed" => Some(Self::ComponentVmFailed),
            "wxml_parse_failed" => Some(Self::WxmlParseFailed),
            "wxss_parse_warning_threshold" => Some(Self::WxssParseWarningThreshold),
            "unsupported_node_kind" => Some(Self::UnsupportedNodeKind),
            "host_renderer_unavailable" => Some(Self::HostRendererUnavailable),
            "api_error" => Some(Self::ApiError),
            "empty_structured_content" => Some(Self::EmptyStructuredContent),
            _ => None,
        }
    }

    pub fn normalize(reason: &str) -> Self {
        if let Some(reason) = Self::from_stable_str(reason) {
            return reason;
        }
        if reason.contains("no_component_path") {
            Self::NoComponentPath
        } else if reason.contains("component_missing") {
            Self::ComponentMissing
        } else if reason.contains("component_load") {
            Self::ComponentLoadFailed
        } else if reason.contains("wxml") && reason.contains("parse") {
            Self::WxmlParseFailed
        } else if reason.contains("wxss") {
            Self::WxssParseWarningThreshold
        } else if reason.contains("unsupported_node") || reason.contains("unsupported WXML tag") {
            Self::UnsupportedNodeKind
        } else if reason.contains("render_failed") || reason.contains("component_vm") {
            Self::ComponentVmFailed
        } else {
            Self::HostRendererUnavailable
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
