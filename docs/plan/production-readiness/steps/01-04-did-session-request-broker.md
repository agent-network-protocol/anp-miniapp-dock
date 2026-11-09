# Step 01-04：DID 会话与 RequestBroker 收敛

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：01-04
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-12 11:47:39 +0800 |
| Completed | 2026-06-12 12:23:36 +0800 |
| Commit | `e9cbdbe` |
| Review evidence | commit 前 Review 已完成：发现并修复 `wx.*` callback exception 会改变原始 Promise outcome、跳过 `complete` 的契约问题；确认 token 只在 Host/runtime 边界，JS auth header fail closed，response auth/token headers 已脱敏；production Host RequestBroker transport 和 persistent audit 按 Phase 4 残余风险记录 |
| Verification evidence | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 15]`；`cargo fmt --check` 通过；`cargo test -p anp-adapter session` 8 passed；`cargo test -p js-runtime-quickjs wx_login` 3 passed；`cargo test -p js-runtime-quickjs check_session` 2 passed；`cargo test -p js-runtime-quickjs wx_request` 3 passed；`cargo test -p js-runtime-quickjs wx_callback_exception` 1 passed；`cargo test -p js-runtime-quickjs model_context_expire_all_cards` 4 passed；`cargo test -p js-runtime-quickjs` 28 passed；`cargo test -p anp-adapter` 41 passed；`cargo test -p wx-compat` 9 passed；`cargo test -p demo-server token` 4 passed under filter；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/js-runtime-quickjs crates/wx-compat crates/dock-core crates/demo-server crates/dock-cli examples/coffee-fastapi-server docs/architecture docs/runbook docs/security docs/plan` 无输出 |
| Next action | 返回主 Plan 检查是否触发最终全局 Review |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：把当前 demo 中分散的 DID 登录、token cache 和 `wx.request` Authorization 注入收敛为正式 `DidAuthSessionManager` / RequestBroker 路径。
- 用户 / 系统可见行为：`wx.login` 返回 code-like receipt，`wx.checkSession` 校验 session/token 状态，`wx.request` 走 allowlist、permission、scope、DID auth、redaction；Skill JS 不接触 raw token、DID proof、Authorization header。
- 非目标：不实现完整 secret store；不实现生产支付/隐私 provider；不实现长期持久化 token store。
- 完成标准：coffee demo 不依赖散落 demo bridge；demo-server 与 FastAPI 示例共享或对齐 challenge/login JSON contract；token 不出现在 JS result、CLI output、日志和 audit 中。

## 3. 设计方法

- 设计边界：Skill JS 只看到微信兼容登录/请求结果；DID credential provider、challenge proof、capability token 留在 host/runtime 边界。
- 核心决策：Session key 使用 `serverBaseUrl + merchantDid + userDid + agentDid? + skillId + sessionId`；Authorization 由 host/runtime 注入，JS 提供 Authorization 必须剥离或拒绝。
- 契约 / API / 数据流：`wx.login -> DidAuthSessionManager.ensure_session -> challenge -> proof -> login -> token cache -> receipt`；`wx.request -> normalize -> allowlist -> ensure_session -> attach auth -> transport -> redact -> WxApiOutcome`。
- 兼容性：保留 coffee flow；失败路径按 Step 01-01 bridge contract 返回 `errMsg` 和 redacted safe message。
- 风险控制：replay challenge、wrong audience、missing scope、expired token、JS Authorization override、non-allowlist host 必须失败。

## 4. 实现方法

