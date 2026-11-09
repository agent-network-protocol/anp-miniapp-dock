# Step 02-01：Render IR schemaVersion 与 fallback reason enum

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：02-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 21:11:25 +0800 |
| Completed | 2026-06-12 21:20:27 +0800 |
| Commit | `0cfea24` |
| Review evidence | 本文 Review 环节已记录：未发现阻塞问题；确认所有 Component Runtime Render IR 输出带 `schemaVersion`，fallback reason 对外为稳定枚举值，旧自由字符串只在内部 normalize，未向 CLI/Host payload 泄露路径或错误细节。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p component-runtime render` 5 passed；`cargo test -p component-runtime render_output_serializes_schema_version` 1 passed；`cargo test -p card-spec fallback` 1 passed；`cargo test -p dock-core fallback` 1 passed；`cargo test -p dock-cli preview_card` 2 passed；`cargo test -p card-spec -p dock-core -p component-runtime` 40 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime crates/dock-core crates/card-spec crates/dock-cli docs/architecture docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假路径和文档安全说明。 |
| Next action | 进入 Step 02-02 Component manifest metadata runtime flow |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为 Render IR 输出增加稳定 `schemaVersion`，并把 fallback reason 从自由字符串收敛为枚举。
- 用户 / 系统可见行为：Host renderer、CLI 和 tests 可以按 `dock.render-ir.v1` 与稳定 reason 判断兼容性和 fallback 原因。
- 非目标：不实现完整 Host renderer；不新增表单、地图或动态组件节点。
- 完成标准：Render IR / fallback contract 有代码枚举、序列化测试、文档矩阵和 CLI/audit 可观测证据。

## 3. 设计方法

- 设计边界：Render IR 是 Component Runtime 与 Host 的稳定边界；版本化必须先于 P1 节点和动态能力扩展。
- 核心决策：schema version 初始值使用 `dock.render-ir.v1`；fallback reason 使用 snake_case 枚举并对未知输入 fail closed 或映射为 safe fallback。
- 契约 / API / 数据流：Component Runtime render -> `RenderOutcome` / Render IR JSON -> Host / CLI；失败路径 -> `FallbackReason` enum -> CardSpec / structuredContent / content fallback。
- 兼容性：现有 coffee snapshots / tests 需要稳定新增字段，不破坏既有 RenderNode tree。
- 风险控制：fallback reason 和 debug 信息不得泄露本地绝对路径、token、Authorization、private key path 或原始隐私数据。

## 4. 实现方法

1. 阅读 `docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md` 的 Render IR contract 和 fallback reason 列表。
2. 阅读 `docs/architecture/component-compatibility-matrix.md` 中 Render IR、fallback 与 fixture 状态。
3. 在 `component-runtime` 定义 `schemaVersion` 输出字段和 node/action registry 文档对应的常量。
4. 在 `dock-core` / `card-spec` / `dock-cli` 中把 fallback reason 字符串收敛为 enum 或可序列化的稳定类型。
5. 增加 tests：正常 render 包含 schemaVersion；每个 fallback reason 可序列化；unknown / parse failure / missing component 等路径映射正确；debug redacted。
6. 更新组件兼容矩阵、Phase 2 子文档、release gates 或 runbook。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/component-runtime` | Render IR `schemaVersion`、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | fallback reason enum / routing | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/card-spec` | CardSpec fallback reason 映射 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | CLI preview / fallback 输出 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步 Render IR contract 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md` | 同步 contract 细节 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步 Render IR gate | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/02-01-render-ir-schema-fallback-reasons.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-03、Step 01-03。
- 外部文档或决策：Phase 2 Component Runtime Alignment、Render IR 与 Fixture 体系、组件兼容矩阵、Threat Model。
- 环境前提：Rust toolchain 1.88.0；无需真实 Host renderer。

## 7. 验收标准

