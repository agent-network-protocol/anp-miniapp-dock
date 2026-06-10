use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChallenge {
    pub challenge_id: String,
    pub merchant_did: String,
    pub nonce: String,
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeLoginRequest {
    pub session_id: String,
    pub skill_id: String,
    pub user_did: String,
    pub agent_did: Option<String>,
    pub merchant_did: String,
    pub challenge_id: String,
    pub signed_challenge: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeLoginResponse {
    pub capability_token: String,
    pub expires_at_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn challenge_login_contract_is_camel_case() {
        let request = ChallengeLoginRequest {
            session_id: "session-1".to_owned(),
            skill_id: "coffee".to_owned(),
            user_did: "did:wba:user.example".to_owned(),
            agent_did: Some("did:wba:agent.example".to_owned()),
            merchant_did: "did:wba:merchant.example".to_owned(),
            challenge_id: "challenge-1".to_owned(),
            signed_challenge: json!({"proof": "mock"}),
        };

        let encoded = serde_json::to_value(request).expect("request serializes");

        assert_eq!(encoded["sessionId"], "session-1");
        assert_eq!(encoded["capabilityToken"], Value::Null);
        assert_eq!(encoded["signedChallenge"]["proof"], "mock");
    }
}
