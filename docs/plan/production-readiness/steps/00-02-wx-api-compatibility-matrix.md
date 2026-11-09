# Step 00-02：wx API 兼容矩阵

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：00-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 10:18:26 +0800 |
| Completed | 2026-06-12 10:31:32 +0800 |
| Commit | `22e7f25` |
| Review evidence | 初审未发现需修复的矩阵内容问题；确认状态未过度承诺，`wx.login` / `wx.request` 仍标记为 `demo-only`，高风险 API 均要求 ConsentGate/audit 或明确 unsupported |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 3]`；`git diff --check -- docs/architecture/wx-api-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md` 无输出；协议覆盖抽样 `rg 'wx\.login|wx\.request|wx\.requestPayment|wx\.getPhoneNumber|wx\.chooseAddress|wx\.modelContext' docs/weichat-miniapp-mcp-protocol docs/architecture/wx-api-compatibility-matrix.md` 命中协议参考和矩阵；按表结构检查 status 列无非法枚举；矩阵 Markdown 链接检查无破链；L3/L4 与敏感字段抽样确认有 ConsentGate/audit/fail closed/opaque handle/redaction 说明；post-commit `git status --short --branch` = `## main...origin/main [ahead 4]` |
| Next action | 进入 Step 00-03 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`，覆盖小程序 MCP 关键 `wx.*` / `wx.modelContext` API 的环境、状态、映射、风险和测试证据。
- 用户 / 系统可见行为：后续 Phase 1 实现时，每个 API 都有明确的支持状态、目标 Phase、owner crate、错误语义和安全边界。
- 非目标：不实现任何 API；不改变 JS bridge；不补测试代码。
- 完成标准：矩阵覆盖本地协议参考中的关键 API；状态枚举一致；unsupported API 有原因和建议；高风险 API 有 risk level 和 ConsentGate/Host provider 策略。

## 3. 设计方法

- 设计边界：矩阵是 Phase 1/3/5 的契约输入，不是实现代码。
- 核心决策：矩阵状态固定为 `supported`、`host-boundary`、`planned-p1`、`planned-p2`、`demo-only`、`unsupported-by-design`。
- 契约 / API / 数据流：记录 `wx.login`、`wx.checkSession`、`wx.request`、storage、payment、phone/address/location/media/file、unsupported API、`wx.modelContext` 的 ANP DID / Host provider / RequestBroker 映射。
- 兼容性：微信兼容行为不确定时，标记为需要 Phase 1 contract 决策，不能在矩阵中模糊处理。
- 风险控制：所有 L3/L4 能力默认需要 consent/audit；所有 unsupported 能力默认 deterministic fail closed。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt`，抽取原子接口环境、原子组件环境、半屏页面环境中与当前容器相关的 API。
2. 读取 Step 00-01 输出的当前能力基线，确认现有支持、host-boundary 和 demo-only 状态。
3. 新增 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`，建议字段为：`category`、`api`、`environment`、`status`、`target_phase`、`runtime_mapping`、`risk_level`、`owner_crate`、`callback_promise`、`tests`、`notes`。
4. 对关键 API 写清映射策略：ANP DID 替代、Host provider、RequestBroker、Storage scope、Payment Intent、ConsentGate、unsupported error。
5. 更新 `anp/anp-miniapp-dock/README.md` 或相关架构索引，加入矩阵入口。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 新增 wx API 兼容矩阵 | 必须 |
| `anp/anp-miniapp-dock/docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt` | 读取参考 | 不修改 |
| `anp/anp-miniapp-dock/docs/architecture/current-capability-baseline.md` | 读取 Step 00-01 输出 | 不修改，除非发现错误并更新台账 |
| `anp/anp-miniapp-dock/README.md` | 补文档入口链接 | 如新增矩阵文档则建议 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-01。
- 外部文档或决策：Phase 0 文档、小程序 MCP 本地参考、Step 00-01 基线。
- 环境前提：无需运行服务；需要能读取本地协议参考。

## 7. 验收标准

- [x] 矩阵覆盖 `wx.modelContext`、auth、network、storage、privacy、media/file、location、payment、device/scan/phone call、unsupported API 分组。
- [x] 每行都有 status、target phase、owner crate 或明确 `unsupported-by-design` 原因。
- [x] `wx.login`、`wx.checkSession`、`wx.request`、storage、`wx.requestPayment`、`wx.getPhoneNumber`、`wx.chooseAddress` 有明确 ANP DID / Host provider / consent 映射。
- [x] callback/Promise 不确定项明确标为 Phase 1 contract 决策点。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档路径检查 | `cd anp/anp-miniapp-dock && git diff --check -- docs/architecture/wx-api-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md` | 无空白错误 |
| 协议覆盖抽样 | `cd anp/anp-miniapp-dock && rg "wx\\.login|wx\\.request|wx\\.requestPayment|wx\\.getPhoneNumber|wx\\.chooseAddress|wx\\.modelContext" docs/weichat-miniapp-mcp-protocol docs/architecture/wx-api-compatibility-matrix.md` | 关键 API 在矩阵中可追踪 |
| 状态枚举检查 | 手工检查矩阵状态值 | 只使用约定状态 |
| 安全检查 | 手工检查 L3/L4 API 行 | 均要求 consent/audit 或明确 unsupported |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：矩阵完成、覆盖抽样和路径检查完成后、commit 前。
- Review 重点：API 覆盖是否漏项；状态是否过度承诺；unsupported 是否有原因；高风险 API 是否默认 fail closed；是否为 Phase 1 contract 留出清晰决策点。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 无阻塞问题 | 矩阵未把协议“支持”误写为本项目生产支持；`wx.login`、`wx.request` 当前能力仍标为 `demo-only`，Host/provider 边界未被过度承诺。 |
| 已修复问题 | 无文档内容修复 | 验证中发现一次 status 枚举检查命令过宽，会误报 API 名称列；已改用按表结构定位 status 列的 `awk` 检查，未把失败命令作为通过证据。 |
| 剩余风险 | 可接受 | 协议参考长尾 API 较多，本 Step 以关键 API 和大类 unsupported/deferred 分组覆盖；后续 Step 01-01 需冻结 callback/Promise、错误语义和 unsupported stub 形态。 |
| 新增或缺失测试 | 未新增自动化测试 | 本 Step 为文档矩阵冻结；验证使用 diff whitespace 检查、协议覆盖抽样、状态枚举结构化检查、Markdown 链接检查和高风险 API 安全抽样。 |
| 已更新或缺失文档 | 已更新 | 新增 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`，并在 `anp/anp-miniapp-dock/README.md` 增加入口链接；主 Plan 与本 Step 文档已回填 review/verification evidence。 |

## 10. Commit 要求

- Commit 时机：矩阵、索引、验证、Review 完成后。
- Commit 范围：只包含 Step 00-02 的 API 矩阵和直接索引变更。
- Commit 前状态：`git status --short --branch` 显示 `README.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md` 修改，`docs/architecture/wx-api-compatibility-matrix.md` 新增。
- 纳入文件：`anp/anp-miniapp-dock/README.md`、`anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md`。
- Commit 后证据：主矩阵 commit `22e7f25 docs: add wx api compatibility matrix`；post-commit `git status --short --branch` = `## main...origin/main [ahead 4]`。台账关闭状态由后续小文档提交保存。
- 建议消息：`docs: add wx api compatibility matrix`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 00-02 小 Plan | 将 Phase 0 API 矩阵拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：协议参考较长，抽取时可能遗漏低频 API。
- 回滚 / 回退：遗漏项以后续修正 commit 补齐；如果状态误判，先修矩阵再继续 Phase 1 实现。
- 后续文档：Step 01-01 将基于本矩阵冻结 callback/Promise 与错误语义。
