# Step 00-01：当前能力盘点与基线固化

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：00-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 10:05:15 +0800 |
| Completed | 2026-06-12 10:16:44 +0800 |
| Commit | `de4c3e2` |
| Review evidence | 初审未发现需修复问题；重点复核能力未夸大、demo-only/host-boundary 已明确、证据可追踪 |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 1]`；`cargo metadata --format-version 1 --no-deps` 成功并确认 11 个 crate；`git diff --check -- docs/architecture README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-01-baseline-inventory.md` 无输出；新增基线文档 Markdown 链接手工检查无破链；状态一致性已对照 `README.md`、architecture docs、runbook 和 tests |
| Next action | 进入 Step 00-02 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把当前 P0/P0.5 Demo 能力整理成可追踪基线，说明每项能力的 owner crate、证据、状态和限制。
- 用户 / 系统可见行为：后续执行者能从文档回答“当前已经做到什么、由哪个 crate 负责、哪些测试证明、哪些只是 demo-only”。
- 非目标：不新增运行时能力；不重构 Rust 代码；不改变 CLI、demo-server 或 Skill 行为。
- 完成标准：能力清单覆盖 workspace 主要 crate、CLI/demo 入口、coffee Skill、FastAPI/Mac 辅助链路；每项能力有状态和证据；demo-only 能力明确标注；Phase 0 后续矩阵和 release gates 可以引用该基线。

## 3. 设计方法

- 设计边界：本 Step 是文档基线冻结，只允许修改文档和必要索引。
- 核心决策：状态枚举使用 `implemented`、`host-boundary`、`demo-only`、`planned`、`unsupported-by-design`，并和后续兼容矩阵的状态区分清楚。
- 契约 / API / 数据流：记录现有 crate 责任边界，不改变 `mcp.json`、Atomic API、Component Runtime、ANP DID 或 CLI JSON contract。
- 兼容性：以现有 `README.md`、`docs/architecture/`、`docs/runbook/`、Cargo workspace 和 tests 为事实来源。
- 风险控制：任何无法由源码、测试或文档证明的能力标为 `planned` 或 `unknown-to-resolve`，不能标为 production-ready。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/README.md`、`anp/anp-miniapp-dock/Cargo.toml`、`anp/anp-miniapp-dock/docs/architecture/`、`anp/anp-miniapp-dock/docs/runbook/`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-0-baseline-and-gates.md`。
2. 使用 `rg --files anp/anp-miniapp-dock/crates anp/anp-miniapp-dock/examples` 和 `cargo metadata --format-version 1 --no-deps` 确认 workspace crate 与 examples。
3. 建立或更新基线文档，建议路径为 `anp/anp-miniapp-dock/docs/architecture/current-capability-baseline.md`；如选择合并到已有架构文档，必须在主 Plan 执行台账中记录原因。
4. 按 crate / 入口整理能力、状态、证据、限制和后续 Phase。
5. 标注 demo-only 能力：localhost `wx.request` bridge、mock payment、mock consent provider、非生产 FastAPI 示例、Mac host demo 等。
6. 在本 Step 文档和主 Plan 执行台账中记录实际新增/修改文件、验证证据和 Review 结论。

## 5. 路径

本节路径相对 AWiki workspace 根目录。

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/architecture/current-capability-baseline.md` | 新增当前能力基线文档 | 推荐输出路径 |
| `anp/anp-miniapp-dock/docs/architecture/anp-skill-dock-architecture.md` | 仅在需要时补链接 | 不复制大段内容 |
| `anp/anp-miniapp-dock/README.md` | 如新增基线文档，补入口链接 | 文档索引变更 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-01-baseline-inventory.md` | 回填状态、证据、Review、commit | 必须更新 |

## 6. 依赖

- 前置步骤：无。
- 外部文档或决策：以 `anp/anp-miniapp-dock/AGENTS.md`、主 Plan、Phase 0 文档为准。
- 环境前提：Rust toolchain 可用于 `cargo metadata`；若不可用，记录原因并使用 `Cargo.toml` 与 `rg --files` 作为替代证据。

## 7. 验收标准

- [x] 基线文档覆盖 `mcp-schema`、`skill-loader`、`js-runtime-quickjs`、`component-runtime`、`wx-compat`、`anp-adapter`、`consent-audit`、`card-spec`、`dock-core`、`dock-cli`、`demo-server`、`examples/coffee-skill`。
- [x] 每项能力都有 owner crate、状态、证据和限制说明。
- [x] demo-only、host-boundary、planned 能力没有被误标为 production-ready。
- [x] 基线文档引用的文件路径存在，Markdown 链接可解析。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| Workspace membership | `cd anp/anp-miniapp-dock && cargo metadata --format-version 1 --no-deps` | 命令成功，crate 列表支撑基线清单 |
| 文档路径检查 | `cd anp/anp-miniapp-dock && git diff --check -- docs/architecture README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-01-baseline-inventory.md` | 无 trailing whitespace 或 patch 空白错误 |
| 手工链接检查 | 检查新增 Markdown 链接是否指向存在文件 | 无破链 |
| 状态一致性 | 对照 `README.md`、architecture docs、runbook 和 tests | demo-only / host-boundary 标注与现状一致 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：基线文档完成、验证运行后、commit 前。
- Review 重点：能力是否夸大、证据是否可追踪、demo-only 是否明确、路径和术语是否符合 `AGENTS.md`、是否为后续矩阵提供足够输入。
- Review 结论必须记录在下表和主 Plan 执行台账中。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 无阻塞问题 | 初审检查能力表未把 `wx.login` / `wx.request` localhost bridge、mock consent、mock payment、FastAPI 和 Mac host 写成 production-ready。 |
| 已修复问题 | 无 | 验证前发现一次链接检查命令写法错误，已用更简单命令重跑；未作为通过证据记录。 |
| 剩余风险 | 可接受 | 基线是代码/文档现状快照；后续 Step 00-02/00-03 需要继续按 API 和组件全量矩阵细化覆盖。 |
| 新增或缺失测试 | 未新增自动化测试 | 本 Step 为文档基线冻结；验证使用 `cargo metadata`、Markdown 链接检查、diff whitespace 检查和状态一致性人工复核。 |
| 已更新或缺失文档 | 已更新 | 新增 `anp/anp-miniapp-dock/docs/architecture/current-capability-baseline.md`，并在 `anp/anp-miniapp-dock/README.md` 增加入口链接。 |

## 10. Commit 要求

- Commit 时机：本 Step 文档、基线文档、必要索引、验证和 Review 完成后。
- Commit 范围：只包含 Step 00-01 的文档基线和索引变更。
- Commit 前状态：`git status --short` 显示 `README.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/00-01-baseline-inventory.md` 修改，`docs/architecture/current-capability-baseline.md` 新增。
- 纳入文件：`anp/anp-miniapp-dock/README.md`、`anp/anp-miniapp-dock/docs/architecture/current-capability-baseline.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-01-baseline-inventory.md`。
- Commit 后证据：主基线 commit `de4c3e2 docs: freeze production readiness baseline`；post-commit `git status --short --branch` = `## main...origin/main [ahead 2]`。台账关闭状态由后续小文档提交保存。
- 建议消息：`docs: freeze production readiness baseline`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 00-01 小 Plan | 将 Phase 0 当前能力盘点拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：基线文档可能遗漏未被 README 暴露的测试或 crate 内能力。
- 回滚 / 回退：若发现事实错误，先修正文档和执行台账；尚未 commit 时直接修正，已 commit 时用后续修正 commit。
- 后续文档：Step 00-02、00-03、00-04 将引用本基线输出。
