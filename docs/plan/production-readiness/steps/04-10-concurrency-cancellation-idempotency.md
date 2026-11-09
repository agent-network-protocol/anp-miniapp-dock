# Step 04-10：并发、取消、重试与幂等策略

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-10
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 21:54:25 +0800 |
| Completed | 2026-06-13 22:28:04 +0800 |
| Commit | `932e2e5` |
| Review evidence | 2026-06-13 22:22:30 +0800 commit 前 Review 进行中：已发现并修复公开 policy 与实现不一致的问题，将 `requiredForHighRisk` 改为 `false`，避免把兼容期未强制的 idempotency key 误声明为 required；已补 session close 清理本地 replay cache；已补不同 session 高风险并发与低风险同 session 并发测试。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 80]`，工作区无未提交变更；已读取主 Plan、Step 04-10 文档、Phase 4 并发/取消/重试与幂等章节、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-09 closure evidence；`cargo fmt --check` 通过；`cargo test -p dock-core concurrency` 初次在测试重命名前只命中 1 个 policy test，已重跑后覆盖 5 passed；`cargo test -p anp-adapter retry` 1 passed；`cargo test -p component-runtime cleanup` 1 passed；`cargo test -p dock-core` 43 passed；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`cargo clippy -p dock-core --all-targets -- -D warnings` 通过；`cargo clippy -p anp-adapter --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/anp-adapter crates/component-runtime crates/consent-audit crates/dock-cli docs/security docs/runbook docs/plan docs/architecture` 无输出；敏感词抽样仅命中源码 redaction 逻辑、测试假值和安全/计划文档条目，未发现真实 token、Authorization、signature、private key material、本机私有路径或生产凭据泄露。 |
| Next action | 进入 04-11 Phase 4 最终 Review 与整体验证。 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：为多 session、多 API 并发、高风险交易串行化、cancellation token、timeout、retry policy 和 idempotency key 建立 Runtime 策略。
- 用户 / 系统可见行为：取消后不会继续执行高风险 action；并发不会串 session/token/storage；非幂等支付/下单默认不自动重试。
- 非目标：不实现分布式事务；不保证跨 Host 的全局锁。
- 完成标准：Runtime API 和 RequestBroker 有并发隔离、取消、超时、重试和幂等 tests；公开 policy 不把兼容期未强制的 idempotency key 误写成 required。

## 3. 设计方法

- 设计边界：并发策略保护用户交易与 session 隔离；不能为了吞吐绕过 consent、audit 或 idempotency。
- 核心决策：同一 session 可并发普通 API；同一高风险 transaction 按 policy 串行；支付/下单等非幂等请求默认不自动 retry。
- 契约 / API / 数据流：Runtime call -> session manager -> cancellation token/timeout -> permission/consent -> broker/executor -> idempotency/audit。
- 兼容性：现有 coffee flow 不回归；request 401 auth retry 仍只用于安全认证握手。
- 风险控制：取消/timeout 后不再调用 callback/action/provider；dynamic timer/request 在 expire/session close 清理。

## 4. 实现方法

1. 阅读 Runtime API facade、RequestBroker、DID session manager、component dynamic cleanup 和 dock-core orchestrator。
2. 定义 session manager、per-session high-risk in-flight registry、cancellation token 和 timeout shape。
3. 定义 retry policy：auth handshake 可重试，非幂等 business API 默认不自动重试；idempotency key 由 Runtime 转发给 order/payment/provider boundary。
4. 实现 session close、pre-dispatch cancellation/timeout 和 high-risk replay cache cleanup；component expire 的 dynamic request/timer cleanup 复用既有 Component VM gate。
5. 增加 tests：multi-session isolation、parallel safe API、high-risk serial、cancel before provider、timeout、non-idempotent no retry、idempotency key propagation/replay。
6. 更新 Phase 4 文档、Threat Model、Release Gates 和 runbook。
7. 回填本 Step 和主 Plan 执行台账；Phase 4 完成后触发阶段 Review。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-core` | session manager、cancellation、high-risk serial policy、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/anp-adapter` | RequestBroker retry/idempotency policy | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/component-runtime` | dynamic timer/request cleanup on expire/session close | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/consent-audit` | idempotency/audit summary | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/security/threat-model.md` | 同步 concurrency/retry 风险 | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | concurrency/idempotency gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步并发策略 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-10-concurrency-cancellation-idempotency.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-05、Step 03-03、Step 03-05、Step 04-01、Step 04-05、Step 04-06、Step 04-07、Step 04-09。
- 外部文档或决策：Runtime API facade、RequestBroker contract、Threat Model。
- 环境前提：Rust toolchain 1.88.0；无需真实 distributed lock。

## 7. 验收标准

