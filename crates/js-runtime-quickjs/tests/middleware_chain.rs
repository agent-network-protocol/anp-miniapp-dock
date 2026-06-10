use js_runtime_quickjs::{ApiCall, ApiVm, ApiVmConfig, ApiVmError};
use mcp_schema::{ApiDeclaration, SkillManifest, ValidationReport};
use serde_json::json;
use skill_loader::{LoadedSkill, SourceFile};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn middleware_runs_in_onion_order() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.use(async (ctx, next) => {
  ctx.arguments.events.push('outer-before')
  await next()
  ctx.arguments.events.push('outer-after')
})
skill.use(async (ctx, next) => {
  ctx.arguments.events.push('inner-before')
  await next()
  ctx.arguments.events.push('inner-after')
})
skill.registerAPI('ordered', async (ctx) => {
  ctx.arguments.events.push('handler')
  return {
    content: [{ type: 'text', text: ctx.arguments.events.join(',') }],
    structuredContent: { events: ctx.arguments.events }
  }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["ordered"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "ordered",
            json!({ "events": [] }),
        ))
        .expect("call ordered");

    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("events"))
            .and_then(|events| events.as_array())
            .cloned(),
        Some(vec![
            json!("outer-before"),
            json!("inner-before"),
            json!("handler"),
            json!("inner-after"),
            json!("outer-after"),
        ])
    );
}

#[test]
fn async_handler_promise_is_resolved() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('asyncValue', async (ctx) => {
  const suffix = await Promise.resolve(ctx.arguments.suffix)
  return { content: [{ type: 'text', text: 'async-' + suffix }] }
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["asyncValue"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new(
            "skill",
            "session",
            "asyncValue",
            json!({ "suffix": "ok" }),
        ))
        .expect("call asyncValue");

    assert_eq!(result.content[0].text, "async-ok");
}

#[test]
fn timeout_interrupts_long_running_handler() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('loop', () => {
  while (true) {}
})
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["loop"],
    );
    let vm = ApiVm::load_skill_with_config(
        skill,
        ApiVmConfig {
            timeout: Duration::from_millis(20),
            ..Default::default()
        },
    )
    .expect("load VM");

    let error = vm
        .call(ApiCall::new("skill", "session", "loop", json!({})))
        .expect_err("loop should time out");

    assert!(matches!(error, ApiVmError::Timeout(name, _) if name == "loop"));
}

#[test]
fn require_parent_escape_is_rejected() {
    let mut modules = BTreeMap::new();
    modules.insert(
        "safe".to_owned(),
        r#"
module.exports = () => ({ content: [{ type: 'text', text: 'never' }] })
"#
        .to_owned(),
    );
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('escape', require('../secret'))
module.exports = skill
"#,
        modules,
        vec!["escape"],
    );

    let error = ApiVm::load_skill(skill).expect_err("escape require must fail");
    assert!(
        matches!(error, ApiVmError::QuickJs(message) if message.contains("outside skill package"))
    );
}

#[test]
fn sandbox_globals_are_not_available_to_skill_code() {
    let skill = test_skill(
        r#"
const skill = wx.modelContext.createSkill(__dirname)
skill.registerAPI('globals', () => ({
  content: [{ type: 'text', text: [
    typeof process,
    typeof fetch,
    typeof eval,
    typeof Function,
    typeof (() => {}).constructor,
    typeof (async function() {}).constructor,
    typeof (function* () {}).constructor,
    typeof (async function* () {}).constructor
  ].join(',') }]
}))
module.exports = skill
"#,
        BTreeMap::new(),
        vec!["globals"],
    );
    let vm = ApiVm::load_skill(skill).expect("load VM");

    let result = vm
        .call(ApiCall::new("skill", "session", "globals", json!({})))
        .expect("call globals");

    assert_eq!(
        result.content[0].text,
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
}

fn test_skill(
    entry_js: &str,
    api_modules: BTreeMap<String, String>,
    api_names: Vec<&str>,
) -> LoadedSkill {
    LoadedSkill {
        root: PathBuf::from("/tmp/test-skill"),
        skill_md: source("SKILL.md", "Test skill"),
        manifest: SkillManifest {
            apis: api_names
                .into_iter()
                .map(|name| ApiDeclaration {
                    name: name.to_owned(),
                    description: format!("{name} test API"),
                    input_schema: json!({ "type": "object" }),
                    output_schema: None,
                    meta: None,
                    extra: BTreeMap::new(),
                })
                .collect(),
            components: Vec::new(),
            extra: BTreeMap::new(),
        },
        entry_js: source("index.js", entry_js),
        api_modules: api_modules
            .into_iter()
            .map(|(name, body)| (name.clone(), source(format!("apis/{name}.js"), body)))
            .collect(),
        components: BTreeMap::new(),
        component_routes: BTreeMap::new(),
        validation: ValidationReport::ok(),
    }
}

fn source(path: impl AsRef<Path>, source: impl Into<String>) -> SourceFile {
    let relative_path = path.as_ref().to_path_buf();
    SourceFile {
        absolute_path: Path::new("/tmp/test-skill").join(&relative_path),
        relative_path,
        source: source.into(),
    }
}
