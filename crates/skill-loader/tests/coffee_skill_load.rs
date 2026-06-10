use skill_loader::{
    load_skill, resolve_package_path, validate_inside_skill_root, SkillPackageError,
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
