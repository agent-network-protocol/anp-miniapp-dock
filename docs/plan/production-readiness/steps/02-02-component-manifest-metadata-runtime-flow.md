# Step 02-02：Component manifest metadata runtime flow

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：02-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 21:22:16 +0800 |
| Completed | 2026-06-12 21:37:07 +0800 |
| Commit | `79417d5` |
| Review evidence | 本文 Review 环节已记录：未发现阻塞问题；确认 metadata 来自 manifest 而非 JS state，`componentPath` 支持 `/index` alias，unsafe `relatedPage.path` 不进入 runtime metadata，query / scopeDynamic 脱敏，dynamic request/timer 仍默认关闭。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p mcp-schema -p skill-loader component` 5 passed；`cargo test -p component-runtime metadata` 1 passed；`cargo test -p dock-cli metadata` 2 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo test -p component-runtime -p dock-cli -p mcp-schema -p skill-loader` 73 passed；`cargo run -p dock-cli -- validate examples/coffee-skill` 输出 `demo-only` 且 components 含 `runtimeMetadata`；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/mcp-schema crates/skill-loader crates/component-runtime crates/wx-compat crates/dock-cli docs/architecture docs/plan docs/runbook` 无输出；敏感词抽样仅命中 redaction 规则、测试假值和文档红线。 |
| Next action | 进入 Step 02-03 WXML/WXSS P1 语法增强 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：让 `components[].relatedPage`、`permissions.scope.dynamic`、`expirable`、`expiredText` 从 manifest 校验结果真正进入 runtime / RenderOutcome。
- 用户 / 系统可见行为：Component Runtime 和 CLI/Host 能基于 manifest metadata 判断 dynamic 权限、过期显示、关联页面和 production warnings。
- 非目标：不实现真实 Host 详情页、动态 request/timer 或生产 card manager；这些由后续 Step 和 Phase 4 接入。
- 完成标准：manifest metadata 经过 loader、schema、runtime context、Render IR/action 或 RenderOutcome 可观测，并有路径 canonicalization 和安全测试。

## 3. 设计方法

- 设计边界：manifest metadata 是 runtime 决策输入，不是 Skill JS 可随意篡改的状态。
- 核心决策：`components[].path` 与 `_meta.ui.componentPath` 使用同一 canonicalize 规则；未知 `_meta` 保留但不进入模型可见结果；dynamic 只表达权限，不自动开放网络/timer。
- 契约 / API / 数据流：`mcp.json` -> `mcp-schema` -> `skill-loader` -> component metadata -> `ComponentInput` / runtime profile -> RenderOutcome actions/warnings/metadata。
- 兼容性：继续支持 coffee 现有组件；未声明 metadata 时保持 P0 默认行为。
- 风险控制：relatedPage path/query 必须 canonicalize 和脱敏；expiredText 不得包含敏感字段；dynamic 未声明时默认 deny。

## 4. 实现方法

1. 阅读 Step 01-02 的 manifest 对齐结果和 `mcp-schema` / `skill-loader` 当前字段。
2. 阅读 `docs/architecture/component-compatibility-matrix.md` 中 relatedPage、expirable、dynamic 的状态。
3. 在 schema/loader/runtime 之间补齐 metadata 传递结构，避免 runtime 重新解析 raw manifest。
4. 让 Component Runtime 在 RenderOutcome、actions、warnings 或 metadata 中可观测 `relatedPage`、`dynamic`、`expirable`、`expiredText`。
5. 增加 tests：componentPath canonicalize、relatedPage path/query safe、dynamic 默认 deny、expirable filter、expiredText render/fallback、unknown `_meta` 保留但不泄露。
6. 更新组件兼容矩阵、Phase 2 文档和 release gates。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/mcp-schema` | component manifest metadata 结构和 tests | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/skill-loader` | component path canonicalize、metadata 传递 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/component-runtime` | ComponentInput / RenderOutcome metadata flow | 代码实现 |
| `anp/anp-miniapp-dock/crates/wx-compat` | component capability profile / dynamic permission | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | validate / preview 输出 metadata 证据 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步 metadata runtime flow 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-component-runtime-alignment.md` | 同步组件 manifest 元数据完成状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/02-02-component-manifest-metadata-runtime-flow.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-02、Step 01-03、Step 02-01。
- 外部文档或决策：Component Runtime Alignment、组件兼容矩阵、Render IR contract。
- 环境前提：Rust toolchain 1.88.0；无需真实 Host renderer。

## 7. 验收标准

- [x] `relatedPage` 从 manifest 进入 runtime 可观测 metadata/action，并经过 path/query canonicalize。
- [x] `permissions.scope.dynamic` 进入 component capability profile；未声明 dynamic 的组件默认 deny request/timer。
- [x] `expirable` 和 `expiredText` 进入 runtime 过期策略或 RenderOutcome metadata，未声明时不被误判为 production expirable。
- [x] `_meta.ui.componentPath` 与 `components[].path` 的 canonical path 规则一致。
- [x] 未知 `_meta` 保留在 Host/private 边界，不进入模型可见输出。
- [x] 组件兼容矩阵和 Phase 2 文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Schema / loader tests | `cd anp/anp-miniapp-dock && cargo test -p mcp-schema -p skill-loader component` | manifest metadata 和 path tests 通过 |
| Runtime tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime metadata` | runtime metadata tests 通过；若 filter 不匹配，记录实际命令 |
| CLI validate | `cd anp/anp-miniapp-dock && cargo run -p dock-cli -- validate examples/coffee-skill` | 输出仍为 `demo-only`，metadata/report 不回归 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/mcp-schema crates/skill-loader crates/component-runtime crates/wx-compat crates/dock-cli docs/architecture docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 RenderOutcome / CLI JSON / warnings | 不含 token、Authorization、signature、private key path 或隐私原文 |