1. 阅读 `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-1-did-request-session-manager.md` 和 Step 01-01 contract。
2. 阅读 `anp/anp-miniapp-dock/crates/anp-adapter` 中 credential provider、HTTP signature、challenge proof、token cache、allowlist request broker。
3. 阅读 `anp/anp-miniapp-dock/crates/js-runtime-quickjs` 中当前 demo `wx.login` / `wx.request` 注入路径。
4. 阅读 `anp/anp-miniapp-dock/crates/demo-server` 和 `anp/anp-miniapp-dock/examples/coffee-fastapi-server` challenge/login/business API contract。
5. 设计并实现 `DidAuthSessionManager` public API 或 crate 内接口，收敛 token cache、checkSession、refresh/clear 策略。
6. 将 `wx.login`、`wx.checkSession`、`wx.request` 接入正式 broker，移除或隔离散落 demo bridge。
7. 增加 tests：首次 login、重复 login、expired token、wrong DID/audience、replay challenge、missing scope、JS Authorization override、redaction。
8. 更新 API 矩阵、release gates、security/threat model 或 runbook 中相关状态。
9. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/anp-adapter` | `DidAuthSessionManager`、token/cache/request auth | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | `wx.login`、`wx.checkSession`、`wx.request` bridge 接入 | 代码实现 |
| `anp/anp-miniapp-dock/crates/wx-compat` | RequestBroker trait / outcome / unsupported shape | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-core` | Orchestrator permission/consent/audit 接入 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/demo-server` | challenge/login contract 对齐和 tests | 视当前结构修改 |
| `anp/anp-miniapp-dock/examples/coffee-fastapi-server` | FastAPI contract 对齐 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/dock-cli` | run-demo / output redaction 回归 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/architecture/wx-api-compatibility-matrix.md` | 同步状态 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | 同步安全 gate | 视实现影响 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 同步 demo credential/options | 视实现影响 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/01-04-did-session-request-broker.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 01-01、Step 01-02。
- 外部文档或决策：DID Request Session Manager 子文档、wx API bridge contract、API 矩阵、security runbook。
- 环境前提：Rust toolchain 1.88.0；FastAPI flow 需要 Python 环境，若不可用需记录并至少运行 Rust demo-server tests。

## 7. 验收标准

- [x] `DidAuthSessionManager` 有清晰 public API 或 crate 内接口，session key 隔离多 merchant、多 user、多 skill。
- [x] `wx.login` 返回 code-like receipt，不暴露 raw token/proof。
- [x] `wx.checkSession` 校验 token 存在、过期和 revocation/clear 状态。
- [x] `wx.request` 通过 `wx-compat::RequestBroker` trait 的本地 DID broker，不再把 token cache 和 header 注入散落在 JS-visible bridge 中；production Host transport 仍按 Phase 4 记录。
- [x] JS 提供 Authorization 被剥离或拒绝；非 allowlist host 不出站。
- [x] replay challenge、wrong audience、missing scope、expired token 失败路径有测试。
- [x] CLI/log/audit/JS result 不含 capability token、Authorization、HTTP signature、private key path。
- [x] API 矩阵、release gates/runbook 与实现状态同步。
- [x] Review 发现已修复或明确记录。
- [x] 本步骤在进入下一步之前已创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Focused auth/request tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter login && cargo test -p js-runtime-quickjs request && cargo test -p demo-server token` | 相关测试通过；若 filter 不匹配，记录实际命令 |
| Coffee E2E | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过且输出脱敏 |
| Workspace regression | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过；如耗时或环境受限，记录原因和 focused 替代 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/anp-adapter crates/js-runtime-quickjs crates/wx-compat crates/dock-core crates/demo-server crates/dock-cli examples/coffee-fastapi-server docs/architecture docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查测试输出、CLI JSON、audit/debug 输出 | 无 raw token、Authorization、signature、private key path |

如果某个命令不能运行，必须记录原因、影响和替代证据。

已执行验证：

