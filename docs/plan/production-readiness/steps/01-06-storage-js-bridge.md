# Step 01-06：Storage JS Bridge

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-06
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 20:22:41 +0800 |
| Completed | 2026-06-12 20:43:49 +0800 |
| Commit | `1599294` |
| Review evidence | 本文 Review 环节已记录：未发现阻塞问题；确认 scope 使用 `userDid + merchantDid + skillId` 且不含 `sessionId`，sync/async 语义与 Step 01-01 契约一致，storage 内容不自动进入 model-visible result，未引入生产持久化承诺。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p wx-compat storage` 8 passed；`cargo test -p js-runtime-quickjs storage` 6 passed；`cargo test -p js-runtime-quickjs wx_` 20 passed；`cargo test -p js-runtime-quickjs` 39 passed；`cargo test -p wx-compat` 16 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/runbook docs/security docs/plan` 无输出；`cargo clippy --workspace --all-targets -- -D warnings` 通过；敏感词抽样仅命中文档规则、测试假值和 redaction 断言。 |
| Next action | 进入 Step 01-07 Device/App Info Atomic API |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把已有 Rust scoped storage 基础能力暴露为 `wx.getStorage` / `wx.setStorage` / `wx.removeStorage` / `wx.clearStorage` 和同步版本。
- 用户 / 系统可见行为：Skill 可在 `userDid + merchantDid + skillId` scope 内读写小型 JSON-safe 数据；不同 DID、merchant 和 Skill 之间不可见。
- 非目标：不实现生产持久化、加密存储迁移、batch storage 或完整微信本地缓存配额体系。
- 完成标准：异步 / 同步 storage API 语义符合 Step 01-01 契约，错误 fail closed，storage 内容不自动进入模型可见结果。

## 3. 设计方法

- 设计边界：storage 是 Skill-scoped runtime state，不是 session token store，不保存 DID 私钥、capability token、Authorization 或隐私原文。
- 核心决策：scope 固定为 `userDid + merchantDid + skillId`，不把 `sessionId` 纳入长期隔离维度；key/value 需要 size limit、JSON-safe 校验和敏感 key 检查。
- 契约 / API / 数据流：JS wrapper normalize options -> StorageBroker -> scoped storage trait -> `WxApiOutcome` 或 sync return / throw。
- 兼容性：异步 API 同时支持 callback 与 Promise；同步 API 不接受 callback，不返回 Promise，失败抛出脱敏 Error。
- 风险控制：空 key、NUL、超限 key/value、非 JSON-safe value、quota exceeded、敏感 key 默认 fail closed 或 redacted diagnostic。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/crates/wx-compat/src/storage.rs` 和 `anp/anp-miniapp-dock/crates/wx-compat/tests/scoped_storage.rs`。
2. 阅读 Step 01-01 bridge contract 中 storage sync / async 语义和错误 shape。
3. 在 `wx-compat` 补齐 storage trait / helper 能力，至少覆盖 clear、key/value 校验、scope 构造、quota / size limit。
4. 在 `js-runtime-quickjs` 注入异步和同步 storage API，复用统一 callback / Promise wrapper。
5. 增加 tests：set/get/remove/clear、sync throw、async callbacks、scope 隔离、空 key、超限 value、敏感 key redaction、model-visible 隔离。
6. 更新 API 矩阵、release gates 或 threat model 中 storage 状态。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/wx-compat` | storage broker、scope、validation、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | storage JS API 注入和 VM tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | 如需向 Atomic API context 传递 storage scope | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步 storage API 状态和证据 | 必须 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 storage 残余风险 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步 storage gate | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-06-storage-js-bridge.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-01、Step 01-04、Step 01-05。
- 外部文档或决策：wx API Bridge Contract、wx API 兼容矩阵、Threat Model。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host provider。

## 7. 验收标准

