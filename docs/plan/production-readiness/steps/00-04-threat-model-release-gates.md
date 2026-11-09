# Step 00-04：Threat model 与 release gates 初版

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：00-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 10:41:43 +0800 |
| Completed | 2026-06-12 10:51:58 +0800 |
| Commit | `04448a1` |
| Review evidence | 初审未发现需修复的安全/发布门槛内容问题；确认 planned gate 未被误写成当前已自动化，demo-only/mock 能力仍为 production release blocker |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 7]`；`git diff --check -- docs/security docs/runbook/release-gates.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md` 无输出；安全红线抽样 `rg -n "token|Authorization|signature|private key|ConsentGate|audit|sandbox|allowlist|fail closed" docs/security docs/runbook/release-gates.md` 命中 threat model、release gates 和 redaction 规则；README、threat model、release gates Markdown 相对链接检查无破链；release gate 命令与 `AGENTS.md` 手工核对一致；post-commit `git status --short --branch` = `## main...origin/main [ahead 8]` |
| Next action | 进入 Step 01-01 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：新增 `anp/anp-miniapp-dock/docs/security/threat-model.md` 和 `anp/anp-miniapp-dock/docs/runbook/release-gates.md` 初版。
- 用户 / 系统可见行为：后续开发有明确安全红线、release gate、回滚条件和验证命令，避免 demo-only 能力进入生产路径。
- 非目标：不实现安全功能；不接入 CI；不新增持久化 audit 或 package signing。
- 完成标准：threat model 覆盖核心攻击者、资产、控制、测试证据和残余风险；release gates 覆盖基础 Cargo gates、文档 gates、安全 gates、fixture/snapshot gates 和失败回滚规则。

## 3. 设计方法

- 设计边界：本 Step 定义安全和发布门槛初版，为 Phase 3 详细安全实现提供输入。
- 核心决策：高风险能力默认 fail closed；未配置 consent/provider 时不得执行 L3/L4 API；任何 token/signature/private key path 不得进入输出。
- 契约 / API / 数据流：安全边界覆盖 Skill 包、QuickJS sandbox、Component Runtime、RequestBroker、ANP DID、capability token、ConsentGate、audit、Render IR、CLI/demo output。
- 兼容性：release gates 先以本仓库本地命令为主，未来可映射到 CI。
- 风险控制：无法自动化的检查必须写成手工检查和残余风险，不允许写成已自动保障。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/runbook/security.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-threat-model-and-controls.md`。
2. 结合 Step 00-01、00-02、00-03 输出，列出资产、攻击者、控制措施、测试证据和残余风险。
3. 新增 `anp/anp-miniapp-dock/docs/security/threat-model.md`；如果 `docs/security/` 不存在，创建该目录。
4. 新增 `anp/anp-miniapp-dock/docs/runbook/release-gates.md`，明确基础命令、文档检查、兼容矩阵覆盖、安全红线、失败处理和回滚条件。
5. 更新 `anp/anp-miniapp-dock/README.md` 或相关索引，加入 threat model 和 release gates 入口。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 新增安全模型 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 新增发布门槛 runbook | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/security.md` | 读取或补链接 | 保持 runbook 一致 |
| `anp/anp-miniapp-dock/README.md` | 补文档入口链接 | 如新增文档则建议 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-01、00-02、00-03。
- 外部文档或决策：Phase 3 文档、安全 runbook、API/组件矩阵。
- 环境前提：无需运行服务；基础 Cargo 命令可作为 release gates 写入，不要求本 Step 全量执行。

## 7. 验收标准

- [x] threat model 覆盖 DID private key、capability token、Skill package、scoped storage、audit records、Render IR、Host providers。
- [x] 攻击者模型覆盖恶意 Skill、被篡改 Skill 包、恶意商家 Agent、网络中间人、恶意或误配置 Host provider、日志/审计读取者、本地文件系统攻击者。
- [x] release gates 包含 `cargo metadata --format-version 1 --no-deps`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p dock-cli --test coffee_order_flow`。
- [x] release gates 明确 sandbox escape、allowlist deny、redaction、token replay/scope、Render IR snapshot、Markdown link check 的启用阶段或缺口。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档路径检查 | `cd anp/anp-miniapp-dock && git diff --check -- docs/security docs/runbook/release-gates.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md` | 无空白错误 |
| 安全红线抽样 | `cd anp/anp-miniapp-dock && rg "token|Authorization|signature|private key|ConsentGate|audit|sandbox|allowlist|fail closed" docs/security docs/runbook/release-gates.md` | 红线和 gate 可追踪 |
| release 命令检查 | 手工核对 runbook 命令和 `AGENTS.md` | 命令与仓库指导一致 |
| 目录链接检查 | 手工检查 README / runbook / threat model 链接 | 无破链 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：threat model 和 release gates 初版完成、验证后、commit 前。
- Review 重点：是否覆盖关键资产和攻击者；release gates 是否可执行；安全红线是否足够阻止高风险绕过；是否把未实现能力误写成已实现 gate。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 无阻塞问题 | threat model 覆盖 Step 要求的资产和攻击者；release gates 包含仓库基础命令、文档 gate、安全 gate、fixture/Render IR gate、demo-only 禁止项和失败回滚规则。 |
| 已修复问题 | 无文档内容修复 | 验证中未发现空白错误、相对链接破链或 release 命令与 `AGENTS.md` 不一致。 |
| 剩余风险 | 可接受 | package signature、token revoke/jti replay、persistent audit、Host provider conformance、Render IR snapshot 仍是 planned gates；文档已明确不得作为当前已通过 gate 或 production-ready 能力。 |
| 新增或缺失测试 | 未新增自动化测试 | 本 Step 为安全模型和 release runbook 初版；验证使用 diff whitespace 检查、安全红线抽样、Markdown 链接检查和 release 命令一致性核对。 |
| 已更新或缺失文档 | 已更新 | 新增 `anp/anp-miniapp-dock/docs/security/threat-model.md` 和 `anp/anp-miniapp-dock/docs/runbook/release-gates.md`，并在 `anp/anp-miniapp-dock/README.md` 增加入口链接；主 Plan 与本 Step 文档已回填 review/verification evidence。 |

## 10. Commit 要求

- Commit 时机：threat model、release gates、索引、验证、Review 完成后。
- Commit 范围：只包含 Step 00-04 的安全/发布文档和直接索引变更。
- Commit 前状态：`git status --short --branch` 显示 `README.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md` 修改，`docs/runbook/release-gates.md` 和 `docs/security/threat-model.md` 新增。
- 纳入文件：`anp/anp-miniapp-dock/README.md`、`anp/anp-miniapp-dock/docs/security/threat-model.md`、`anp/anp-miniapp-dock/docs/runbook/release-gates.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md`。
- Commit 后证据：主产物 commit `04448a1 docs: define production security and release gates`；post-commit `git status --short --branch` = `## main...origin/main [ahead 8]`。台账关闭状态由后续小文档提交保存。
- 建议消息：`docs: define production security and release gates`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 00-04 小 Plan | 将 Phase 0 threat model 与 release gates 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：release gates 初版可能包含当前还未自动化的检查。
- 回滚 / 回退：把未自动化检查标为 planned gate，避免阻塞当前文档基线；后续 Phase 实现后再升级为必跑命令。
- 后续文档：Phase 3 将细化 threat model、安全控制和 CI gate。
