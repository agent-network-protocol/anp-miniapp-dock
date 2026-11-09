# Step 02-04：表单与静态媒体节点

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：02-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 21:54:59 +0800 |
| Completed | 2026-06-12 22:03:40 +0800 |
| Commit | `cc7b3b8` |
| Review evidence | 本文 Review 环节已记录：修复 `maxlength` 等数值 props 初版以字符串输出的问题；确认新增表单节点只是 Render IR 数据，disabled 会抑制 `input` / `change` / tap event，`map-preview` 不透传精确经纬度/markers，`canvas-static` 不开放 script/touch 交互，未绕过 Orchestrator input validation、ConsentGate 或 Host provider。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p component-runtime node` 7 passed under filter（lib 3 passed、wxml_bindings 4 passed）；`cargo test -p component-runtime` 40 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime crates/dock-core docs/architecture docs/plan` 无输出；敏感词抽样仅命中本步骤精确经纬度/markers 拒绝测试、文档安全说明和既有台账文字，Render IR 不输出这些字段。 |
| Next action | 进入 Step 02-05 Dynamic component controls |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：在 Render IR 中新增 P1 表单和静态媒体节点：`input`、`textarea`、`radio`、`checkbox`、`picker`、`map-preview`、`canvas-static`。
- 用户 / 系统可见行为：组件可以表达地址表单、选择控件、静态地图预览和静态 canvas 占位，Host 可按 node kind 渲染或 fallback。
- 非目标：不实现完整输入法事件模型、复杂 form submit、MapContext 交互、任意 canvas 脚本、真实位置 provider 或真实媒体 provider。
- 完成标准：新增 node kind 有 parser/compiler/Render IR tests，Host 不支持时有 warning/fallback，表单值不能绕过 Orchestrator input validation。

## 3. 设计方法

- 设计边界：表单节点是 Render IR 数据，不直接执行高风险动作；用户提交仍要回到 Host / Orchestrator 受控路径。
- 核心决策：表单节点只输出安全 props、value、placeholder、options、disabled、events；`map-preview` 和 `canvas-static` 只表达静态预览，不开放交互 API。
- 契约 / API / 数据流：WXML node -> compiler -> `RenderNodeKind` -> Host renderer；用户事件 -> controlled action -> Orchestrator / Host provider。
- 兼容性：未知 Host renderer 可以 fallback placeholder 或整卡 fallback；现有 view/text/image/button/scroll-view 不回归。
- 风险控制：位置、地址、文件/media 值不直接从节点进入 Skill API；真实 L4 数据需 Step 01-08 provider boundary 和 consent。

## 4. 实现方法

