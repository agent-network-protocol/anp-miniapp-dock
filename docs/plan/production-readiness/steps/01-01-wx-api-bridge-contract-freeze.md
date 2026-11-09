# Step 01-01：wx API Bridge Contract 冻结

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 10:53:13 +0800 |
| Completed | 2026-06-12 10:59:18 +0800 |
| Commit | `10db676` |
| Review evidence | 初审未发现需修复的契约问题；确认 callback/Promise、`wx.request` HTTP status、unsupported shape、JS-provided auth header、sync API 和 redaction 均已有冻结行为 |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 9]`；`git diff --check -- docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md docs/architecture/wx-api-compatibility-matrix.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md` 无输出；未决行为检查 `rg -n "或 resolve\\?|待确认|TODO|不确定" docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md docs/architecture/wx-api-compatibility-matrix.md` 无命中；脱敏检查命中 token/authorization/signature/private/credential/phone/address/fileContent 规则；契约抽样命中 HTTP response、4xx/5xx、Authorization、Signature、Promise reject、success/fail/complete、unsupported、provider_unavailable、consent_required、network_denied、invalid_options、WxApiCall、WxApiOutcome；contract 与矩阵 Markdown 链接检查无破链；post-commit `git status --short --branch` = `## main...origin/main [ahead 10]` |
| Next action | 进入 Step 01-02 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` 从建议稿补强为实现前冻结契约，消除 `Promise reject 或 resolve?` 这类未决行为。
- 用户 / 系统可见行为：后续实现 `wx.*` JS bridge 时，有统一入口、统一 outcome、统一 callback/Promise 调用顺序、统一 unsupported/error/redaction 语义。
- 非目标：不实现 bridge；不修改 QuickJS 代码；不新增 API。
- 完成标准：contract 文档明确 `WxApiCall` / `WxApiOutcome`、失败时 Promise 行为、HTTP 非 2xx 行为、callback 调用顺序、redaction、unsupported shape，并同步引用 Step 00-02 API 矩阵。

## 3. 设计方法

- 设计边界：先冻结文档契约，再进入实现；不让每个 API 独立决定 callback/Promise 和错误行为。
- 核心决策：所有异步 API 统一从 JS wrapper 进入 Rust broker；unsupported API 必须是函数并返回 deterministic failure；Host 私有数据不进入 JS result。
- 契约 / API / 数据流：`Skill JS -> JS wrapper -> __dock.wxApi -> Rust WxApiCall -> Capability Broker -> WxApiOutcome -> callback/Promise`。
- 兼容性：与 Step 00-02 矩阵一致；微信语义不确定处在文档中明确本容器行为，后续实现不得再隐式漂移。
- 风险控制：明确 L3/L4 `consent_required`、`permission_denied`、`network_denied`、`provider_unavailable` 等错误码和脱敏规则。

## 4. 实现方法

1. 阅读 Step 00-02 输出的 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`。
2. 更新 `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md`，把建议性字段调整为冻结契约。
3. 明确 Promise 行为：普通 fail/unsupported/permission/consent 默认 reject；`wx.request` 的 HTTP 非 2xx 是否 fail 按冻结决策记录。
4. 明确 `success`、`fail`、`complete` 的同步/异步调用顺序，以及 callback exception 是否影响 Promise。
5. 明确 `WxApiOutcome.audit` 只能是脱敏摘要，不得携带 raw proof/token/private path。
6. 更新主 Plan 执行台账和本 Step 状态。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` | 补强并冻结 bridge contract | 必须 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 读取或同步 contract 链接 | 如需引用，谨慎修改 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 00-02。
- 外部文档或决策：Phase 1 文档、wx API 矩阵、小程序 MCP 本地参考。
- 环境前提：无需运行代码；只修改计划/契约文档。

## 7. 验收标准

- [x] `phase-1-wx-api-bridge-contract.md` 不再保留未决的 `reject(result) 或 resolve?` 文案，必须有冻结行为。
- [x] 文档明确 `wx.request` HTTP status、network error、permission deny、unsupported、consent required 的 callback/Promise 行为。
- [x] `WxApiCall` / `WxApiOutcome` 字段与 Phase 1 broker 计划一致。
- [x] redaction 规则覆盖 token、authorization、signature、secret、private、credential、phone、address、fileContent。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 文档路径检查 | `cd anp/anp-miniapp-dock && git diff --check -- docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md` | 无空白错误 |
| 未决行为检查 | `cd anp/anp-miniapp-dock && rg "或 resolve\\?|待确认|TODO|不确定" docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` | 无未决契约，或每项都有明确决策记录 |
| 脱敏检查 | `cd anp/anp-miniapp-dock && rg "token|authorization|signature|private|credential|phone|address|fileContent" docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` | 敏感字段规则可追踪 |
| 矩阵一致性 | 手工对照 wx API 矩阵 | 分组、错误码、风险等级一致 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：contract 文档完成、验证后、commit 前。
- Review 重点：是否仍有未决行为；是否能指导单一 JS wrapper 实现；错误语义是否稳定；安全/隐私边界是否明确；是否与 API 矩阵冲突。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 无阻塞问题 | contract 已删除未决 `reject(result) 或 resolve?`，并冻结通用失败 Promise reject、`wx.request` HTTP response resolve、broker/local failure reject、unsupported deterministic failure、sync API exception 和 callback exception 处理。 |
| 已修复问题 | 已同步矩阵漂移 | `wx-api-compatibility-matrix.md` 原本仍写着 Step 01-01 待确认；已改为引用冻结契约，并同步 `wx.request` 非 2xx、JS-provided auth header、login/checkSession 和 modelContext callback/Promise 决策。 |
| 剩余风险 | 可接受 | 当前 `js-runtime-quickjs` demo bridge 的 `wx.request` fail Promise 仍按 demo-only 行为 resolve；本 Step 不改代码，已在 contract 和矩阵中明确 Step 01-04 迁移到冻结契约。 |
| 新增或缺失测试 | 未新增自动化测试 | 本 Step 为契约文档冻结；后续 Step 01-03/01-04 实现 JS wrapper 和 RequestBroker 时必须新增 callback/Promise、unsupported、redaction 和 `Authorization` 拒绝测试。 |
| 已更新或缺失文档 | 已更新 | 更新 `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md` 和 `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`；主 Plan 与本 Step 文档已回填 review/verification evidence。 |

## 10. Commit 要求

- Commit 时机：contract 冻结、验证、Review 完成后。
- Commit 范围：只包含 Step 01-01 的契约文档和直接引用更新。
- Commit 前状态：`git status --short --branch` 显示 `docs/architecture/wx-api-compatibility-matrix.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md`、`docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md` 修改。
- 纳入文件：`anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`、`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md`。
- Commit 后证据：主产物 commit `10db676 docs: freeze wx api bridge contract`；post-commit `git status --short --branch` = `## main...origin/main [ahead 10]`。台账关闭状态由后续小文档提交保存。
- 建议消息：`docs: freeze wx api bridge contract`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-01 小 Plan | 将 Phase 1 bridge contract 冻结拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：过早冻结与微信真实语义有偏差。
- 回滚 / 回退：实现中发现不可行时，先更新 Plan 变更记录和 contract，再改代码。
- 后续文档：Step 01-02 至 01-04 必须遵守本 contract。
