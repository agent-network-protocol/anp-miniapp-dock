# Step 00-03：组件兼容矩阵

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：00-03
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 10:33:02 +0800 |
| Completed | 2026-06-12 10:40:11 +0800 |
| Commit | `fea9d35` |
| Review evidence | 初审未发现需修复的组件矩阵内容问题；确认 Runtime/Host 责任分离，未把 Host renderer、dynamic request/timer、完整页面/半屏能力误标为 production-ready |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 5]`；`git diff --check -- docs/architecture/component-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md` 无输出；覆盖抽样 `rg "Component|WXML|WXSS|Render IR|sendFollowUpMessage|api/call|expireAllCards|dynamic" docs/architecture/component-compatibility-matrix.md docs/architecture/miniapp-mcp-component-runtime.md docs/plan/production-readiness/phase-2-component-runtime-alignment.md` 命中矩阵、架构和 Phase 2 计划；按表结构检查 status 列无非法枚举；矩阵 Markdown 链接检查无破链；安全边界抽样确认 `api/call`、dynamic、L3/L4、token/redaction、Host unknown action 均有约束；Host 边界抽样确认 renderer/provider/card manager 均未写成容器已生产支持；post-commit `git status --short --branch` = `## main...origin/main [ahead 6]` |
| Next action | 进入 Step 00-04 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增 `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md`，记录 Component JS、WXML、WXSS、内置组件、事件、动态组件、Render IR、Host fallback 的支持状态。
- 用户 / 系统可见行为：Phase 2 实现前，交易型卡片必须能力、可延期能力和 unsupported-by-design 能力边界清晰。
- 非目标：不实现组件能力；不新增 fixture；不修改 Render IR schema。
- 完成标准：矩阵覆盖当前 coffee 组件能力和 Phase 2 P1/P2 目标；每项能力有状态、目标 Phase、owner crate、fixture/snapshot 需求和 fallback 策略。

## 3. 设计方法

- 设计边界：容器承诺 Render IR contract，不承诺完整微信 UI runtime。
- 核心决策：区分 Component JS、WXML、WXSS、内置组件、事件、`wx.modelContext`、动态组件和 Host adapter，不把 Host renderer 责任写成 component-runtime 责任。
- 契约 / API / 数据流：Render IR 是 `component-runtime` 与 Host renderer 的边界；高风险 action 必须回到 Orchestrator/ConsentGate。
- 兼容性：当前 P0 能力、目标 P1/P2 能力和明确不做的完整小程序页面能力分开记录。
- 风险控制：所有动态 request/timer 默认关闭，只有声明 `permissions.scope.dynamic` 后才允许受限开放。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/architecture/miniapp-mcp-component-runtime.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-component-runtime-alignment.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-render-ir-and-fixtures.md`。
2. 读取 Step 00-01 输出的当前能力基线，确认已实现的 Component VM、WXML/WXSS、events、Render IR 能力。
3. 新增 `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md`，建议字段为：`category`、`capability`、`status`、`target_phase`、`render_ir_mapping`、`host_boundary`、`security_notes`、`fixture_or_snapshot`、`owner_crate`、`notes`。
4. 显式列出不支持或后置能力：完整页面路由、TabBar、半屏页面、slots、behaviors、relations、复杂 WXML 表达式、完整地图交互等。
5. 更新 `anp/anp-miniapp-dock/README.md` 或相关架构索引，加入矩阵入口。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 新增组件兼容矩阵 | 必须 |
| `anp/anp-miniapp-dock/docs/architecture/miniapp-mcp-component-runtime.md` | 读取或补链接 | 避免重复长文 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-2-component-runtime-alignment.md` | 读取 Phase 2 目标 | 不修改，除非发现计划漂移 |
| `anp/anp-miniapp-dock/README.md` | 补文档入口链接 | 如新增矩阵文档则建议 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-01。
- 外部文档或决策：Phase 2 文档、Render IR 子文档、组件架构文档。
- 环境前提：无需运行服务。

