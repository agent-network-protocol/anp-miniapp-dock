use std::fmt;
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

#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for DidCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DidCredential")
            .field("did_document_path", &"[CONFIGURED]")
            .field("private_key_path", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DidCredentialConfig {
    pub did_document_path: PathBuf,
    pub private_key_path: PathBuf,
    pub check_private_key_permissions: bool,
}

impl DidCredentialConfig {
    pub fn new(
        did_document_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            did_document_path: did_document_path.into(),
            private_key_path: private_key_path.into(),
            check_private_key_permissions: true,
        }
    }

    pub fn without_private_key_permission_check(mut self) -> Self {
        self.check_private_key_permissions = false;
        self
    }

    pub fn validate(&self) -> Result<DidCredential, DidCredentialError> {
        validate_readable_file("DID document", &self.did_document_path)?;
        validate_readable_file("DID private key", &self.private_key_path)?;
        if self.check_private_key_permissions {
            validate_private_key_permissions(&self.private_key_path)?;
        }
        Ok(DidCredential::new(
            self.did_document_path.clone(),
            self.private_key_path.clone(),
        ))
    }
}

impl fmt::Debug for DidCredentialConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DidCredentialConfig")
            .field("did_document_path", &"[CONFIGURED]")
            .field("private_key_path", &"[REDACTED]")
            .field(
                "check_private_key_permissions",
                &self.check_private_key_permissions,
            )
            .finish()
    }
}

pub trait DidCredentialProvider: Clone {
    fn credential_for(
        &self,
        session: &IdentitySession,
    ) -> Result<DidCredential, DidCredentialError>;
}

#[derive(Clone)]
pub struct FileDidCredentialProvider {
    did_document_path: PathBuf,
    private_key_path: PathBuf,
    check_private_key_permissions: bool,
}

impl FileDidCredentialProvider {
    pub fn new(
        did_document_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            did_document_path: did_document_path.into(),
            private_key_path: private_key_path.into(),
            check_private_key_permissions: true,
        }
    }

    pub fn from_config(config: DidCredentialConfig) -> Result<Self, DidCredentialError> {
        config.validate()?;
        Ok(Self {
            did_document_path: config.did_document_path,
            private_key_path: config.private_key_path,
            check_private_key_permissions: config.check_private_key_permissions,
        })
    }

    pub fn did_document_path(&self) -> &Path {
        &self.did_document_path
    }

    pub fn private_key_path(&self) -> &Path {
        &self.private_key_path
    }
}

impl fmt::Debug for FileDidCredentialProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileDidCredentialProvider")
            .field("did_document_path", &"[CONFIGURED]")
            .field("private_key_path", &"[REDACTED]")
            .field(
                "check_private_key_permissions",
                &self.check_private_key_permissions,
            )
            .finish()
    }
}

impl DidCredentialProvider for FileDidCredentialProvider {
    fn credential_for(
        &self,
        _session: &IdentitySession,
    ) -> Result<DidCredential, DidCredentialError> {
        let mut config = DidCredentialConfig::new(
            self.did_document_path.clone(),
            self.private_key_path.clone(),
        );
        config.check_private_key_permissions = self.check_private_key_permissions;
        config.validate()
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DidCredentialError {
    #[error("DID credential is unavailable for session")]
    Unavailable,

    #[error("{kind} path is missing")]
    MissingPath { kind: &'static str },

    #[error("{kind} path is not a readable file")]
    Unreadable { kind: &'static str, reason: String },

    #[error("DID private key permissions are too broad")]
    PermissionTooBroad,

    #[error("DID identity configuration is invalid")]
    InvalidIdentity,
}

fn validate_readable_file(kind: &'static str, path: &Path) -> Result<(), DidCredentialError> {
    if path.as_os_str().is_empty() {
        return Err(DidCredentialError::MissingPath { kind });
    }
    let metadata = std::fs::metadata(path).map_err(|error| DidCredentialError::Unreadable {
        kind,
        reason: error.kind().to_string(),
    })?;
    if !metadata.is_file() {
        return Err(DidCredentialError::Unreadable {
            kind,
            reason: "not a file".to_owned(),
        });
    }
    std::fs::File::open(path).map_err(|error| DidCredentialError::Unreadable {
        kind,
        reason: error.kind().to_string(),
    })?;
    Ok(())
}

#[cfg(unix)]
fn validate_private_key_permissions(path: &Path) -> Result<(), DidCredentialError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path).map_err(|error| DidCredentialError::Unreadable {
        kind: "DID private key",
        reason: error.kind().to_string(),
    })?;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(DidCredentialError::PermissionTooBroad);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_private_key_permissions(_path: &Path) -> Result<(), DidCredentialError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn file_provider_returns_paths_without_exposing_key_content() {
        let fixture = CredentialFixture::new();
        let provider =
            FileDidCredentialProvider::from_config(fixture.config()).expect("config is valid");
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

        assert_eq!(credential.did_document_path, fixture.did_path);
        assert_eq!(credential.private_key_path, fixture.key_path);
        assert_eq!(provider.did_document_path(), fixture.did_path.as_path());
        assert_eq!(provider.private_key_path(), fixture.key_path.as_path());
    }

    #[test]
    fn missing_did_document_fails_closed() {
        let fixture = CredentialFixture::new();
        fs::remove_file(&fixture.did_path).expect("remove DID document");

        let error = FileDidCredentialProvider::from_config(fixture.config())
            .expect_err("missing DID document must fail");

        assert!(matches!(
            error,
            DidCredentialError::Unreadable {
                kind: "DID document",
                ..
            }
        ));
    }

    #[test]
    fn missing_private_key_fails_closed() {
        let fixture = CredentialFixture::new();
        fs::remove_file(&fixture.key_path).expect("remove key");

        let error = FileDidCredentialProvider::from_config(fixture.config())
            .expect_err("missing key must fail");

        assert!(matches!(
            error,
            DidCredentialError::Unreadable {
                kind: "DID private key",
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn broad_private_key_permissions_fail_closed() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = CredentialFixture::new();
        fs::set_permissions(&fixture.key_path, fs::Permissions::from_mode(0o644))
            .expect("set broad permissions");

        let error = FileDidCredentialProvider::from_config(fixture.config())
            .expect_err("broad key permissions must fail");

        assert_eq!(error, DidCredentialError::PermissionTooBroad);
    }

    struct CredentialFixture {
        _dir: TempDir,
        did_path: PathBuf,
        key_path: PathBuf,
    }

    impl CredentialFixture {
        fn new() -> Self {
            let dir = TempDir::new("anp-miniapp-dock-did").expect("temp dir");
            let did_path = dir.path().join("did.json");
            let key_path = dir.path().join("key.pem");
            fs::write(&did_path, br#"{"id":"did:wba:user.example"}"#).expect("write DID");
            fs::write(&key_path, "test-only-key").expect("write key");
            set_private_key_permissions(&key_path);
            Self {
                _dir: dir,
                did_path,
                key_path,
            }
        }

        fn config(&self) -> DidCredentialConfig {
            DidCredentialConfig::new(self.did_path.clone(), self.key_path.clone())
        }
    }

    #[cfg(unix)]
    fn set_private_key_permissions(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("set key permissions");
    }

    #[cfg(not(unix))]
    fn set_private_key_permissions(_path: &Path) {}

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!(
                "{}-{}-{}",
                prefix,
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_suffix() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        format!("{nanos}-{counter}")
    }
}
