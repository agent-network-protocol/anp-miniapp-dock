use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderNodeKind {
    View,
    Text,
    Image,
    Button,
    ScrollView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNode {
    pub id: String,
    pub kind: RenderNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
    #[serde(default, skip_serializing_if = "RenderStyle::is_empty")]
    pub style: RenderStyle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<RenderEventBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<RenderNode>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub accessibility: Map<String, Value>,
}

impl RenderNode {
    pub fn new(id: impl Into<String>, kind: RenderNodeKind) -> Self {
        Self {
            id: id.into(),
            kind,
            text: None,
            props: Map::new(),
            style: RenderStyle::default(),
            events: Vec::new(),
            children: Vec::new(),
            accessibility: Map::new(),
        }
    }

    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut node = Self::new(id, RenderNodeKind::Text);
        node.text = Some(text.into());
        node
    }

    pub fn with_child(mut self, child: RenderNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_event(mut self, event: RenderEventBinding) -> Self {
        self.events.push(event);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flex_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

impl RenderStyle {
    pub fn is_empty(&self) -> bool {
        self.display.is_none()
            && self.flex_direction.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.margin.is_none()
            && self.padding.is_none()
            && self.color.is_none()
            && self.background.is_none()
            && self.opacity.is_none()
            && self.font_size.is_none()
            && self.font_weight.is_none()
            && self.line_height.is_none()
            && self.border.is_none()
            && self.border_radius.is_none()
            && self.text_align.is_none()
            && self.extra.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderEventKind {
    Tap,
    ImageLoad,
    ImageError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderEventBinding {
    pub event: RenderEventKind,
    pub method: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub dataset: Map<String, Value>,
}

impl RenderEventBinding {
    pub fn new(event: RenderEventKind, method: impl Into<String>) -> Self {
        Self {
            event,
            method: method.into(),
            dataset: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ComponentAction {
    SendFollowUpMessage {
        content: String,
    },
    ApiCall {
        name: String,
        arguments: Value,
    },
    ExpireCards {
        component_paths: Vec<String>,
    },
    OpenDetailPageFallback {
        path: String,
        query: Map<String, Value>,
    },
    Noop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_ir_serializes_platform_neutral_node() {
        let node = RenderNode::new("root", RenderNodeKind::View)
            .with_child(RenderNode::text("title", "Latte"))
            .with_event(RenderEventBinding::new(RenderEventKind::Tap, "confirm"));

        assert_eq!(
            serde_json::to_value(node).unwrap(),
            json!({
                "id": "root",
                "kind": "view",
                "events": [{ "event": "tap", "method": "confirm" }],
                "children": [{ "id": "title", "kind": "text", "text": "Latte" }]
            })
        );
    }

    #[test]
    fn supports_p0_node_kinds() {
        let kinds = [
            RenderNodeKind::View,
            RenderNodeKind::Text,
            RenderNodeKind::Image,
            RenderNodeKind::Button,
            RenderNodeKind::ScrollView,
        ];

        assert_eq!(kinds.len(), 5);
    }
}
