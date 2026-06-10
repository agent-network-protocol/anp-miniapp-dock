use crate::render_ir::{RenderEventBinding, RenderEventKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentEventKind {
    Tap,
    ImageLoad,
    ImageError,
}

impl From<&RenderEventKind> for ComponentEventKind {
    fn from(kind: &RenderEventKind) -> Self {
        match kind {
            RenderEventKind::Tap => Self::Tap,
            RenderEventKind::ImageLoad => Self::ImageLoad,
            RenderEventKind::ImageError => Self::ImageError,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentEvent {
    pub kind: ComponentEventKind,
    pub method: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub dataset: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub detail: Map<String, Value>,
}

impl ComponentEvent {
    pub fn new(kind: ComponentEventKind, method: impl Into<String>) -> Self {
        Self {
            kind,
            method: method.into(),
            dataset: Map::new(),
            detail: Map::new(),
        }
    }

    pub fn from_binding(binding: &RenderEventBinding) -> Self {
        let dataset = normalize_dataset(&binding.dataset);
        Self {
            kind: ComponentEventKind::from(&binding.event),
            method: binding.method.clone(),
            dataset,
            detail: Map::new(),
        }
    }

    pub fn to_js_event(&self) -> Value {
        json!({
            "type": self.kind,
            "currentTarget": {
                "dataset": self.dataset
            },
            "target": {
                "dataset": self.dataset
            },
            "detail": self.detail
        })
    }
}

fn normalize_dataset(dataset: &Map<String, Value>) -> Map<String, Value> {
    let mut normalized = dataset.clone();
    for (key, value) in dataset {
        if key.contains('-') {
            normalized
                .entry(kebab_to_camel(key))
                .or_insert_with(|| value.clone());
        }
    }
    normalized
}

fn kebab_to_camel(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_ir::{RenderEventBinding, RenderEventKind};
    use serde_json::json;

    #[test]
    fn data_attributes_are_available_as_original_and_camel_case_dataset_keys() {
        let mut binding = RenderEventBinding::new(RenderEventKind::Tap, "payOrder");
        binding
            .dataset
            .insert("order-id".to_owned(), json!("order_demo_001"));

        let event = ComponentEvent::from_binding(&binding);

        assert_eq!(
            event.dataset.get("order-id"),
            Some(&json!("order_demo_001"))
        );
        assert_eq!(event.dataset.get("orderId"), Some(&json!("order_demo_001")));
    }
}
