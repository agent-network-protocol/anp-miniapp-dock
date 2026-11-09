# Step 02-06：Fixture 与 Render IR snapshots

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：02-06
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 22:33:56 +0800 |
| Completed | 2026-06-12 22:59:08 +0800 |
| Commit | `f778a14` |
| Review evidence | 2026-06-12 22:56:36 +0800 commit 前 Review 已记录：修复 dynamic snapshot `brokerCalls` 取值时机和 dynamic policy 过期文案；确认 snapshots 稳定、mock-only、无禁用敏感串 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p component-runtime snapshot` 通过；`cargo test -p dock-cli fixture` 通过；`cargo test -p mcp-schema` 13 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；严格 fixture/snapshot 敏感串扫描无命中 |
| Next action | 进入 Step 02-07 批次最终 Review 与整体验证 gate，不得直接进入 Phase 3 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增 address-form、media-review、dynamic-status、location-map-preview fixture，并建立 Render IR golden snapshots。
- 用户 / 系统可见行为：Phase 2 P1 组件能力可以通过稳定 fixture 和 snapshot 被 CLI、tests、Host adapter 复用验证。
- 非目标：不实现真实 Host provider、真实支付、真实文件读取或真实地图交互；fixtures 使用 mock/dev-only provider 时必须明确标识。
- 完成标准：每个 fixture 有 Skill package、API input cases、expected Render IR snapshot、actions、warnings、metadata、state、audit summary 或明确的 planned gap 记录。

## 3. 设计方法

- 设计边界：fixture 是兼容性证据和回归测试，不是 production data；所有数据必须 mock-only 且不含真实 DID、token、手机号、地址或文件内容。
- 核心决策：snapshot 分层保存 root、actions、warnings、audit summary；随机 id、时间戳、token、signature 通过 normalization 移除。
- 契约 / API / 数据流：fixture Skill -> API call / component preview -> RenderOutcome -> normalized snapshot -> regression tests / CLI evidence。
- 兼容性：coffee fixture 继续作为交易主线；新增 fixtures 覆盖 Phase 2 P1 表单、媒体、dynamic 和 map preview。
- 风险控制：snapshot 不包含 Host private metadata、raw consent proof、Authorization、private key path、本地路径或隐私原文。

## 4. 实现方法

1. 阅读 `phase-2-render-ir-and-fixtures.md` 的 fixture 目录建议和 snapshot 规则。
2. 基于 Step 02-01 至 02-05 的实现，确定 fixture 放置位置：优先 `examples/fixtures/` 与 `testdata/render-ir/`，或按 owning crate tests 记录原因。
3. 创建 address-form fixture，覆盖 input/textarea/picker、chooseAddress fail closed 或 mock consent summary。
4. 创建 media-review fixture，覆盖 `format:image/file`、image preview、opaque file handle 和 media provider boundary。
5. 创建 dynamic-status fixture，覆盖 `scope.dynamic`、request/timer、expire/detach cleanup。
6. 创建 location-map-preview fixture，覆盖 location provider fail closed、map-preview node 和 fallback。
7. 增加 snapshot runner 或 focused tests，完成 normalization 和 golden comparison。
8. 更新组件/API 兼容矩阵、release gates、Phase 2 子文档和 README 索引。
9. 回填本 Step 和主 Plan 执行台账；完成后进入 Step 02-07 批次最终 Review 与整体验证 gate，不得直接进入 Phase 3。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/examples/fixtures` | 新增 address-form、media-review、dynamic-status、location-map-preview fixture | 已采用共享 fixture 目录 |
| `anp/anp-miniapp-dock/testdata/render-ir` | 新增 golden snapshots | 已采用共享 snapshot 目录 |
| `anp/anp-miniapp-dock/crates/component-runtime/tests/render_ir_snapshots.rs` | snapshot runner / Render IR tests | 必须 |
| `anp/anp-miniapp-dock/crates/dock-cli/tests/coffee_order_flow.rs` | CLI validate / preview fixture tests | 必须 |
| `anp/anp-miniapp-dock/crates/dock-core/tests` | action -> Orchestrator -> render loop tests | 未新增；本 Step 以 component-runtime snapshot 和 CLI preview 覆盖，Orchestrator 主线已由既有 coffee E2E 覆盖 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步 fixture/snapshot 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 fixture 覆盖的 high-risk boundary 状态 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步 Render IR golden snapshot gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md` | 同步 fixture 目录和完成状态 | 必须 |
| `anp/anp-miniapp-dock/README.md` | 同步 fixture / snapshot 命令入口 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账并指向 Step 02-07 final Review gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/02-06-fixtures-render-ir-snapshots.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-01、Step 02-02、Step 02-03、Step 02-04、Step 02-05。
- 外部文档或决策：Render IR contract、fixture 规则、组件/API 兼容矩阵、Release Gates、Threat Model。
- 环境前提：Rust toolchain 1.88.0；fixtures 必须 mock-only，不依赖真实外部 Host provider。

## 7. 验收标准

- [x] 至少新增 address-form、media-review、dynamic-status、location-map-preview 四类 fixture，或明确记录不能新增的 blocker。
- [x] 每个 fixture 有 Render IR snapshot，包含 schemaVersion、root、actions、warnings 和必要 audit summary。
- [x] Snapshot normalization 移除随机 id、时间戳、token、signature、Authorization、private key path、本地路径和隐私原文。
- [x] address-form 覆盖表单节点和 address consent/provider boundary。
- [x] media-review 覆盖 image/file format、opaque file handle 和 media provider boundary。
- [x] dynamic-status 覆盖 dynamic request/timer、resource limit、expire/detach cleanup。
- [x] location-map-preview 覆盖 location fail closed、map-preview node 和 fallback。
- [x] 兼容矩阵、release gates、Phase 2 文档和 README 索引与 fixture 状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入 Step 02-07 最终 Review gate 之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Snapshot tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime snapshot` | snapshot / fixture tests 通过；若 filter 不匹配，记录实际命令 |
| CLI fixture tests | `cd anp/anp-miniapp-dock && cargo test -p dock-cli fixture` | CLI preview / fixture tests 通过；若未新增 CLI test，记录原因 |
| Workspace 回归 | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过；如耗时受限，记录 focused 替代和风险 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- examples testdata crates/component-runtime crates/dock-cli crates/dock-core docs/architecture docs/runbook docs/plan README.md` | 无空白错误 |
| 敏感信息扫描 | `cd anp/anp-miniapp-dock && rg -n "token|Authorization|signature|private key|phone|address|latitude|longitude" examples testdata docs/plan docs/architecture docs/runbook` | 命中只允许出现在 mock/dev-only 文档、redaction 规则或脱敏 fixture；不得包含真实 secret 或隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

本次执行证据：

- `cargo fmt --check`：通过。
- `cargo test -p component-runtime snapshot`：通过，Render IR snapshot runner 和 fixture package tests 通过。
- `cargo test -p dock-cli fixture`：通过，fixture `validate` / `preview-component` 覆盖 address-form、media-review、location-map-preview 和 dynamic-status。
- `cargo run -p dock-cli -- validate examples/fixtures/dynamic-status`：通过，输出 `status: ok`、`compatibilityLevel: demo-only`，dynamic component 被识别并保留 production Host policy warning。
- `rg -n "Authorization|Signature|Signature-Input|fixture-token|private key|phoneNumber|real_address|latitude|longitude|/home/|/Users/" testdata/render-ir examples/fixtures`：无命中。
- `git diff --check -- examples testdata crates/component-runtime crates/dock-cli crates/dock-core docs/architecture docs/runbook docs/plan README.md`：无输出。
- `cargo test --workspace`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p mcp-schema`：13 passed。
- broad sensitive scan 命中只出现在 mock handles、redaction 规则、安全文档和计划说明；未发现真实 secret、真实 token、真实地址、手机号、精确经纬度或本机路径写入 fixture/snapshot。