- [x] 所有正常 Render IR 输出包含 `schemaVersion: "dock.render-ir.v1"`。
- [x] fallback reason 有稳定枚举，至少覆盖 `no_component_path`、`component_missing`、`component_load_failed`、`component_vm_failed`、`wxml_parse_failed`、`wxss_parse_warning_threshold`、`unsupported_node_kind`、`host_renderer_unavailable`、`api_error`、`empty_structured_content`。
- [x] CLI / tests 可以观察 fallback reason，且不会泄露本地绝对路径或敏感数据。
- [x] Host unknown node/action 的策略保持 fail closed 或 fallback，不静默执行。
- [x] 组件兼容矩阵和 Phase 2 子文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Component tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime render` | schemaVersion 和 render/fallback tests 通过；若 filter 不匹配，记录实际命令 |
| Core / CLI tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core fallback && cargo test -p dock-cli --test coffee_order_flow` | fallback routing 和 coffee 回归通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/component-runtime crates/dock-core crates/card-spec crates/dock-cli docs/architecture docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 fallback reason、debug、CLI JSON | 不含 token、Authorization、signature、private key path、本地绝对路径或隐私原文 |

补充验证：`cargo test -p component-runtime render_output_serializes_schema_version` 1 passed；`cargo test -p card-spec -p dock-core -p component-runtime` 40 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过。

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：schemaVersion 是否出现在所有 Render IR path；fallback reason 是否稳定且覆盖失败路径；新增字段是否破坏 Host contract；debug 是否 redacted。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 未发现阻塞问题 | Review 覆盖 Render IR schemaVersion 全路径、fallback reason 稳定枚举、CLI/Host payload、敏感错误信息脱敏和文档同步。 |
| 已修复问题 | 将 `ComponentRenderOutput` 变为可序列化 camelCase contract 并默认输出 `dock.render-ir.v1`；将旧 `component_render_failed` / `renderer_unavailable` 等自由字符串收敛到 Step 要求的稳定枚举；dock-core render failure 对外输出 `component_vm_failed`。 | focused tests、相关包测试和 coffee E2E 已通过。 |
| 剩余风险 | Render IR golden snapshots、集中 fixture 目录、Host renderer unknown node/action conformance 仍在 Step 02-06 / Phase 4；本 Step 只稳定版本与 fallback reason contract。 | 组件矩阵和 release gates 已保留 planned 项。 |
| 新增或缺失测试 | 新增 Render IR schemaVersion 直接序列化测试、fixture schemaVersion 断言、CLI schemaVersion 断言、fallback reason enum 覆盖测试和 render failure stable reason 断言；未新增 golden snapshot。 | golden snapshots 属于 Step 02-06。 |
| 已更新或缺失文档 | 已更新组件兼容矩阵、Phase 2 Render IR 子文档、release gates、本 Step 和主 Plan 台账。 | 未更新 Host adapter contract，留到 Phase 4。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 Render IR schemaVersion、fallback reason enum、直接 tests 和相关文档。
- Commit 前状态：`git status --short` 包含本 Step Render IR schemaVersion、fallback reason enum、直接 tests 和相关文档，未发现其它 Step 完成工作。
- 纳入文件：`crates/component-runtime/src/compiler.rs`、`crates/component-runtime/src/render_ir.rs`、`crates/component-runtime/src/lib.rs`、`crates/component-runtime/tests/wxml_bindings.rs`、`crates/card-spec/src/fallback.rs`、`crates/card-spec/tests/order_card.rs`、`crates/dock-core/src/orchestrator.rs`、`crates/dock-core/tests/api_call_flow.rs`、`crates/dock-cli/src/commands.rs`、`crates/dock-cli/tests/coffee_order_flow.rs`、`docs/architecture/component-compatibility-matrix.md`、`docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/steps/02-01-render-ir-schema-fallback-reasons.md`、`docs/plan/production-readiness-roadmap.md`。
- Commit 后证据：实现提交 `0cfea24 phase2: version render ir fallback reasons`；提交后 `git status --short --branch` = `## main...origin/main [ahead 31]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase2: version render ir fallback reasons`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 02-01 小 Plan | 将 Phase 2 Render IR contract 稳定化拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：Render IR 字段变化会影响 Host adapter 和 CLI snapshot。
- 回滚 / 回退：保持 `dock.render-ir.v1` 向后兼容；breaking change 必须新版本和 migration note，不在本 Step 引入。
- 后续文档：Step 02-06 golden snapshots 必须基于本 schemaVersion 和 fallback reason enum。
