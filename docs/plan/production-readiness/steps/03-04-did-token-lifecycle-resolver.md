# Step 03-04：DID / Token 生命周期与 Resolver 信任锚

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：03-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 14:49:53 +0800 |
| Completed | 2026-06-13 15:11:49 +0800 |
| Commit | `24a8f10` |
| Review evidence | 2026-06-13 15:10:32 +0800 commit 前 Review 已记录：修复 `TrustedDidDocumentResolver` 仅校验 DID document `id`、未校验完整 trust anchor 内容的问题；确认 token 仍只在 Host/runtime 边界，`verify()` 兼容普通 JWT 校验，新增 lifecycle API 显式处理 revoke / high-risk `ConsumeOnce` jti gate，challenge 登录尝试即消费且 resolver/cache/replay failure 均 fail closed |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p anp-adapter token` 18 passed；`cargo test -p anp-adapter session` 10 passed；`cargo test -p anp-adapter challenge` 15 unit + 1 integration passed；`cargo test -p anp-adapter` 44 unit + 11 integration passed；`cargo test -p demo-server token` 5 unit + 1 integration passed；`cargo test -p demo-server` 7 lib + 4 main + 6 integration passed；`cargo test -p demo-server demo_signature_and_replayed_challenge_are_rejected` 1 passed；`cargo test -p js-runtime-quickjs wx_login` 3 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/demo-server crates/js-runtime-quickjs docs/security docs/runbook docs/plan` 无输出；敏感信息抽样命中测试假值、文档安全说明、redaction 断言和 `AuthMode::HttpSignatures` 常量，未发现真实 token/proof/Authorization/private key path 输出 |
| Next action | 进入 03-05 Consent Adapter 与持久化 Audit Sink |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：生产化 DID challenge、capability token lifecycle、revoke/logout、jti replay 防护和 DID resolver trust anchor。
- 用户 / 系统可见行为：token refresh/revoke/logout 不泄露 token；wrong DID/audience/scope/expired/replay/resolver mismatch 均 fail closed。
- 非目标：不实现完整企业 secret store；不要求连接生产 DID network，resolver 可用 mock/trusted fixture 证明 contract。
- 完成标准：token claims version、session lifecycle、resolver cache TTL、trust anchor 和 replay store 有实现或明确 host-boundary，并有 tests。

## 3. 设计方法

- 设计边界：DID private key、raw token、proof 和 Authorization 只在 Host/runtime 边界，不进入 JS、日志、CLI JSON、Render IR 或 audit export。
- 核心决策：token claims 固定 issuer、audience、merchantDid、userDid、agentDid、skillId、sessionId、scopes、iat/nbf/exp、jti、version；revoke/logout 必须可清理 session。
- 契约 / API / 数据流：challenge -> proof -> login -> token cache -> request broker -> refresh/revoke/logout -> audit redacted summary。
- 兼容性：保留 Step 01-04 的 `DidAuthSessionManager` 行为，增强生命周期和 resolver 信任策略。
- 风险控制：nonce 一次性、TTL、method/url/audience binding；resolver failure policy 默认 fail closed。

## 4. 实现方法

1. 阅读 `anp-adapter` challenge、session、token、signed_request 和 demo-server token validation。
2. 冻结 token claims version 和 scope derivation source 记录。
3. 实现或补齐 refresh、revoke、logout、cache eviction、jti replay store 和 challenge nonce 一次性。
4. 定义 DID resolver cache、TTL、trust anchor 和 network failure policy；无生产 resolver 时提供 trait + conformance tests。
5. 增加 tests：wrong DID、wrong audience、wrong scope、expired token、replay challenge、jti replay、resolver mismatch、token redaction、session clear。
6. 更新 Threat Model、Release Gates、local demo runbook 和 Phase 3 文档。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/anp-adapter` | token lifecycle、resolver trait/cache、replay store、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/demo-server` | server-side token validation/replay tests | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | `wx.login` / `wx.checkSession` lifecycle 回归 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 DID/token 控制与残余风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | DID/token replay/scope gate | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 credential/session 配置说明 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-3-security-hardening.md` | 同步 token lifecycle 状态 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/03-04-did-token-lifecycle-resolver.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-04、Step 03-01、Step 03-03。
- 外部文档或决策：Threat Model、DID Request Session Manager、Release Gates。
- 环境前提：Rust toolchain 1.88.0；真实 secret store 可在 Phase 4 持久化中落地。

## 7. 验收标准

- [x] token claims version 和 scope derivation 来源稳定记录。
- [x] refresh、revoke/logout、cache eviction 和 expired token 行为有测试。
- [x] challenge nonce 一次性、TTL、audience、method/url、DID document binding 有测试。
- [x] DID resolver cache、TTL、trust anchor 和 failure policy 有 trait / tests / 文档证据。
- [x] raw token、proof、Authorization、private key path 不进入 JS result、CLI JSON、日志、audit export 或 Render IR。
- [x] Threat Model、Release Gates 和 runbook 与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Token/session tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter session` | lifecycle / scope / redaction tests 通过 |
| Challenge/replay tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter challenge` | nonce / TTL / audience tests 通过 |
| Demo-server tests | `cd anp/anp-miniapp-dock && cargo test -p demo-server token` | server validation tests 通过 |
| VM auth回归 | `cd anp/anp-miniapp-dock && cargo test -p js-runtime-quickjs wx_login` | login/checkSession 回归通过；若 filter 不匹配，记录实际命令 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/anp-adapter crates/demo-server crates/js-runtime-quickjs docs/security docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 Debug、error、audit、CLI 输出 | 不含 raw token、proof、Authorization、signature、private key path |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：token 是否只在 Host/runtime 边界；revoke/logout 是否完整；resolver failure 是否 fail closed；replay store 是否覆盖 jti 和 challenge nonce。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 有 | 初版 `TrustedDidDocumentResolver` 只检查上游文档和 trust anchor 的 DID `id`，同 DID 但内容漂移的文档可能被接受，trust anchor 语义偏弱。 |
| 已修复问题 | 已修复 | 改为完整 DID document 必须与 trust anchor 匹配，并新增 `trusted_resolver_rejects_document_drift_for_same_did` 回归测试。 |
| 剩余风险 | 已记录 | 当前 lifecycle/replay/resolver store 是本地内存 gate；跨进程 revocation/replay 恢复、生产 DID network/rotation、secret store 和 token cache 持久化由 Phase 4/6 承接。 |
| 新增或缺失测试 | 已补齐 | 新增 token version/scope derivation、revoked token、jti `ConsumeOnce` replay、lifecycle prune、session logout/expired eviction、challenge nonce replay/prune、resolver trust anchor/cache TTL/unknown/mismatch/document drift、demo-server revoked/replayed token 和 failed-login challenge consumption tests；未新增生产 DID network tests，按非目标记录。 |
| 已更新或缺失文档 | 已更新 | 已同步 Threat Model、Release Gates、local demo runbook、Phase 3 security hardening、Phase 3 threat model summary、主 Plan 和本 Step 文档。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 DID/token lifecycle、resolver/replay、直接 tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase3: harden did token lifecycle`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 03-04 小 Plan | 将 DID/token 生命周期与 Resolver 信任锚拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：revoke/logout 或 resolver failure 策略不清会造成长期 token 误用。
- 回滚 / 回退：任何 resolver 不可信或 replay store 不可用时 fail closed；production profile 不允许 silent fallback。
- 后续文档：Phase 4 持久化和 secret store 必须承接本 Step 的 token/session contract。
