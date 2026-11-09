use skill_loader::{
    compute_package_digest, development_signature_value, load_skill,
    load_skill_with_integrity_policy, resolve_package_path, validate_archive_entry_path,
    validate_inside_skill_root, PackageDigest, PackageIntegrityPolicy, PackageIntegrityStatus,
    SkillPackageError, DEVELOPMENT_SIGNATURE_ALGORITHM,
};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/skill-loader")
        .to_path_buf()
}

fn coffee_skill_root() -> PathBuf {
    repo_root().join("examples/coffee-skill")
}

#[test]
fn loads_coffee_skill_fixture() {
    let skill = load_skill(coffee_skill_root()).expect("coffee skill should load");

    assert!(skill.skill_md.source.contains("Coffee Order Skill"));
    assert_eq!(skill.manifest.apis.len(), 3);
    assert_eq!(skill.api_modules.len(), 3);
    assert_eq!(skill.components.len(), 3);
    assert_eq!(
        skill
            .component_routes
            .get("searchDrinks")
            .map(String::as_str),
        Some("components/drink-list/index")
    );
    assert_eq!(
        skill
            .component_routes
            .get("confirmOrder")
            .map(String::as_str),
        Some("components/order-confirm/index")
    );
    assert_eq!(
        skill.component_routes.get("payOrder").map(String::as_str),
        Some("components/payment-result/index")
    );
    assert!(skill.validation.is_valid());
    assert_eq!(skill.integrity.status, PackageIntegrityStatus::DemoUnsigned);
    assert!(!skill.integrity.production_ready);
    assert_eq!(skill.integrity.digest.algorithm, "sha256");
    assert_eq!(skill.integrity.digest.value.len(), 64);
    assert!(skill
        .integrity
        .issue_codes
        .iter()
        .any(|code| code == "unsigned_dev_only"));
}

#[test]
fn missing_required_file_is_explicit_error() {
    let temp = TestSkillDir::new("missing-required");
    temp.write("mcp.json", r#"{"apis":[]}"#);
    temp.write("index.js", "module.exports = {}");

    let error = load_skill(temp.path()).expect_err("missing SKILL.md should fail");

    assert!(matches!(
        error,
        SkillPackageError::MissingRequiredFile { .. }
    ));
    assert!(error.to_string().contains("SKILL.md"));
}

#[test]
fn invalid_component_path_fails_manifest_validation() {
    let temp = TestSkillDir::new("bad-component");
    temp.write("SKILL.md", "# Test Skill");
    temp.write("index.js", "module.exports = {}");
    temp.write(
        "mcp.json",
        r#"{
          "apis": [
            {
              "name": "bad",
              "description": "bad component path",
              "_meta": { "ui": { "componentPath": "components/missing/index" } },
              "inputSchema": {}
            }
          ],
          "components": []
        }"#,
    );

    let error = load_skill(temp.path()).expect_err("missing componentPath should fail");

    assert!(matches!(
        error,
        SkillPackageError::InvalidManifest { error_count: 1, .. }
    ));
}

#[test]
fn component_path_traversal_fails_closed() {
    let temp = TestSkillDir::new("component-traversal");
    temp.write("SKILL.md", "# Test Skill");
    temp.write("index.js", "module.exports = {}");
    temp.write(
        "mcp.json",
        r#"{
          "apis": [],
          "components": [
            { "path": "../outside" }
          ]
        }"#,
    );

    let error = load_skill(temp.path()).expect_err("component traversal should fail");

    assert!(matches!(
        error,
        SkillPackageError::PathEscapesSkillRoot { .. }
    ));
}

#[test]
fn resolver_rejects_path_traversal() {
    let root = coffee_skill_root();
    let error =
        resolve_package_path(&root, "../coffee-skill-escape.js").expect_err("escape should fail");

    assert!(matches!(
        error,
        SkillPackageError::PathEscapesSkillRoot { .. }
    ));
}

#[test]
fn resolver_rejects_absolute_paths() {
    let root = coffee_skill_root();
    let absolute = root.join("index.js");
    let error = resolve_package_path(&root, absolute).expect_err("absolute path should fail");

    assert!(matches!(error, SkillPackageError::AbsolutePath { .. }));
}

#[test]
fn validate_inside_skill_root_rejects_external_canonical_path() {
    let root = coffee_skill_root();
    let external = repo_root().join("README.md");
    let error = validate_inside_skill_root(&root, external).expect_err("external path should fail");

    assert!(matches!(
        error,
        SkillPackageError::PathEscapesSkillRoot { .. }
    ));
}

#[test]
fn production_policy_quarantines_unsigned_local_skill() {
    let temp = TestSkillDir::minimal("unsigned-prod");
    let policy = PackageIntegrityPolicy::production(["did:wba:trusted.example"]);

    let error = load_skill_with_integrity_policy(temp.path(), &policy)
        .expect_err("production policy must reject unsigned packages");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { .. }
    ));
    assert!(error.to_string().contains("missing_supply_chain_metadata"));
}

#[test]
fn trusted_publisher_signature_verifies_for_development_contract() {
    let temp = TestSkillDir::minimal("signed-trusted");
    let publisher = "did:wba:trusted.example";
    let key_id = "did:wba:trusted.example#package-key-1";
    let digest = compute_package_digest(temp.path()).expect("digest unsigned package");
    let signature = development_signature_value(publisher, key_id, &digest);
    temp.write(
        "mcp.json",
        &signed_manifest_json(publisher, key_id, &digest, &signature),
    );
    let policy = PackageIntegrityPolicy::production([publisher]);

    let skill = load_skill_with_integrity_policy(temp.path(), &policy)
        .expect("trusted signed package should load");

    assert_eq!(skill.integrity.status, PackageIntegrityStatus::Verified);
    assert!(skill.integrity.production_ready);
    assert_eq!(skill.integrity.publisher_did.as_deref(), Some(publisher));
    assert_eq!(
        skill.integrity.signature_algorithm.as_deref(),
        Some(DEVELOPMENT_SIGNATURE_ALGORITHM)
    );
    assert_eq!(skill.integrity.signature_key_id.as_deref(), Some(key_id));
}