补充验证：`cargo test -p dock-cli metadata` 2 passed；`cargo test -p component-runtime -p dock-cli -p mcp-schema -p skill-loader` 73 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过。

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：metadata 是否来自 manifest 而非 JS 注入；dynamic 是否默认 deny；relatedPage 是否可逃逸；过期策略是否误伤未声明组件；CLI output 是否泄露 `_meta`。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 修复 validate report 泄露原始 `relatedPage.query.secretToken` 的问题；修复 metadata 初版从 JS snapshot 回传的错误设计。 | 最终实现改为 Rust runtime 保存 manifest-derived metadata，validate / render envelope 都使用 redacted metadata。 |
| 已修复问题 | `ComponentMetadata` 由 `ComponentInput` 进入 `ComponentOperationOutcome`，不进入 JS seed state；`dock-cli` 从 manifest 构造 redacted metadata；`componentPath` 支持 `/index` alias；CLI validate 输出不包含测试假 secret。 | focused tests 已覆盖。 |
| 剩余风险 | 真实 Host detail page、production card manager、dynamic request/timer 开放、Host background lifecycle 和 persistent audit 仍在 Step 02-05 / Phase 4；本 Step 只打通 metadata flow。 | 文档已记录边界。 |
| 新增或缺失测试 | 新增 component-runtime metadata 不进 JS state 测试、dock-cli metadata validate/redaction 测试、componentPath alias 测试、coffee E2E metadata 断言；未新增真实 Host renderer E2E。 | Host renderer 不属于 02-02 范围。 |
| 已更新或缺失文档 | 已更新组件兼容矩阵、Phase 2 component runtime alignment、release gates、本 Step 和主 Plan 台账。 | 未更新 Host adapter contract，留到 Phase 4。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 component manifest metadata flow、直接 tests 和相关文档。
- Commit 前状态：`git status --short` 包含本 Step component manifest metadata flow、直接 tests 和相关文档，未发现其它 Step 完成工作。
- 纳入文件：`crates/component-runtime/src/component_vm.rs`、`crates/component-runtime/src/lib.rs`、`crates/component-runtime/tests/component_lifecycle.rs`、`crates/dock-cli/src/commands.rs`、`crates/dock-cli/tests/coffee_order_flow.rs`、`docs/architecture/component-compatibility-matrix.md`、`docs/plan/production-readiness/phase-2-component-runtime-alignment.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/steps/02-02-component-manifest-metadata-runtime-flow.md`、`docs/plan/production-readiness-roadmap.md`。
- Commit 后证据：实现提交 `79417d5 phase2: flow component manifest metadata`；提交后 `git status --short --branch` = `## main...origin/main [ahead 33]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase2: flow component manifest metadata`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 02-02 小 Plan | 将 Phase 2 component manifest metadata runtime flow 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：metadata 流向如果混入模型可见输出，会泄露 Host/private 信息或制造 prompt surface。
- 回滚 / 回退：保留 metadata 在 private/runtime 边界；Host 展示前必须 redaction。
- 后续文档：Step 02-05 dynamic controls 和 Phase 4 Host adapter contract 依赖本 metadata flow。
