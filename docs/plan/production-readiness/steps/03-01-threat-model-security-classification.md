# Step 03-01：Threat Model 与安全分级收敛

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 13:42:54 +0800 |
| Completed | 2026-06-13 13:51:12 +0800 |
| Commit | `a61a7e7` |
| Review evidence | 2026-06-13 13:49:52 +0800 commit 前 Review：修复 Threat Model 中 Render IR schemaVersion/snapshot 与 unsupported registry 的旧 planned 表述；确认 L0-L4 分级、L3/L4 高风险能力矩阵、Phase 3 required gates、Phase 4/5 后续 gate 和 demo-only/mock 禁止项未冲突，planned gate 未被误写成已自动化。 |
| Verification evidence | `git diff --check -- docs/security docs/runbook docs/architecture docs/plan` 无输出；`rg -n "L3|L4|ConsentGate|audit|redaction|fail closed" docs/security docs/runbook docs/architecture docs/plan/production-readiness` 命中高风险能力与控制说明；`rg -n "token|Authorization|signature|private key|phone|address|file content" docs/security docs/runbook docs/architecture docs/plan` 仅命中文档红线、mock/dev-only 示例、测试说明和计划台账；旧状态残留搜索无命中；新增/修改 Markdown 相对链接目标手工检查存在。 |
| Next action | 进入 Step 03-02 QuickJS 沙箱逃逸回归与资源限制 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 Phase 0 threat model 从初版清单升级为可驱动实现的安全控制矩阵。
- 用户 / 系统可见行为：每个高风险 API、组件 action、Host provider、Skill package 和 runtime 边界都能追溯到风险等级、控制措施、测试证据和 release gate。
- 非目标：不在本 Step 实现 sandbox、token、audit 或 package signing；本 Step 冻结安全分类与验收口径。
- 完成标准：`docs/security/threat-model.md`、Phase 3 子文档和 release gates 中的风险等级、owner、测试 gate、残余风险一致。

## 3. 设计方法

