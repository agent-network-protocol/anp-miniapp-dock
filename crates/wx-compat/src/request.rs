use crate::permissions::{Capability, CapabilityProfile, PermissionDecision};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WxMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WxRequest {
    pub url: String,
    pub method: WxMethod,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl WxRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: WxMethod::Get,
            headers: BTreeMap::new(),
            data: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WxResponse {
    pub status_code: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub data: Value,
}

impl WxResponse {
    pub fn json(status_code: u16, data: Value) -> Self {
        Self {
            status_code,
            headers: BTreeMap::new(),
            data,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum WxRequestError {
    #[error("request denied: {0}")]
    Denied(String),

    #[error("request unsupported: {0}")]
    Unsupported(String),
}

pub trait RequestBroker {
    fn request(
        &self,
        profile: &CapabilityProfile,
        request: WxRequest,
    ) -> Result<WxResponse, WxRequestError>;
}

#[derive(Debug, Clone, Default)]
pub struct UnsupportedRequestBroker;

impl RequestBroker for UnsupportedRequestBroker {
    fn request(
        &self,
        profile: &CapabilityProfile,
        _request: WxRequest,
    ) -> Result<WxResponse, WxRequestError> {
        match profile.check(Capability::Request) {
            PermissionDecision::Allow => Err(WxRequestError::Unsupported(
                "wx.request is defined by wx-compat but real ANP HTTP is implemented in Step 08"
                    .to_owned(),
            )),
            PermissionDecision::Deny { reason, .. } => Err(WxRequestError::Denied(reason)),
        }
    }
}

pub fn unsupported_api(name: &str) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert(
        "errMsg".to_owned(),
        Value::String(format!("{name}:unsupported")),
    );
    value.insert(
        "reason".to_owned(),
        Value::String("mapped to ANP runtime or later integration step".to_owned()),
    );
    value
}