- [x] `wx.getStorage` / `setStorage` / `removeStorage` / `clearStorage` 支持 callback + Promise，成功和失败 shape 稳定。
- [x] `wx.getStorageSync` / `setStorageSync` / `removeStorageSync` / `clearStorageSync` 成功直接返回或返回 `undefined`，失败抛出脱敏 Error。
- [x] storage scope 使用 `userDid + merchantDid + skillId`，不同 scope 数据不可见。
- [x] 空 key、NUL、超限 key/value、非 JSON-safe value 和 quota 问题 fail closed。
- [x] storage value 不自动进入模型可见输出、日志、CLI JSON、Render IR 或 audit export。
- [x] API 矩阵和安全文档与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Focused storage tests | `cd anp/anp-miniapp-dock && cargo test -p wx-compat storage` | 8 passed，覆盖 scoped storage、validation、clear 和 quota rollback |
| VM tests | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs storage` | 6 passed，覆盖 async / sync bridge、scope 隔离、JSON-safe 校验和 model-visible 隔离 |
| Coffee 回归 | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 3 passed |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/runbook docs/security docs/plan` | 无输出 |
| 脱敏抽样 | 手工检查 storage error、audit/debug 和 CLI 输出 | 敏感词扫描仅命中文档规则、测试假值和 redaction 断言；未发现真实 secret、真实 token、private key path 或隐私原文 |

补充回归：`cargo test -p js-runtime-quickjs wx_` 20 passed；`cargo test -p js-runtime-quickjs` 39 passed；`cargo test -p wx-compat` 16 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过。

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：scope 是否正确；sync/async 语义是否与契约一致；size/quota 是否 fail closed；storage 是否误入模型可见结果；是否引入生产持久化承诺。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 未发现阻塞问题 | Review 覆盖 scope、sync/async shape、fail closed、model-visible 隔离和生产持久化边界。 |
| 已修复问题 | 修复 storage focused test 只跑 0 个 VM 测试的问题；修复旧 unsupported sync storage 断言；修复 quota 测试实际命中 value limit 而非 scope quota 的问题；修复 sync storage fail `errMsg` 使用异步 API 名称的问题；补充 `merchantDid` 必填，避免 demo fallback 混淆 storage scope。 | 修复均已由 focused tests 和全量 crate tests 覆盖。 |
| 剩余风险 | 当前 storage backend 为 runtime-local in-memory；生产持久化、加密、migration、backend quota、cleanup 和 retention 仍按 Phase 4 处理。组件环境 storage provider 仍待 Phase 2/4，不在本 Step 范围。 | 文档已在 API 矩阵、threat model、release gates 和 Phase 1 子文档中记录。 |
| 新增或缺失测试 | 新增 `wx-compat` storage validation/clear/quota tests 和 `js-runtime-quickjs` async/sync/scope/JSON-safe/model-visible tests；未新增生产持久化测试。 | 持久化测试不属于 01-06。 |
| 已更新或缺失文档 | 已更新 `docs/architecture/wx-api-compatibility-matrix.md`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、Phase 1 contract/broker 文档和本 Step 文档。 | 未更新 Phase 4 详细持久化 Step，因为该范围已由 04-06 承接。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 storage bridge、直接 tests 和相关文档。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 24]`，包含本 Step storage bridge、直接 tests 和相关文档。
- 纳入文件：`crates/wx-compat/src/storage.rs`、`crates/wx-compat/src/lib.rs`、`crates/wx-compat/src/unsupported.rs`、`crates/wx-compat/tests/scoped_storage.rs`、`crates/wx-compat/tests/component_permissions.rs`、`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/bridge.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`docs/architecture/wx-api-compatibility-matrix.md`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md`、`docs/plan/production-readiness/phase-1-wx-capability-broker.md`、`docs/plan/production-readiness/steps/01-06-storage-js-bridge.md`、`docs/plan/production-readiness-roadmap.md`。
- Commit 后证据：实现提交 `1599294 phase1: add storage js bridge`；提交后 `git status --short --branch` = `## main...origin/main [ahead 25]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase1: add storage js bridge`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-06 小 Plan | 将 Phase 1 storage JS bridge 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：storage 语义若绑定 session，会导致用户长期状态丢失或 scope 混淆。
- 回滚 / 回退：保留 Rust storage 基础能力；如 JS bridge 出现问题，可先 registry fail closed 并保留矩阵为 `host-boundary`。
- 后续文档：Phase 3/4 需要补生产持久化、加密、migration、quota 和 retention 策略。