- 设计边界：安全文档是后续 Phase 3 实现的 contract，不能把 planned gate 写成已自动化。
- 核心决策：沿用 L0-L4 风险等级；L3/L4 默认必须有 ConsentGate、audit、redaction、provider boundary 和 fail-closed。
- 契约 / API / 数据流：API/组件矩阵 -> threat model -> release gate -> Step 验收标准；实现 Step 必须反向回填证据。
- 兼容性：保持 Step 00-04 的安全红线和当前矩阵状态，不把 demo-only/mock 误标 production-ready。
- 风险控制：所有敏感信息示例必须 mock-only；文档不得包含真实 DID 私钥、token、手机号、地址或文件内容。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/security/threat-model.md`、`docs/runbook/release-gates.md`、API/组件兼容矩阵和 Phase 3 子文档。
2. 为 Skill package、Atomic API VM、Component VM、RequestBroker、Storage、DID/token、ConsentGate、audit、Render IR、Host provider 建立统一资产/威胁/控制表。
3. 为 L0-L4 风险等级补齐能力映射、默认处理、必需测试和 release blocker 规则。
4. 将后续 Step 03-02 至 03-06 的控制目标链接回 threat model，不提前宣称实现完成。
5. 更新 release gates 的 planned security gates，明确哪些 gate 在 Phase 3 内升级为 required。
6. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 收敛资产、攻击者、控制矩阵、风险等级和残余风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步 Phase 3 security gates 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 核对 L3/L4 API risk level | 视发现更新 |
| `anp/anp-miniapp-dock/docs/architecture/component-compatibility-matrix.md` | 核对 Host action / Render IR 风险 | 视发现更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md` | 同步阶段检查状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-threat-model-and-controls.md` | 同步控制矩阵细节 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-01-threat-model-security-classification.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-04、Step 01-08、Step 02-07。
- 外部文档或决策：API/组件兼容矩阵、Release Gates、Phase 3 安全计划。
- 环境前提：文档为主；无需外部 Host provider。

## 7. 验收标准

- [x] 每个 L3/L4 API 和高风险 component action 在 threat model 中有控制措施、owner、测试 gate 和残余风险。
- [x] Release gates 明确区分当前已自动化、Phase 3 必须自动化和 Phase 4/5 后续 gate。
- [x] API/组件矩阵的 risk level 与 threat model 不冲突。
- [x] 文档没有把 demo-only/mock/headless provider 写成 production-ready。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- docs/security docs/runbook docs/architecture docs/plan` | 无空白错误 |
| 风险等级抽样 | `cd anp/anp-miniapp-dock && rg -n "L3|L4|ConsentGate|audit|redaction|fail closed" docs/security docs/runbook docs/architecture docs/plan/production-readiness` | 高风险能力均有控制说明 |
| 敏感信息扫描 | `cd anp/anp-miniapp-dock && rg -n "token|Authorization|signature|private key|phone|address|file content" docs/security docs/runbook docs/architecture docs/plan` | 只命中文档中的脱敏规则、mock 示例或安全红线 |
| 链接检查 | 手工检查新增/修改 Markdown 相对链接 | 链接目标存在 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：文档同步完成后、commit 前。
- Review 重点：安全分类是否可驱动实现；是否遗漏 Host provider/Render IR/Skill package；planned gate 是否被误标通过；敏感信息是否只以脱敏方式出现。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | `docs/security/threat-model.md` 仍把 Render IR schemaVersion/snapshot 和 unsupported registry 作为旧 planned gate 描述，和 Step 01-05、02-01、02-06 的 done 证据不一致。 |
| 已修复问题 | 已修复 | 将 Render IR 状态改为已有 schemaVersion、snapshot gate 和 stable fallback reason；将 unsupported API 静默成功 gate 改为当前 deterministic unsupported registry 证据；保留 Phase 4 Host renderer conformance 作为后续 release blocker。 |
| 剩余风险 | 已记录 | Step 03-02 至 03-06 仍需实现 sandbox/resource、permission engine、DID/token lifecycle、persistent consent/audit 和 package integrity；真实 Host renderer/provider、production transport、registry/cache、secret store 和持久化 backend 仍在 Phase 4/5/6。 |
| 新增或缺失测试 | 文档 Step，无新增 Rust 测试 | 本 Step 冻结分类与 gate 口径；验证使用文档 diff check、风险等级抽样、敏感词抽样和链接检查。后续 03-02 至 03-06 必须补实现测试。 |
| 已更新或缺失文档 | 已更新 | 已更新 `docs/security/threat-model.md`、`docs/runbook/release-gates.md`、Phase 3 总文档、Phase 3 threat model 子文档、主 Plan 和本 Step 文档；API/组件矩阵经抽样无直接冲突，未修改。 |

## 10. Commit 要求

- Commit 时机：验证、Review、文档同步完成后。
- Commit 范围：只包含 threat model/security gate 分类相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`docs: classify production security threats`

执行记录：

- Commit 前状态：`git status --short --branch` 显示仅 `docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-3-security-hardening.md`、`docs/plan/production-readiness/phase-3-threat-model-and-controls.md`、`docs/plan/production-readiness-roadmap.md` 和本 Step 文档变更。
- 纳入文件：上述文件均属于 Step 03-01 threat model、安全分级、release gate 收敛和执行台账。
- Commit 后证据：implementation commit `a61a7e7 docs: classify production security threats`；post-commit `git status --short --branch` = `## main...origin/main [ahead 47]`，工作区无未提交实现变更。
- 遗留未提交变更：仅本 Step 文档和主 Plan 的 commit hash / done 状态回填，准备单独创建 docs closure commit。

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-01 小 Plan | 将 Phase 3 threat model 与安全分级收敛拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 将 02-07 设为 Phase 3 前置 gate | 按 Review 发现，Phase 3 不能只依赖 02-06 的 free-form 下一步文字，必须依赖可追踪 final Review Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：安全分类过宽会阻塞开发，过窄会漏掉 production release blocker。
- 回滚 / 回退：若分类争议无法在本 Step 解决，按更严格的 L3/L4 fail-closed 处理并记录开放问题。
- 后续文档：Step 03-02 至 03-06 必须引用本 Step 冻结的控制矩阵。