## 9. Review 环节

- Review 时机：fixtures、snapshots、tests、文档同步完成后、commit 前。
- Review 重点：snapshot 是否稳定；是否包含敏感数据；fixture 是否真实覆盖 P1 能力；mock/dev-only 是否清晰；release gates 是否把 snapshot 纳入后续门禁。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录并修复 | dynamic snapshot 初版在 mount 前计算 `brokerCalls`，会把真实 broker 调用数记录为 `0`；CLI / schema dynamic policy 文案仍暗示 Phase 2 gate 未实现。 |
| 已修复问题 | 已修复 | dynamic snapshot 改为 mount 后读取 broker 调用并断言 `brokerCalls == 1`；`mcp-schema` 与 `dock-cli` dynamic policy 文案改为 Step 02-05 runtime gate 已存在、production Host policy 仍必需。 |
| 剩余风险 | 已记录 | 真实 Host provider、Host renderer conformance、production network transport、background scheduler 和 persistent request/audit 仍在 Phase 3/4，不由本 Step 声明 production-ready。 |
| 新增或缺失测试 | 已补齐本 Step 范围 | 新增 `crates/component-runtime/tests/render_ir_snapshots.rs` 覆盖四类 fixtures、snapshot compare、sensitive string guard、dynamic broker call 和 expire 后拒绝事件；新增 `dock-cli` fixture validate/preview 测试。未新增 `dock-core` orchestrator 专项测试，原因是本 Step 聚焦 Render IR snapshot/CLI fixture，既有 coffee E2E 继续覆盖 action -> render 主线。 |
| 已更新或缺失文档 | 已同步 | 更新 README、组件兼容矩阵、wx API 兼容矩阵、release gates、Phase 2 子文档、Step 文档和主 Plan 台账。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 fixtures、snapshots、runner/tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase2: add render ir fixture snapshots`

Commit 前状态：`git status --short` 显示本 Step 范围内的 fixtures、snapshots、snapshot runner、CLI fixture test、dynamic policy 文案和相关文档变更；未发现无关文件。

纳入文件：`examples/fixtures/`、`testdata/render-ir/`、`crates/component-runtime/tests/render_ir_snapshots.rs`、`crates/dock-cli/tests/coffee_order_flow.rs`、`crates/dock-cli/src/commands.rs`、`crates/mcp-schema/src/validation.rs`、`README.md`、`docs/architecture/component-compatibility-matrix.md`、`docs/architecture/wx-api-compatibility-matrix.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-2-component-runtime-alignment.md`、`docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/02-06-fixtures-render-ir-snapshots.md`。

Commit 后证据：implementation commit `f778a14 phase2: add render ir fixture snapshots`；post-commit `git status --short --branch` = `## main...origin/main [ahead 41]`，工作区无未提交实现变更。

遗留未提交变更：仅本 Step 文档和主 Plan 的 commit hash / done 状态回填，准备单独创建 docs closure commit。

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 02-06 小 Plan | 将 Phase 2 fixture 与 Render IR snapshots 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 接入 Step 02-07 final Review gate | 按 Review 发现，批次最终 Review 必须是可追踪 Step，不能只作为 free-form 下一步文字 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：golden snapshot 太早冻结可能放大后续合理迭代成本；snapshot 含隐私数据会变成长期泄露面。
- 回滚 / 回退：snapshot breaking change 必须说明 schemaVersion / migration；敏感数据命中立即阻塞并清理。
- 后续文档：本 Step 完成后进入 Step 02-07，执行 01-05 至 02-06 的最终全局 Review 和整体验证，并为 Phase 3/4 计划提供基线证据。