- `cargo fmt --check`：通过。
- `cargo test -p anp-adapter session`：通过，8 个相关测试。
- `cargo test -p js-runtime-quickjs wx_login`：通过，bridge + VM login 相关 3 个测试。
- `cargo test -p js-runtime-quickjs check_session`：通过，2 个测试。
- `cargo test -p js-runtime-quickjs wx_request`：通过，3 个测试，覆盖 JS auth header fail closed、非 loopback `network_denied` 和 response header redaction。
- `cargo test -p js-runtime-quickjs wx_callback_exception`：通过，1 个测试，覆盖 callback exception 不改变原始 outcome 且继续调用 `complete`。
- `cargo test -p js-runtime-quickjs model_context_expire_all_cards`：通过，4 个回归测试，确认共用 async wrapper 未破坏 Step 01-03。
- `cargo test -p js-runtime-quickjs`：通过，28 个测试。
- `cargo test -p anp-adapter`：通过，41 个测试，覆盖 challenge proof、wrong DID、wrong audience/payload mismatch、expired proof、token scope/audience、session refresh/clear 和 redaction。
- `cargo test -p wx-compat`：通过，9 个测试。
- `cargo test -p demo-server token`：通过，4 个相关 filtered tests；完整 workspace test 同时覆盖 `demo_signature_and_replayed_challenge_are_rejected`、`login_scope_mismatch_is_rejected`、expired token 和 audit redaction。
- `cargo test -p dock-cli --test coffee_order_flow`：通过，3 个测试。
- `cargo test --workspace`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `git diff --check -- crates/anp-adapter crates/js-runtime-quickjs crates/wx-compat crates/dock-core crates/demo-server crates/dock-cli examples/coffee-fastapi-server docs/architecture docs/runbook docs/security docs/plan`：无输出。
- 脱敏抽样：新增 VM 测试确认 `wx.login` JS result 不含 `cached-token`；`wx.request` response 不暴露 `Authorization`、`Set-Cookie`、`Signature`、`Signature-Input`、token-like header 或 bearer token；coffee E2E 和 demo-server audit 测试继续覆盖 CLI/audit redaction。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：token 是否只在 host/runtime 边界；session key 是否隔离；retry 是否只在安全场景；非幂等请求是否避免危险重试；redaction 是否集中且覆盖失败路径；demo-server/FastAPI contract 是否一致。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已处理 | Review 发现 `__dockAsyncOutcome` 中 callback exception 会改变 Promise outcome，并可能跳过 `complete`，不符合 Step 01-01 冻结契约；同时确认 `https://merchant.example` 应归为 `network_denied` 而不是 `invalid_options`。 |
| 已修复问题 | 已修复 | 新增 `__dockInvokeCallback`，callback 抛错只记录 redacted warning，原始 Host outcome 仍 resolve/reject，`complete` 继续执行；`expireAllCards` 复用统一 wrapper；URL 解析先识别 http/https，再由 network gate 拒绝非 loopback。 |
| 剩余风险 | 已记录 | Atomic API `wx.request` 已走 `wx-compat::RequestBroker` trait 的本地 DID broker，但底层仍是 loopback demo TCP transport；production Host transport、registry allowlist、persistent request audit、logout/revocation list 和跨进程 session store 留到 Phase 3/4。FastAPI 示例未改代码，当前对齐通过共享 challenge/login JSON contract 和 Rust demo-server tests 证明。 |
| 新增或缺失测试 | 已新增 | 新增 `DidAuthSessionManager` key 隔离、expired refresh、clear/revoke、debug redaction 测试；新增 `wx.login` receipt、`wx.checkSession` success/fail、JS auth header rejection、非 loopback deny、response header redaction、callback exception outcome 测试；未新增 production Host transport E2E，原因是 Phase 4 范围。 |
| 已更新或缺失文档 | 已更新 | 已更新 API 矩阵、DID Request Session Manager 子文档、release gates、local demo runbook、threat model、主 roadmap 和本 Step 文档；保留 demo-only/host-boundary 残余风险，未把 loopback transport 标成 production-ready。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 DID session/request broker 收敛相关代码、tests 和直接文档更新。
- Commit 前状态：`git status --short` 包含 `crates/anp-adapter/src/lib.rs`、`crates/anp-adapter/src/session.rs`、`crates/anp-adapter/src/signed_request.rs`、`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/bridge.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`crates/wx-compat/src/request.rs`、`docs/architecture/wx-api-compatibility-matrix.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-1-did-request-session-manager.md`、`docs/plan/production-readiness/steps/01-04-did-session-request-broker.md`、`docs/runbook/local-demo.md`、`docs/runbook/release-gates.md`、`docs/security/threat-model.md`。
- Commit 后证据：`e9cbdbe`；`git status --short --branch` = `## main...origin/main [ahead 16]`。
- 建议消息：`phase1: add did session request broker`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 01-04 小 Plan | 将 Phase 1 DID/session/request 收敛拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：认证与请求路径影响面大，容易改变 coffee demo 行为或泄露调试信息。
- 回滚 / 回退：保持旧 demo path 可在 dev-only feature/flag 下临时隔离，生产默认必须走 broker；任何回退都要记录为 production warning。
- 后续文档：Phase 3 token lifecycle、audit persistence、secret store 将在此基础上加固。