#[test]
fn digest_mismatch_is_quarantined() {
    let temp = TestSkillDir::minimal("digest-mismatch");
    let publisher = "did:wba:trusted.example";
    let key_id = "did:wba:trusted.example#package-key-1";
    let bad_digest = PackageDigest::sha256("0".repeat(64));
    let signature = development_signature_value(publisher, key_id, &bad_digest);
    temp.write(
        "mcp.json",
        &signed_manifest_json(publisher, key_id, &bad_digest, &signature),
    );
    let policy = PackageIntegrityPolicy::production([publisher]);

    let error = load_skill_with_integrity_policy(temp.path(), &policy)
        .expect_err("digest mismatch must quarantine");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "digest_mismatch"
    ));
}

#[test]
fn signature_mismatch_is_quarantined() {
    let temp = TestSkillDir::minimal("signature-mismatch");
    let publisher = "did:wba:trusted.example";
    let key_id = "did:wba:trusted.example#package-key-1";
    let digest = compute_package_digest(temp.path()).expect("digest unsigned package");
    temp.write(
        "mcp.json",
        &signed_manifest_json(publisher, key_id, &digest, "not-a-valid-signature"),
    );
    let policy = PackageIntegrityPolicy::production([publisher]);

    let error = load_skill_with_integrity_policy(temp.path(), &policy)
        .expect_err("signature mismatch must quarantine");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "signature_mismatch"
    ));
}

#[test]
fn unknown_publisher_is_quarantined() {
    let temp = TestSkillDir::minimal("unknown-publisher");
    let publisher = "did:wba:unknown.example";
    let key_id = "did:wba:unknown.example#package-key-1";
    let digest = compute_package_digest(temp.path()).expect("digest unsigned package");
    let signature = development_signature_value(publisher, key_id, &digest);
    temp.write(
        "mcp.json",
        &signed_manifest_json(publisher, key_id, &digest, &signature),
    );
    let policy = PackageIntegrityPolicy::production(["did:wba:trusted.example"]);

    let error = load_skill_with_integrity_policy(temp.path(), &policy)
        .expect_err("unknown publisher must quarantine");

    assert!(matches!(
        error,
        SkillPackageError::PackageQuarantined { reason } if reason == "unknown_publisher"
    ));
}

#[cfg(unix)]
#[test]
fn symlink_outside_package_fails_closed() {
    use std::os::unix::fs::symlink;

    let temp = TestSkillDir::minimal("symlink-outside");
    let outside = temp.path().with_extension("outside-secret");
    fs::write(&outside, "outside").expect("write outside file");
    fs::create_dir_all(temp.path().join("apis")).expect("create apis dir");
    symlink(&outside, temp.path().join("apis/escape.js")).expect("create outside symlink");

    let error = load_skill(temp.path()).expect_err("outside symlink must fail");

    assert!(matches!(
        error,
        SkillPackageError::PathEscapesSkillRoot { .. }
    ));
    let _ = fs::remove_file(outside);
}

#[test]
fn archive_entry_zip_slip_paths_fail_closed() {
    for entry in [
        "../escape.js",
        "components/../../escape.js",
        "/absolute/index.js",
        "https://example.invalid/skill.js",
        "components\\escape.js",
        "",
    ] {
        let error = validate_archive_entry_path(entry).expect_err("entry should fail");
        assert!(
            matches!(
                error,
                SkillPackageError::ZipSlipPath { .. } | SkillPackageError::AbsolutePath { .. }
            ),
            "{error:?}"
        );
    }

    assert_eq!(
        validate_archive_entry_path("components/card/index.js").expect("valid entry"),
        PathBuf::from("components/card/index.js")
    );
}

struct TestSkillDir {
    path: PathBuf,
}

impl TestSkillDir {
    fn new(name: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "anp-miniapp-dock-skill-loader-{name}-{}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale test dir");
        }
        fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }

    fn minimal(name: &str) -> Self {
        let temp = Self::new(name);
        temp.write("SKILL.md", "# Test Skill");
        temp.write(
            "index.js",
            "const skill = wx.modelContext.createSkill(__dirname)\nmodule.exports = skill\n",
        );
        temp.write("mcp.json", r#"{"apis":[],"components":[]}"#);
        temp
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative_path: &str, source: &str) {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, source).expect("write test file");
    }
}

impl Drop for TestSkillDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn signed_manifest_json(
    publisher: &str,
    key_id: &str,
    digest: &PackageDigest,
    signature: &str,
) -> String {
    format!(
        r#"{{
          "_meta": {{
            "anp": {{
              "supplyChain": {{
                "publisherDid": "{publisher}",
                "digest": {{
                  "algorithm": "{}",
                  "value": "{}"
                }},
                "signature": {{
                  "algorithm": "{}",
                  "keyId": "{key_id}",
                  "value": "{signature}"
                }}
              }}
            }}
          }},
          "apis": [],
          "components": []
        }}"#,
        digest.algorithm, digest.value, DEVELOPMENT_SIGNATURE_ALGORITHM
    )
}