## 7. 验收标准

- [x] 矩阵覆盖 Component JS、WXML、WXSS、内置组件、事件、`wx.modelContext`、动态组件、Render IR / Host adapter。
- [x] 当前 P0 支持、Phase 2 P1/P2 计划、unsupported-by-design 能力分离清晰。
- [x] 每项能力有 owner crate、target phase、fixture/snapshot 需求或不需要的原因。
- [x] 高风险 action 不能通过 Render IR 直接执行，必须回 Orchestrator/Host provider consent。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档路径检查 | `cd anp/anp-miniapp-dock && git diff --check -- docs/architecture/component-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md` | 无空白错误 |
| 覆盖抽样 | `cd anp/anp-miniapp-dock && rg "Component|WXML|WXSS|Render IR|sendFollowUpMessage|api/call|expireAllCards|dynamic" docs/architecture/component-compatibility-matrix.md docs/architecture/miniapp-mcp-component-runtime.md docs/plan/production-readiness/phase-2-component-runtime-alignment.md` | 关键能力可追踪 |
| 安全边界检查 | 手工检查 action 和 dynamic 行 | 高风险 action 和 dynamic request 有限制 |
| Host 边界检查 | 手工检查 `host-boundary` 说明 | Host renderer 不阻塞 component runtime contract |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：组件矩阵完成、覆盖抽样和路径检查完成后、commit 前。
- Review 重点：是否混淆 Runtime 和 Host 责任；是否过度承诺完整小程序能力；动态能力是否默认关闭；fixture/snapshot 需求是否足够指导 Phase 2。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 无阻塞问题 | 矩阵未把 Host renderer、dynamic request/timer、`openDetailPage`、完整半屏页面、完整 WXML/WXSS 或完整自定义组件系统写成当前生产能力。 |
| 已修复问题 | 无文档内容修复 | 验证中发现一次 status 枚举检查命令未跳过表头，会误报 `status`；已用修正后的按表结构检查重跑，未把失败命令作为通过证据。 |
| 剩余风险 | 可接受 | 协议组件支持列表和 WXSS 属性范围较长，本 Step 以 P0/P1/P2 能力和大类 unsupported 分组覆盖；Phase 2 实现前需冻结 Render IR schemaVersion、fallback reason enum 和 fixture/snapshot 体系。 |
| 新增或缺失测试 | 未新增自动化测试 | 本 Step 为文档矩阵冻结；验证使用 diff whitespace 检查、覆盖抽样、状态枚举结构化检查、Markdown 链接检查、安全边界抽样和 Host 边界抽样。 |
| 已更新或缺失文档 | 已更新 | 新增 `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md`，并在 `anp/anp-miniapp-dock/README.md` 增加入口链接；主 Plan 与本 Step 文档已回填 review/verification evidence。 |

## 10. Commit 要求

- Commit 时机：矩阵、索引、验证、Review 完成后。
- Commit 范围：只包含 Step 00-03 的组件矩阵和直接索引变更。
- Commit 前状态：`git status --short --branch` 显示 `README.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md` 修改，`docs/architecture/component-compatibility-matrix.md` 新增。
- 纳入文件：`anp/anp-miniapp-dock/README.md`、`anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md`。
- Commit 后证据：主矩阵 commit `fea9d35 docs: add component compatibility matrix`；post-commit `git status --short --branch` = `## main...origin/main [ahead 6]`。台账关闭状态由后续小文档提交保存。
- 建议消息：`docs: add component compatibility matrix`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 00-03 小 Plan | 将 Phase 0 组件矩阵拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：矩阵可能把 Host renderer 展示能力误写为容器必须实现能力。
- 回滚 / 回退：发现职责混淆时先修正文档和执行台账，再进入 Phase 2。
- 后续文档：Step 01-02 和 Phase 2 Step 将引用组件 metadata、fixture/snapshot 和 Render IR 边界。
