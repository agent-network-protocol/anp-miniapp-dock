use crate::storage::StorageScope;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedPage {
    pub path: String,
    #[serde(default)]
    pub query: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardEvent {
    ExpireAllCards {
        component_paths: Vec<String>,
        match_policy: Option<String>,
    },
    ExpirePreviousCards {
        component_paths: Vec<String>,
        match_policy: Option<String>,
    },
    SetRelatedPage(RelatedPage),
}

pub trait CardEventSink {
    fn record(&self, event: CardEvent);
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryCardEventSink {
    events: Arc<Mutex<Vec<CardEvent>>>,
}

impl InMemoryCardEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<CardEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl CardEventSink for InMemoryCardEventSink {
    fn record(&self, event: CardEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub model: String,
    pub language: String,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            platform: "anp-miniapp-dock".to_owned(),
            model: "host-runtime".to_owned(),
            language: "en".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelContext {
    session_id: String,
    skill_id: String,
    user_did: String,
    merchant_did: String,
}

impl ModelContext {
    pub fn new(
        session_id: impl Into<String>,
        skill_id: impl Into<String>,
        user_did: impl Into<String>,
        merchant_did: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            skill_id: skill_id.into(),
            user_did: user_did.into(),
            merchant_did: merchant_did.into(),
        }
    }

    pub fn create_skill(&self, skill_path: impl Into<String>) -> SkillHandle {
        SkillHandle {
            skill_path: skill_path.into(),
            skill_id: self.skill_id.clone(),
            session_id: self.session_id.clone(),
        }
    }

    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }

    pub fn storage_scope(&self) -> StorageScope {
        StorageScope::new(
            self.user_did.clone(),
            self.merchant_did.clone(),
            self.skill_id.clone(),
        )
    }

    pub fn expire_all_cards(
        &self,
        sink: &impl CardEventSink,
        component_paths: impl IntoIterator<Item = impl Into<String>>,
        match_policy: Option<String>,
    ) {
        sink.record(CardEvent::ExpireAllCards {
            component_paths: component_paths.into_iter().map(Into::into).collect(),
            match_policy,
        });
    }

    pub fn expire_previous_cards(
        &self,
        sink: &impl CardEventSink,
        component_paths: impl IntoIterator<Item = impl Into<String>>,
        match_policy: Option<String>,
    ) {
        sink.record(CardEvent::ExpirePreviousCards {
            component_paths: component_paths.into_iter().map(Into::into).collect(),
            match_policy,
        });
    }

    pub fn set_related_page(&self, sink: &impl CardEventSink, related_page: RelatedPage) {
        sink.record(CardEvent::SetRelatedPage(related_page));
    }

    pub fn get_device_info(&self) -> DeviceInfo {
        DeviceInfo::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillHandle {
    pub skill_path: String,
    pub skill_id: String,
    pub session_id: String,
}