- [x] 多 session 并发不会串 token/storage/audit；Runtime in-flight key 绑定 user DID、agent DID、merchant DID、Skill id 和 session id，新增不同 session 高风险并发测试。
- [x] 高风险交易按 policy 串行或明确拒绝并发；同 session / same API / same optional idempotency key 的 L3/L4 in-flight 会 fail closed。
- [x] cancellation token 和 timeout 能阻止后续 provider/action/callback；当前边界是 pre-dispatch 和 deadline check，不声明抢占式中断已运行 executor/provider。
- [x] 非幂等支付/下单默认不自动重试；auth handshake retry 仍安全，新增 RequestBroker 业务 500 no-retry 测试。
- [x] idempotency key 可传递到 order/payment/provider boundary，并进入脱敏 audit summary；显式 key 在同一 RuntimeService 内成功 replay，不重复执行 executor。
- [x] session close / component expire 清理 dynamic request/timer；session close 清理本地 cancellation、in-flight 和 replay cache，component expire cleanup 由既有 Component VM focused test 证明。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Core concurrency tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core concurrency` | session/serial/cancel tests 通过；若 filter 不匹配，记录实际命令 |
| Request retry tests | `cd anp/anp-miniapp-dock && cargo test -p anp-adapter retry` | retry/idempotency tests 通过 |
| Component cleanup tests | `cd anp/anp-miniapp-dock && cargo test -p component-runtime cleanup` | expire/detach cleanup tests 通过 |
| Workspace 回归 | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过；如耗时受限，记录 focused 替代和风险 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-core crates/anp-adapter crates/component-runtime crates/consent-audit docs/security docs/runbook docs/plan` | 无空白错误 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：取消后是否仍可能执行高风险操作；retry 是否会重复交易；session isolation 是否覆盖 token/storage/audit。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 3 项，均非阻塞且已处理或记录 | 1. `RuntimeIdempotencyPolicy.required_for_high_risk` 初版为 `true`，但实现为兼容旧调用没有强制 idempotency key；2. session close 初版没有清理本地 high-risk replay cache；3. 初始 `concurrency` filter 只覆盖 policy test，multi-session / low-risk 并发证据不足。 |
| 已修复问题 | 已修复 | 将 `required_for_high_risk` 改为 `false` 并更新测试/文档；session close 清理 replay cache；新增不同 session 高风险并发和低风险同 session 并发测试，并重命名测试使 `cargo test -p dock-core concurrency` 能覆盖关键路径。 |
| 剩余风险 | 已记录 | 当前 cancellation/timeout 是 Runtime pre-dispatch/deadline 边界，不抢占已运行同步 executor/provider；idempotency replay 是本地内存级，不是 durable merchant/provider idempotency；high-risk serial 是单 RuntimeService 内存状态，不是分布式 lock。 |
| 新增或缺失测试 | 已补 focused tests | 新增 Runtime policy、cancel/close、idempotency forward/replay、同 session 高风险串行、不同 session 高风险并发、低风险并发测试；新增 RequestBroker 非幂等业务失败 no-retry 测试；component cleanup 复用既有 dynamic timer cleanup test。 |
| 已更新或缺失文档 | 已更新 | 已同步 Phase 4 实施计划、Threat Model、Release Gates、Component Compatibility Matrix、Step 文档和 roadmap；未新增 production Host/provider 文档，因为真实 Host conformance 仍是后续 blocker。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 concurrency/cancellation/idempotency、直接 tests 和相关文档。
- Commit 前状态：记录 `git status --short`。
- 纳入文件：记录本步骤 commit 包含的文件。
- Commit 后证据：记录 commit hash 和 commit 后 `git status --short --branch`。
- 遗留未提交变更：必须记录原因以及为什么安全。
- 建议消息：`phase4: add runtime concurrency controls`

执行记录：

- Commit 前状态：`git status --short` 仅包含 04-10 Runtime concurrency/cancellation/idempotency、RequestBroker retry test、直接调用点兼容更新和相关文档变更。
- 纳入文件：`crates/dock-core/src/runtime.rs`、`crates/dock-core/src/orchestrator.rs`、`crates/dock-core/src/lib.rs`、`crates/dock-core/tests/runtime_facade.rs`、`crates/dock-core/tests/host_adapter_contract.rs`、`crates/anp-adapter/tests/capability_token_scope.rs`、`crates/dock-cli/src/commands.rs`、`docs/architecture/component-compatibility-matrix.md`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-4-runtime-host-integration.md`、`docs/plan/production-readiness/steps/04-10-concurrency-cancellation-idempotency.md`、`docs/runbook/release-gates.md`、`docs/security/threat-model.md`。
- 实现 commit：`932e2e5 phase4: add runtime concurrency controls`。
- Commit 后状态：`git status --short --branch` = `## main...origin/main [ahead 81]`，工作区无未提交变更。
- 遗留未提交变更：无。

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建并发控制初版小 Plan | 将并发、取消、重试与幂等策略拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |
| 2026-06-12 | 顺延为 Step 04-10 | 为拆分原 04-04 持久化大 Step，保留并发/取消/幂等独立 Step 并顺延编号 | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：retry/并发策略错误会造成重复下单、重复支付或跨 session 数据污染。
- 回滚 / 回退：高风险能力不确定时默认串行和 no retry；记录 residual risk。
- 后续文档：Phase 6 runbook 必须覆盖 cancellation、timeout、idempotency failure 和 dynamic cleanup。
