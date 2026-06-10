use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySession {
    pub user_did: String,
    pub agent_did: Option<String>,
    pub merchant_did: String,
    pub skill_id: String,
    pub session_id: String,
}

impl IdentitySession {
    pub fn new(
        user_did: impl Into<String>,
        agent_did: Option<String>,
        merchant_did: impl Into<String>,
        skill_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            user_did: user_did.into(),
            agent_did,
            merchant_did: merchant_did.into(),
            skill_id: skill_id.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DidCredential {
    pub did_document_path: PathBuf,
    pub private_key_path: PathBuf,
}

impl DidCredential {
    pub fn new(
        did_document_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            did_document_path: did_document_path.into(),
            private_key_path: private_key_path.into(),
        }
    }
}

pub trait DidCredentialProvider: Clone {
    fn credential_for(
        &self,
        session: &IdentitySession,
    ) -> Result<DidCredential, DidCredentialError>;
}

#[derive(Debug, Clone)]
pub struct FileDidCredentialProvider {
    did_document_path: PathBuf,
    private_key_path: PathBuf,
}

impl FileDidCredentialProvider {
    pub fn new(
        did_document_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            did_document_path: did_document_path.into(),
            private_key_path: private_key_path.into(),
        }
    }

    pub fn did_document_path(&self) -> &Path {
        &self.did_document_path
    }
}

impl DidCredentialProvider for FileDidCredentialProvider {
    fn credential_for(
        &self,
        _session: &IdentitySession,
    ) -> Result<DidCredential, DidCredentialError> {
        Ok(DidCredential::new(
            self.did_document_path.clone(),
            self.private_key_path.clone(),
        ))
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DidCredentialError {
    #[error("DID credential is unavailable for session")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_provider_returns_paths_without_exposing_key_content() {
        let provider = FileDidCredentialProvider::new("did.json", "key.pem");
        let session = IdentitySession::new(
            "did:wba:user.example",
            None,
            "did:wba:merchant.example",
            "coffee",
            "session-1",
        );

        let credential = provider
            .credential_for(&session)
            .expect("credential is available");

        assert_eq!(credential.did_document_path, PathBuf::from("did.json"));
        assert_eq!(provider.did_document_path(), Path::new("did.json"));
    }
}
