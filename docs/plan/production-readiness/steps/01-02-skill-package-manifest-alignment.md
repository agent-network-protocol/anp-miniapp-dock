# Step 01-02：Skill package 与 manifest 对齐

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 11:00:47 +0800 |
| Completed | 2026-06-12 11:22:56 +0800 |
| Commit | `ec46e1f` |
| Review evidence | 2026-06-12 11:19:23 +0800 commit 前 Review：发现并修复 CLI registration mismatch 报告测试缺口和根级 `format:file` warning path 稳定性问题；未发现阻塞问题 |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 11]`；`cargo fmt` 已运行；`cargo fmt --check` 通过；`cargo test -p mcp-schema` 13 passed；`cargo test -p skill-loader` 7 passed；`cargo test -p dock-cli validate` 2 passed；`cargo run -p dock-cli -- validate examples/coffee-skill` 输出 `status: ok`、`compatibilityLevel: demo-only` 和 `compatibilityReport.apis/components/permissions/risks/fallbacks/releaseBlockers`；`git diff --check -- crates/mcp-schema crates/skill-loader crates/dock-cli docs/architecture docs/runbook docs/plan` 无输出；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；post-commit `git status --short --branch` = `## main...origin/main [ahead 12]` |
| Next action | 进入 Step 01-03 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：扩展 Skill package / manifest 校验，使 `mcp.json`、`components[]`、`apis[]`、`_meta.ui.componentPath`、`relatedPage`、`permissions.scope.dynamic`、`expirable`、`expiredText` 等关键字段有稳定解析、校验和兼容报告。
- 用户 / 系统可见行为：`dock-cli validate` 能输出机器可读兼容性报告，区分 spec error、compatibility warning、production warning。
- 非目标：不实现 runtime 行为；不实现 package signing/cache；不支持完整小程序分包。
- 完成标准：manifest 校验覆盖 Phase 1.1 计划项；warning 有修复建议或明确降级行为；API/组件矩阵与实现状态同步。

## 3. 设计方法

