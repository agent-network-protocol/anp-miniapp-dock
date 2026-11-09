# Step 03-05：Consent Adapter 与持久化 Audit Sink

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-05
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 15:11:49 +0800 |
| Completed | 2026-06-13 15:34:39 +0800 |
| Commit | `e7c9f49` |
| Review evidence | 2026-06-13 15:32:32 +0800 commit 前 Review 已记录：修复 `FileAuditSink` export 只信任已持久化 redacted record、可能导出 legacy/raw JSONL 的问题；补充 Host adapter denied fail-closed audit 测试；确认 ConsentGate 在 executor/provider 前、provider unavailable/denied fail closed 且可审计、dev/headless mock 有显式 provider/actor、JSONL audit record/export 默认脱敏 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p consent-audit consent` 3 unit + 3 integration passed；`cargo test -p consent-audit audit` 2 unit + 4 integration passed；`cargo test -p dock-core consent` 9 passed；`cargo test -p consent-audit` 5 unit + 7 integration passed；`cargo test -p dock-core` 15 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/consent-audit crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假值、文档安全说明、redaction 断言、`AuthMode::HttpSignatures` 常量和 demo-only secret placeholder，未发现真实 secret/token/proof/private key path 输出 |
| Next action | 进入 03-06 Skill 包完整性与供应链 Gate |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把 mock consent provider 抽象为 Host consent adapter，并提供至少一个生产候选 persistent audit sink。
- 用户 / 系统可见行为：L3/L4 操作没有 consent proof 不会执行；audit 可落盘、查询、retention，并默认脱敏导出。
- 非目标：不实现具体 Mac/Flutter 真实 UI；不存储隐私原文作为默认策略。
- 完成标准：ConsentProof 含 policy version、prompt digest、decision actor、timestamp、parameter digest；audit sink 有持久化 tests 和 redaction regression。

## 3. 设计方法

- 设计边界：ConsentGate 在 provider/executor 前执行；audit 在成功、失败、拒绝、provider unavailable 等路径都可记录脱敏摘要。
- 核心决策：Host consent adapter trait 分离 CLI/headless/mock/真实 Host；mock 必须 dev-only；persistent audit 默认只存 redacted summary。
- 契约 / API / 数据流：RiskPolicy -> ConsentRequest -> Host adapter -> ConsentProof -> executor/provider -> AuditRecord -> persistent sink/export。
- 兼容性：保留现有 `consent-audit` tests 和 dock-core enforcement order；新增持久化 backend 不改变模型可见输出。
- 风险控制：Authorization、Signature、token、private key、手机号、地址、文件内容、精确位置不落盘或必须加密/脱敏。

## 4. 实现方法

1. 阅读 `consent-audit`、`dock-core` consent flow、Threat Model 和 Release Gates。
2. 定义 Host consent adapter trait、ConsentRequest、ConsentProof 字段和 stable deny/required error shape。
3. 实现 persistent audit sink 候选：SQLite 或 append-only 文件；如暂选 trait + file backend，必须记录生产限制。
4. 增加 audit query/export 默认 redacted 行为和 retention policy 文档。
5. 增加 tests：无 consent 不执行 provider、denied fail closed、proof 字段完整、audit persistence、redaction regression、export redacted。
6. 更新 Threat Model、Release Gates、local demo runbook 和 Phase 3 文档。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/consent-audit` | Host consent adapter、ConsentProof、persistent audit sink、redaction tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-core` | Orchestrator consent/audit enforcement order | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | audit query/export 或 redaction 回归 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 consent/audit 控制 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | persistent audit/redaction gate | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 headless/mock consent 使用方式 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md` | 同步 consent/audit 完成状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-05-consent-adapter-persistent-audit.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-08、Step 03-01、Step 03-03。
- 外部文档或决策：Threat Model、Release Gates、Host provider boundary。
- 环境前提：Rust toolchain 1.88.0；生产 Host UI 可后置到 Phase 4 Host adapter。

## 7. 验收标准

