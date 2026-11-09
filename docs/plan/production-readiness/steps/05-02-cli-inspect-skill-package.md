# Step 05-02：CLI inspect Skill package

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：05-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 23:54:31 +0800 |
| Completed | 2026-06-14 00:13:03 +0800 |
| Commit | `ed5599f` |
| Review evidence | 2026-06-14 00:10:41 +0800 commit 前 Review 已记录：确认 `inspect` 只调用 `load_skill`、API registration trace / 静态注册扫描、文件树和 `wx.*` 静态扫描，不调用 Skill API 或 Host provider；修复 `validation_summary` / inspect warning 可能透传敏感 issue 文本的问题，统一输出 `[REDACTED]` 且不回显 `Authorization` / `Signature` / token marker；确认 file tree 只输出相对路径、类型和大小，不输出源码；确认 dynamic property access 与静态扫描限制已在输出和 Phase 5 文档中标注。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 88]`；已读取主 Plan、Step 05-02 文档、Phase 5 文档、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理和 Plan 变更记录；已确认 05-01 implementation commit `153027c` 与 closure commit `d8ae27f`；`cargo fmt --check` 通过；`cargo test -p dock-cli inspect` 2 unit + 1 integration passed；`cargo test -p skill-loader` 14 package/path tests + 11 registry/cache tests + doctests passed；`cargo run -p dock-cli -- inspect examples/coffee-skill` 输出 `dock.inspect-report.v1`、`status = warning`、`commandStatus = ok`、`registeredApisSource = api-vm-registration-trace`；`python3 -m json.tool /tmp/dock-inspect-0502.json` 可解析；inspect JSON 脱敏抽样未命中 `/home/`、Authorization、Signature、capabilityToken、private、secret 或 token；`git diff --check -- crates/dock-cli crates/skill-loader crates/mcp-schema docs/plan README.md` 无输出；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过。 |
| Next action | 进入 05-03 CLI test-skill 与 Fixture Runner |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增或增强 `dock-cli inspect`，展示 Skill package 文件、API 注册与 manifest 对照、组件路径、权限需求、风险等级和 `wx.*` 使用痕迹。
- 用户 / 系统可见行为：开发者能定位 manifest/注册不一致、组件路径、风险 API 和 potential fallback，而不读 Rust 源码。
- 非目标：不执行 Skill API；不做完整 JS 静态分析器。
- 完成标准：inspect 输出 machine-readable JSON 和必要 human summary，敏感路径/参数 redacted。

## 3. 设计方法

- 设计边界：inspect 是静态和轻量 runtime metadata 检查，不调用高风险 provider。
- 核心决策：优先基于 manifest、loader、registration trace 和简单 `wx.*` scan；无法确定时输出 unknown-with-reason，而不是猜测 supported。
- 契约 / API / 数据流：Skill package -> loader/schema -> inspect graph -> JSON/human output。
- 兼容性：复用 validate report 的状态枚举和 redaction helper。
- 风险控制：不输出 private key path、raw local absolute secret path、token、Authorization 或真实隐私 payload。

## 4. 实现方法

1. 阅读 `dock-cli validate` report 和 skill-loader manifest/registration evidence。
2. 定义 inspect output：files、apis、registeredApis、components、permissions、riskLevels、wxApiUsage、warnings。
3. 实现 package file tree 安全展示，路径相对 Skill root，拒绝包外路径。
4. 增加 `wx.*` usage scan 或 runtime trace summary，并标注静态扫描限制。
5. 增加 tests：coffee inspect、registration mismatch、component path、permission/risk summary、redaction。
6. 更新 Phase 5 文档和 README/CLI docs。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-cli` | `inspect` command、output schema、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/skill-loader` | package file tree / registration metadata | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/mcp-schema` | inspect DTO/status enum 复用 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-5-developer-experience.md` | 同步 inspect contract | 必须 |
| `anp/anp-miniapp-dock/README.md` | 视 CLI 文档更新 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/05-02-cli-inspect-skill-package.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 05-01。
- 外部文档或决策：validate report schema、Skill package integrity contract。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host provider。

## 7. 验收标准

- [x] `dock-cli inspect` 输出 Skill package 文件、API/registration 对照、componentPath、permissions、risk、wxApiUsage。
- [x] 输出支持 JSON，可由 CI/文档示例复用。
- [x] 路径以 Skill root 相对路径展示，包外路径 fail closed 或 redacted。
- [x] 静态扫描无法确定的能力标注 unknown-with-reason，不误标 supported。
- [x] 输出 redacted，不含 token、Authorization、signature、private key path 或隐私原文。
- [x] Phase 5 文档和 README/CLI docs 与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| CLI inspect tests | `cd anp/anp-miniapp-dock && cargo test -p dock-cli inspect` | inspect tests 通过 |
| Loader tests | `cd anp/anp-miniapp-dock && cargo test -p skill-loader` | package path/loader tests 不回归 |
| Manual inspect | `cd anp/anp-miniapp-dock && cargo run -p dock-cli -- inspect examples/coffee-skill` | 输出 JSON/human summary，redacted |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-cli crates/skill-loader crates/mcp-schema docs/plan README.md` | 无空白错误 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：inspect 是否不执行高风险代码；路径是否安全；unknown 是否不被误标 supported；输出是否可读且 redacted。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 有，已修复 | 初版 `validation_summary` / inspect warning 直接序列化 loader validation issue，极端情况下可能把敏感 path/message/suggestion 带入 JSON；初版 redaction placeholder 仍回显 `Signature` 等 marker，导致敏感词扫描命中。 |
| 已修复问题 | 已修复 | 新增 `validation_issue_json`，对 path/message/suggestion 统一调用 `redact_text`；`redact_text` 改为只输出 `[REDACTED]`；补充 `validation_summary_redacts_sensitive_issue_text` 回归测试。 |
| 剩余风险 | 已记录 | `registeredApisSource = static-register-api-scan` 与 `wxApiUsage` 仍是轻量静态扫描，dynamic property access 必须通过后续 `test-skill` / fixture gate 验证。 |
| 新增或缺失测试 | 已新增 | 新增 CLI inspect arg/unit 测试、inspect package graph/redaction 测试、coffee inspect integration 测试；未新增完整 JS 静态分析器测试，符合本 Step 非目标。 |
| 已更新或缺失文档 | 已更新 | 更新 Phase 5 `inspect` schema/限制说明和 README CLI 示例；主 Plan 和本 Step 台账同步。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 inspect command、直接 tests 和相关文档。
- Commit 前状态：`git status --short` = `M README.md`、`M crates/dock-cli/src/commands.rs`、`M crates/dock-cli/tests/coffee_order_flow.rs`、`M docs/plan/production-readiness-roadmap.md`、`M docs/plan/production-readiness/phase-5-developer-experience.md`、`M docs/plan/production-readiness/steps/05-02-cli-inspect-skill-package.md`。
- 纳入文件：`README.md`、`crates/dock-cli/src/commands.rs`、`crates/dock-cli/tests/coffee_order_flow.rs`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-5-developer-experience.md`、`docs/plan/production-readiness/steps/05-02-cli-inspect-skill-package.md`。
- Commit 后证据：implementation commit `ed5599f phase5: add skill inspect command`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 89]`。
- 遗留未提交变更：仅本 closure 文档更新，随后单独提交。
- 建议消息：`phase5: add skill inspect command`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 无 | 无 | 无 | 当前步骤 / 整体计划 | 无 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 05-02 小 Plan | 将 CLI inspect Skill package 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：过度静态分析容易给出错误确定性。
- 回滚 / 回退：不确定时输出 unknown-with-reason，并建议运行 test-skill。
- 后续文档：迁移指南应展示 inspect + validate 的组合用法。
