use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardActionKind {
    SendFollowUpMessage,
    ApiCall,
    ExpireCards,
    OpenDetailPageFallback,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardAction {
    pub kind: CardActionKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl CardAction {
    pub fn api_call(label: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            kind: CardActionKind::ApiCall,
            label: label.into(),
            target: Some(name.into()),
            payload: Some(arguments),
        }
    }

    pub fn send_follow_up(label: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            kind: CardActionKind::SendFollowUpMessage,
            label: label.into(),
            target: None,
            payload: Some(Value::String(content.into())),
        }
    }

    pub fn noop(label: impl Into<String>) -> Self {
        Self {
            kind: CardActionKind::Noop,
            label: label.into(),
            target: None,
            payload: None,
        }
    }
}