- [x] Host consent adapter trait 支持 CLI/headless/mock 与未来 Host UI 分离。
- [x] ConsentProof 包含 policy version、prompt digest、decision actor、timestamp、parameter digest。
- [x] 无 consent、denied、provider unavailable 均 fail closed 且可审计。
- [x] persistent audit sink 至少有一个生产候选实现或明确 host-boundary，且有 restart/query tests。
- [x] audit query/export 默认脱敏，redaction regression 覆盖 token/signature/private/phone/address/file content。
- [x] Threat Model、Release Gates 和 runbook 与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Consent tests | `cd anp/anp-miniapp-dock && cargo test -p consent-audit consent` | proof/adapter tests 通过 |
| Audit tests | `cd anp/anp-miniapp-dock && cargo test -p consent-audit audit` | persistence/export/redaction tests 通过 |
| Core enforcement | `cd anp/anp-miniapp-dock && cargo test -p dock-core consent` | provider 前 enforcement tests 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/consent-audit crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 audit DB/file/export/CLI JSON | 不含 raw token、Authorization、signature、private key path、手机号、地址、文件内容 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：ConsentGate 是否在执行前；mock 是否 dev-only；audit sink 是否默认脱敏；retention/export 是否不会泄露隐私。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 有 | 初版 `FileAuditSink::export_redacted_json` 依赖查询出的记录已被脱敏，若读取 legacy/raw JSONL 或手工写入文件，export 仍可能输出 raw `parameterSummary`；Host adapter denied 路径缺少明确 focused test。 |
| 已修复问题 | 已修复 | export 前再次调用 `AuditRecord::redacted()`；retention rewrite 也写入 redacted record；新增 `file_audit_export_redacts_legacy_raw_records` 和 `host_consent_adapter_denial_fails_closed_with_audit` 回归测试。 |
| 剩余风险 | 已记录 | `FileAuditSink` 是 append-only JSONL 生产候选后端，默认脱敏并可 restart/query/export/retention；部署级 encryption、access control、backend config、migration、privacy deletion 和真实 Host consent UI/conformance 由 Phase 4/6 承接。 |
| 新增或缺失测试 | 已补齐 | 新增 Host consent adapter provider/actor/unavailable tests、ConsentProof policy/prompt/actor/timestamp/digest assertions、provider unavailable audit、denied audit、JSONL restart/query/export/retention、legacy raw export redaction、token/signature/private/phone/address/file content redaction；未新增真实 Host UI tests，按非目标记录。 |
| 已更新或缺失文档 | 已更新 | 已同步 Threat Model、Release Gates、local demo runbook、Phase 3 security hardening、Phase 3 threat model summary、主 Plan 和本 Step 文档；兼容矩阵未改，因为本 Step 不新增 `wx.*` 或 component API 状态。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 consent adapter、persistent audit sink、直接 tests 和相关文档。
- Commit 前状态：`git status --short` 只包含 03-05 代码、测试和文档变更。
- 纳入文件：`crates/consent-audit/src/audit.rs`、`crates/consent-audit/src/consent.rs`、`crates/consent-audit/src/lib.rs`、`crates/consent-audit/tests/payment_requires_consent.rs`、`crates/dock-core/src/host.rs`、`crates/dock-core/src/lib.rs`、`crates/dock-core/src/orchestrator.rs`、`crates/dock-core/tests/api_call_flow.rs`、`crates/dock-cli/src/commands.rs`、`docs/security/threat-model.md`、`docs/runbook/release-gates.md`、`docs/runbook/local-demo.md`、`docs/plan/production-readiness/phase-3-security-hardening.md`、`docs/plan/production-readiness/phase-3-threat-model-and-controls.md`、`docs/plan/production-readiness-roadmap.md`、本 Step 文档。
- Commit 后证据：`e7c9f49`；post-commit `git status --short --branch` = `## main...origin/main [ahead 55]`。
- 遗留未提交变更：无；closure 文档回填将作为单独提交记录。
- 建议消息：`phase3: add consent adapter audit sink`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-05 小 Plan | 将 Consent Adapter 与持久化 Audit Sink 拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：audit 持久化若保存隐私原文，会扩大泄露影响面。
- 回滚 / 回退：默认只保存 redacted summary；需要原文时必须另行设计加密、访问控制和审批。
- 后续文档：Phase 4 persistence config 和 Phase 6 runbook 必须承接 audit backend、retention 和故障处理。
