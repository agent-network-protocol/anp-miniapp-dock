# Step 04-01：Runtime API Facade 与版本化

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：04-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-13 18:28:18 +0800 |
| Completed | 2026-06-13 18:53:14 +0800 |
| Commit | `1b470d5` |
| Review evidence | 2026-06-13 18:43:02 +0800 commit 前 Review：发现并修复 `RuntimeSkillSummary` 输出本机 skill root 绝对路径的问题，改为 digest `packageRef` 或 `local-dev-package`；发现含 `capability_token` 的 request DTO 派生 `Debug` 会扩大日志泄露风险，已移除 `RuntimeCallRequest` / `RuntimeDispatchComponentActionRequest` 的 `Debug` derive；发现 runtime validation report 可能回显本机路径、Authorization、token、private key 路径或 secret suggestion，已对 error message、validation issue path/message/suggestion 做二次 redaction 并补回归测试；确认 `expire_cards` / `close_session` 仅冻结稳定边界，不冒充生产 card/session store。 |
| Verification evidence | `cargo fmt --check` 通过；`cargo test -p dock-core runtime` 4 passed，覆盖 runtime version negotiation、validate/load/call/render/action/expire/audit/close、error JSON serialization、token/Authorization/path redaction；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/dock-cli crates/component-runtime crates/skill-loader docs/runbook docs/plan` 无输出；手工运行 `cargo run -q -p dock-cli -- call-api examples/coffee-skill searchDrinks '{}'` 和 `cargo run -q -p dock-cli -- call-api examples/coffee-skill confirmOrder '{}'` 抽样，CLI JSON 保持兼容且 validation error 为 stable code；敏感词抽样只命中 `crates/dock-core/tests/runtime_facade.rs` 中的刻意测试假值，未命中 Runtime API / CLI 输出样本。 |
| Next action | 进入 04-02 IPC / SDK 形态与 Host 进程边界 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：定义并实现稳定的 public Runtime API facade，覆盖 validate/load/call/render/action/expire/audit/close session。
- 用户 / 系统可见行为：CLI、Host adapter 和未来 IPC 都调用同一 runtime contract，不再维护第二套流程。
- 非目标：不在本 Step 实现 HTTP/gRPC sidecar；不实现真实 Host UI。
- 完成标准：Runtime API 输入输出 JSON 可序列化、错误码稳定、API version 可协商，CLI 主流程可以逐步迁移到 facade。

## 3. 设计方法

- 设计边界：Runtime facade 是产品化宿主集成边界；内部 crate 可以重构，但对外 contract 必须版本化。
- 核心决策：先稳定 Rust library API，再让 CLI 和 IPC 复用；所有返回不得泄露 token、private key、Host private meta。
- 契约 / API / 数据流：Host/CLI -> RuntimeService -> skill-loader / dock-core / component-runtime / consent-audit -> RuntimeResult / RuntimeError。
- 兼容性：保持 coffee flow、`dock-cli validate`、`run-demo` 行为；新增 facade 不改变现有公开 JSON，除非记录 migration。
- 风险控制：错误码和 `_meta` 隔离必须复用现有 redaction 和 model-visible 边界。

## 4. 实现方法

1. 阅读 `dock-core` orchestrator、`dock-cli` commands、skill-loader、component-runtime 和 current CLI flow。
2. 定义 `RuntimeService` 或等价 facade：`validate_skill`、`load_skill`、`call_api`、`render_component`、`dispatch_component_action`、`expire_cards`、`get_audit_records`、`close_session`。
3. 定义 version、request/response DTO、stable error codes 和 redaction boundary。
4. 将至少一个 CLI 主流程迁移到 facade，避免 duplicate orchestration。
5. 增加 runtime API tests：load/call/render/action/expire/audit/close、error serialization、redaction。
6. 更新 Phase 4 文档、README/runbook 或 developer notes。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-core` | Runtime API facade、DTO、error/version tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/dock-cli` | 迁移 CLI 主流程调用 facade | 代码实现 |
| `anp/anp-miniapp-dock/crates/component-runtime` | render/action facade 接入 | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/skill-loader` | load/validate facade 接入 | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-4-runtime-host-integration.md` | 同步 Runtime API contract | 必须 |
| `anp/anp-miniapp-dock/docs/runbook/local-demo.md` | 若 CLI 命令行为变化，同步说明 | 视实现结果更新 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/04-01-runtime-api-facade-versioning.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 02-06、Step 03-07。
- 外部文档或决策：Phase 4 Runtime Host Integration、Release Gates、Threat Model。
- 环境前提：Rust toolchain 1.88.0；无需外部 Host。

## 7. 验收标准

- [x] Runtime facade 覆盖 validate/load/call/render/action/expire/audit/close session 的最小 stable API。
- [x] Runtime API 有 version 字段或版本协商策略。
- [x] Error code stable、JSON 可序列化、敏感字段 redacted。
- [x] CLI 至少一个关键路径调用 facade，且 coffee E2E 不回归。
- [x] Phase 4 文档记录 API contract 和 migration 影响。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Runtime tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core runtime` | facade/API DTO/error tests 通过；若 filter 不匹配，记录实际命令 |
| CLI 回归 | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | coffee CLI E2E 通过 |
| Workspace 回归 | `cd anp/anp-miniapp-dock && cargo test --workspace` | 通过；如耗时受限，记录 focused 替代和风险 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-core crates/dock-cli crates/component-runtime crates/skill-loader docs/runbook docs/plan` | 无空白错误 |
| 脱敏抽样 | 手工检查 Runtime API error/result JSON | 不含 token、Authorization、signature、private key path 或隐私原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：facade 是否成为唯一 orchestration contract；version/error 是否稳定；CLI 是否避免第二套流程；redaction 是否集中。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 3 项需修复问题 | 1. `RuntimeSkillSummary` 初版输出本机 skill root 绝对路径，不适合作为 public Runtime DTO；2. 含 `capability_token` 的 request DTO 派生 `Debug`，未来日志中可能泄露 host-only token；3. `RuntimeErrorResponse` 初版只脱敏 message，validation report 的 path/message/suggestion 仍可能回显本机路径、Authorization、token 或 private key 路径。 |
| 已修复问题 | 已修复 | `RuntimeSkillSummary` 改为 `packageRef`，优先使用 `sha256:<digest>`，本地开发包为 `local-dev-package`；移除 `RuntimeCallRequest` / `RuntimeDispatchComponentActionRequest` 的 `Debug` derive；新增 validation report 二次脱敏并补 `runtime_facade_validation_errors_redact_reports`。 |
| 剩余风险 | 已记录，非本 Step 阻塞 | `expire_cards` 当前只返回 `host-managed-card-store` 边界，真实 card/session store、取消/幂等由 04-09/04-10 承接；`close_session` 当前为 `stateless-runtime-facade`，token/cache/session 清理由 04-05/04-10 承接；CLI JSON 保持兼容但不是 04-02 的 IPC/Host production protocol。 |
| 新增或缺失测试 | 已补 focused tests | 新增 `crates/dock-core/tests/runtime_facade.rs`，覆盖 version negotiation、validate/load summary、call/render/action/expire/audit/close、error JSON serialization、token/Authorization/path redaction；未新增 HTTP/gRPC sidecar 或真实 Host UI tests，按本 Step 非目标留给 04-02/04-09。 |
| 已更新或缺失文档 | 已更新 | 已同步 `docs/plan/production-readiness/phase-4-runtime-host-integration.md` 的 Runtime API contract、版本策略、CLI migration 和后续边界；`docs/runbook/local-demo.md` 未改，因为 `dock-cli call-api` / `run-demo` 用户可见 JSON 未改变。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 Runtime API facade、直接 tests、CLI 接入和相关文档。
- Commit 前状态：`git status --short` 只包含 04-01 Runtime facade 代码、测试和文档变更。
- 纳入文件：`crates/dock-core/src/runtime.rs`、`crates/dock-core/src/lib.rs`、`crates/dock-core/src/host.rs`、`crates/dock-core/src/orchestrator.rs`、`crates/dock-core/tests/runtime_facade.rs`、`crates/dock-cli/src/commands.rs`、`docs/plan/production-readiness/phase-4-runtime-host-integration.md`、`docs/plan/production-readiness-roadmap.md`、本 Step 文档。
- Commit 后证据：主实现 commit `1b470d5`；post-commit `git status --short --branch` = `## main...origin/main [ahead 63]`。
- 遗留未提交变更：主实现提交后无未提交变更；本 closure 文档回填作为单独提交记录。
- 建议消息：`phase4: add runtime api facade`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 04-01 小 Plan | 将 Runtime API Facade 与版本化拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：facade 如果只包一层 CLI 逻辑，会变成第三套流程而不是收敛。
- 回滚 / 回退：先让 CLI 逐步迁移，保持旧命令行为；任何 contract break 需要 migration note。
- 后续文档：Step 04-02 IPC/SDK 必须复用本 facade，不直接调内部 crate。