- 设计边界：先让 loader/schema/CLI 知道字段和风险，不把字段读取直接等同为 runtime 已支持。
- 核心决策：校验结果分层为 spec error、compatibility warning、production warning；demo 可用但生产不允许的能力不能静默通过。
- 契约 / API / 数据流：`skill-loader` 读取包，`mcp-schema` 表达 manifest，`dock-cli validate` 输出兼容报告，runtime 后续按报告和 manifest metadata 执行。
- 兼容性：保留未知字段和 `_meta`；不破坏现有 coffee Skill；`app.json` / `AGENTS.md` 只做可选读取规划。
- 风险控制：路径必须 canonicalize，阻断 path traversal、跨包 component/API path 和跨包 require。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/crates/mcp-schema`、`anp/anp-miniapp-dock/crates/skill-loader`、`anp/anp-miniapp-dock/crates/dock-cli` 中现有 manifest / validate 实现和测试。
2. 扩展 schema：`components[].relatedPage`、`components[].permissions.scope.dynamic`、`components[].expirable`、`components[].expiredText`、`format: "image" | "file"` 输入字段识别等。
3. 扩展 validator：`SKILL.md` 单文件和长度限制、`mcp.json` 长度统计、`apis[].name` 与注册名一致、`inputSchema` 必须为对象、`outputSchema` mismatch warning、component path 关系检查。
4. 扩展 `dock-cli validate` JSON 输出：status、compatibilityLevel、apis、components、permissions、risks、fallbacks、releaseBlockers。
5. 新增或更新 focused tests，覆盖 success、spec error、compatibility warning、production warning、路径错误。
6. 同步更新 Step 00-02 / 00-03 输出的矩阵状态和相关 runbook/README 链接。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/mcp-schema` | manifest 字段和校验模型 | 代码实现 |
| `anp/anp-miniapp-dock/crates/skill-loader` | package loading / path validation | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-cli` | `validate` 输出兼容报告 | 代码实现 |
| `anp/anp-miniapp-dock/examples/coffee-skill` | 如需 fixture 更新 | 保持 mock-only |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 API 状态 | 必须按实际影响更新 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步组件状态 | 必须按实际影响更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 如 validate 成为 gate，补说明 | 视实现影响 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-02-skill-package-manifest-alignment.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-02、Step 00-03、Step 01-01。
- 外部文档或决策：Phase 1.1 计划、API/组件矩阵、bridge contract。
- 环境前提：Rust toolchain 1.88.0；无需外部服务。

## 7. 验收标准

- [x] `dock-cli validate` 输出机器可读兼容报告，包含 status、apis、components、permissions、risks、fallbacks、releaseBlockers。
- [x] manifest warning 都有修复建议或明确降级行为。
- [x] `apis[].name`、`inputSchema`、`outputSchema` mismatch、`_meta.ui.componentPath`、component path、dynamic/expirable metadata 有测试。
- [x] 路径穿越、跨包 path、非法 componentPath fail closed。
- [x] `wx-api-compatibility-matrix.md` 和 `component-compatibility-matrix.md` 与实现状态同步。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Unit / crate tests | `cd anp/anp-miniapp-dock && cargo test -p mcp-schema -p skill-loader -p dock-cli validate` | 相关测试通过；若 test target 名称不同，记录实际命令 |
| CLI validate | `cd anp/anp-miniapp-dock && cargo run -p dock-cli -- validate examples/coffee-skill` | 输出 JSON 且不含敏感信息 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/mcp-schema crates/skill-loader crates/dock-cli docs/architecture docs/runbook docs/plan` | 无空白错误 |
| 安全回归抽样 | 手工或测试覆盖 path traversal、非法 componentPath、demo-only production warning | fail closed 或 warning 符合设计 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：manifest 字段是否过度接受；路径校验是否 fail closed；CLI JSON 是否稳定且可 CI 消费；warning 分层是否清晰；矩阵是否同步；是否泄露 private path/token。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 2 项非阻塞问题 | `dock-cli validate` 的 API registration mismatch 需要直接体现在兼容报告测试中；根级 `inputSchema.format` warning path 不应产生空后缀。 |
| 已修复问题 | 已修复 | 新增 `validate_reports_api_registration_mismatch_as_release_blocker`；修正根级 `format:file/image` warning path，并新增 `reports_root_media_format_without_empty_path_suffix`。 |
| 剩余风险 | 已记录 | `dock-cli validate` 报告结构是 Phase 5 开发者体验会复用的候选契约，release gate 已先纳入人工检查；`relatedPage`、dynamic、`expirable` 的 runtime 行为仍是 host-boundary / 后续 Phase，不在本 Step 宣称 production-ready。 |
| 新增或缺失测试 | 已新增 focused tests | `mcp-schema` 覆盖 component metadata、media/file format、unsafe relatedPage；`skill-loader` 覆盖 component path traversal fail closed；`dock-cli` 覆盖 compatibility report、demo-only blocker、registration mismatch blocker。 |
| 已更新或缺失文档 | 已同步 | 已更新 API 矩阵、组件矩阵、release gates、当前能力基线、本 Step 和主 roadmap；无缺失文档项。 |

Review 补充检查：

- 敏感字段抽样：`dock-cli validate examples/coffee-skill` 输出未包含 capability token、Authorization、HTTP Signature、private key path/material；`skillRoot` 保留为旧 CLI 字段，不携带凭据。
- 行为边界：manifest 字段解析和兼容报告不等于 runtime 已支持；矩阵中保持 `host-boundary` / `planned-p1` 标注。
- 回归风险：coffee E2E、workspace clippy、相关 crate tests 均通过。

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 manifest/validate 对齐相关代码、测试和直接文档更新。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 11]`，包含 `crates/mcp-schema`、`crates/skill-loader`、`crates/dock-cli`、`docs/architecture`、`docs/runbook`、`docs/plan` 的 Step 01-02 相关改动。
- Commit 后证据：`ec46e1f`；`git status --short --branch` = `## main...origin/main [ahead 12]`。
- 建议消息：`phase1: align skill manifest validation`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-02 小 Plan | 将 Phase 1.1 manifest 对齐拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：validate 输出 schema 一旦被 CI 或开发者使用，会成为公开契约。
- 回滚 / 回退：若输出 schema 不稳定，先在文档标为 experimental，并在后续 Step 冻结。
- 后续文档：Phase 5 CLI/开发者体验会复用此报告结构。