1. 阅读 `phase-2-render-ir-and-fixtures.md` 中 P1 Node Kind Registry。
2. 阅读 Step 01-08 高风险 Host boundary，确认表单/媒体节点不绕过 provider。
3. 在 `component-runtime` 扩展 `RenderNodeKind`、parser/compiler、props normalization 和 warnings。
4. 为 `input`、`textarea`、`radio`、`checkbox`、`picker` 定义最小 props 和 event 输出；disabled 时阻断 action。
5. 为 `map` preview 和 `canvas` static 定义 `map-preview` / `canvas-static` node，不支持 `MapContext.*` 或 canvas script。
6. 增加 tests：每类 node output、disabled、options、unknown props warning、Host fallback reason、敏感字段不进入 Render IR。
7. 更新组件兼容矩阵、Phase 2 文档和 Render IR 子文档。
8. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/component-runtime/src/render_ir.rs` | 新增 node kind 和 props | 代码实现 |
| `anp/anp-miniapp-dock/crates/component-runtime/src/compiler.rs` | 表单/media node 编译 | 代码实现 |
| `anp/anp-miniapp-dock/crates/component-runtime/src/wxml.rs` | 如需 parser 支持 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/component-runtime/tests` | node / fallback / warning tests | 必须 |
| `anp/anp-miniapp-dock/crates/dock-core` | 如需 fallback reason 映射 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 同步 P1 node 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md` | 同步 Node Kind Registry | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/02-04-form-static-media-nodes.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-08、Step 02-01、Step 02-03。
- 外部文档或决策：Render IR contract、组件兼容矩阵、高风险 API Host boundary。
- 环境前提：Rust toolchain 1.88.0；无需真实 Host renderer 或真实位置/media provider。

## 7. 验收标准

- [x] Render IR 支持 `input`、`textarea`、`radio`、`checkbox`、`picker` node kind 和最小 props。
- [x] Render IR 支持 `map-preview`、`canvas-static` 静态节点，不开放 MapContext/canvas script。
- [x] disabled 表单或按钮节点不会产生可执行 action。
- [x] 表单值进入后续 API 前仍需 Orchestrator input validation、permission、ConsentGate 和 audit。
- [x] Host 不支持新增 node kind 时 fallback / warning 可观测，不静默执行未知 action。
- [x] 组件兼容矩阵和 Render IR 子文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Node tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime node` | 新 node tests 通过；若 filter 不匹配，记录实际命令 |
| Component 回归 | `cd anp/anp-miniapp-dock && cargo test -p component-runtime` | 全部 component runtime tests 通过 |
| Core / CLI 回归 | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/component-runtime crates/dock-core docs/architecture docs/plan` | 无空白错误 |
| 安全抽样 | 手工检查 Render IR props/actions | 不含手机号、地址原文、精确位置、文件内容、本地路径、token、Authorization、signature |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：新增 node 是否只是数据；是否绕过 consent/input validation；Host unknown node 是否 fallback；map/canvas 是否没有交互 API。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 初版 `maxlength` 等数值 props 以字符串输出，不利于 Host renderer 稳定消费；需确认 map/canvas 不透传交互或精确位置字段。 | Review 前已修复数值属性归一化，并通过 tests 验证静态媒体边界。 |
| 已修复问题 | `maxlength`、`scale`、`width`、`height` 现在可归一为 JSON number；`map-preview` 对 `latitude` / `longitude` / `markers` / `polyline` / `controls` warning 且不透传；`canvas-static` 对 script / draw / touch event warning 且不生成事件。 | focused tests 已覆盖。 |
| 剩余风险 | 真实 Host 输入控件、表单提交路径、位置/media provider、Host fallback renderer 和 golden snapshots 仍未实现。 | 按计划留到 Step 02-06 和 Phase 4；本 Step 只定义 Render IR 数据边界。 |
| 新增或缺失测试 | 新增 node kind registry、表单 props/event、disabled 抑制、map/canvas 静态边界 tests；未新增真实 Host renderer E2E 或 golden snapshot。 | Host E2E / snapshots 不属于 02-04 范围。 |
| 已更新或缺失文档 | 已更新组件兼容矩阵、Phase 2 component runtime alignment、Render IR 子文档、本 Step 和主 Plan 台账。 | 未更新 Host adapter contract，留到 Phase 4。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 P1 表单/静态媒体 node、直接 tests 和相关文档。
- Commit 前状态：`git status --short` 包含本 Step 的 `component-runtime` node kind / compiler / event / tests、组件矩阵、Phase 2 文档、Render IR 子文档、Step 文档和主 Plan in-progress/review 记录，未发现其它 Step 完成工作。
- 纳入文件：`crates/component-runtime/src/compiler.rs`、`crates/component-runtime/src/events.rs`、`crates/component-runtime/src/render_ir.rs`、`crates/component-runtime/tests/wxml_bindings.rs`、`docs/architecture/component-compatibility-matrix.md`、`docs/plan/production-readiness/phase-2-component-runtime-alignment.md`、`docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md`、`docs/plan/production-readiness/steps/02-04-form-static-media-nodes.md`、`docs/plan/production-readiness-roadmap.md`。
- Commit 后证据：实现提交 `cc7b3b8 phase2: add form static media nodes`；提交后 `git status --short --branch` = `## main...origin/main [ahead 37]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase2: add form static media nodes`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 02-04 小 Plan | 将 Phase 2 表单与静态媒体节点拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：表单节点如果被误当作直接 API input，会绕过验证和 consent。
- 回滚 / 回退：保持表单输出为 Render IR 数据；Host event 必须回到 Orchestrator。
- 后续文档：Step 02-06 address-form 和 location-map-preview snapshots 必须覆盖这些 node。
