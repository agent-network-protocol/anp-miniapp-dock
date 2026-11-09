# Step 06-01：结构化观测事件与脱敏日志

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：06-01
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 03:41:41 +0800 |
| Completed | 2026-06-14 04:03:32 +0800 |
| Commit | `3fb65f0` |
| Review evidence | 2026-06-14 03:59:38 +0800 commit 前 Review 已记录：确认 `dock-core::observability` 事件 schema、redaction policy、hashed user DID、RuntimeService emit hooks 和 tests 已覆盖当前公共 Runtime 入口；修复前期 UTF-8 截断和构造函数参数过多风险；确认事件 fields 不记录 raw arguments、token、Authorization、Signature、private key path、手机号、地址、文件内容或精确位置。剩余风险：`wx_api_call_*` / `request_*` 当前为稳定 schema 类型，尚未从 QuickJS / RequestBroker 细粒度路径 emit；06-02 必须接入 metrics/tracing bridge 时补真实链路事件和 trace propagation。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 103]`，工作区无未提交变更；已读取主 Plan、Step 06-01 文档、Phase 6 计划、Release Gates、RuntimeService/Orchestrator/Host/Audit/Component Runtime 相关源码和 runtime tests；已确认 Phase 5 final review commit `18ca5b2`、closure commit `ebec9b7`、closure evidence commit `8feb301`。`cargo fmt --check` 通过；`cargo test -p dock-core observability` 通过，实际命中 1 个 observability unit test 和 2 个 runtime observability tests；`cargo test -p dock-core runtime_observability` 通过，实际命中 2 个 runtime observability tests；`cargo test -p dock-core` 46 passed；`cargo test -p dock-cli --test coffee_order_flow` 12 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`git diff --check -- crates/dock-core docs/runbook docs/plan Cargo.toml Cargo.lock` 无输出；敏感串扫描仅命中测试假值、redaction 断言和安全/计划文档红线，未发现真实凭据或隐私 payload 泄露。 |
| Next action | 进入 06-02 Metrics / Tracing 与请求链路关联 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：建立统一结构化事件模型，覆盖 skill load、API call、wx API、request、consent、render、component event、fallback、audit、sandbox limit。
- 用户 / 系统可见行为：线上问题可按 traceId/sessionId/skillId/apiName/componentPath 定位阶段，但不需要查看敏感 payload。
- 非目标：不接入具体云日志平台；不实现完整 distributed tracing backend。
- 完成标准：结构化事件字段、redaction、event IDs、severity/outcome 规范落地，并有 tests。

## 3. 设计方法

- 设计边界：观测事件只记录定位所需 metadata，不记录 token、Authorization、signature、private key、手机号、地址、文件内容或精确位置。
- 核心决策：公共字段包括 traceId、sessionId、skillId、apiName、componentPath、merchantDid、hashed userDid、runtimeVersion、renderIrVersion、outcome、latencyMs。
- 契约 / API / 数据流：Runtime operation -> event builder -> redaction -> log sink/metrics/tracing bridge。
- 兼容性：现有日志保持可读，但 production profile 输出 structured JSON 或可解析事件。
- 风险控制：userDid 默认 hash；debug payload dev-only 且 redacted。

## 4. 实现方法

1. 阅读 Phase 6 observability 计划、Threat Model redaction 要求和 Runtime API facade。
2. 定义 event enum 和公共字段，覆盖 Phase 6 列出的结构化事件。
3. 在核心路径埋点：skill load、api call、wx api call、request、consent、component render/event、fallback、audit write、sandbox limit。
4. 实现 redaction helper 和 test fixtures，确保 event payload 不含敏感字段。
5. 增加 tests：event serialization、required fields、hashed userDid、redaction、latency/outcome。
6. 更新 Release Gates、Phase 6 文档和 runbook。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-core` | observability event model、runtime emit points、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | wx API/sandbox event hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/component-runtime` | render/component event hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/consent-audit` | audit written event | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | observability redaction gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-6-observability-release.md` | 同步事件模型 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/06-01-structured-observability-events.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 04-01、Step 04-04、Step 05-07。
- 外部文档或决策：Threat Model、Runtime API、Release Gates。
- 环境前提：Rust toolchain 1.88.0；无需外部 log backend。

## 7. 验收标准

- [x] 结构化事件 schema 覆盖 Phase 6 列出的关键 runtime 操作；当前 RuntimeService emit 覆盖 Skill load、API call、ConsentGate、Render IR render、component event、Host action、fallback、audit written 和 timeout/limit，`wx_api_call_*` / `request_*` 真实 QuickJS / RequestBroker emit 留给 06-02。
- [x] 每个 RuntimeService 事件有 traceId/sessionId/skillId/outcome/latency 或明确不适用理由；skill load 事件无 sessionId/latency，记录 api/component count 和 readiness metadata。
- [x] userDid 默认 hash，merchantDid 可按 policy 输出或 redacted。
- [x] 事件 payload 不含 token、Authorization、signature、private key path、手机号、地址、文件内容或精确位置。
- [x] Release Gates 增加 observability redaction 检查。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Event tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core observability` | event serialization/redaction tests 通过；若 filter 不匹配，记录实际命令 |
| Runtime 回归 | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-core crates/js-runtime-quickjs crates/component-runtime crates/consent-audit docs/runbook docs/plan` | 无空白错误 |
| 敏感信息扫描 | 手工或 `rg` 检查 event fixtures/log output | 不含 raw sensitive payload |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：事件是否足以定位失败；是否过度记录 payload；redaction 是否在 emit 前；event schema 是否稳定。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已记录 | 初版 Review 发现两个实现风险：redaction 长字符串不能按 byte slice 截断；RuntimeService observability 构造函数不应通过过长参数列表扩大 clippy 风险。另发现范围风险：`wx_api_call_*` / `request_*` 事件类型已存在但未从 QuickJS / RequestBroker 真实 emit。 |
| 已修复问题 | 已修复 | redaction 截断改为 char-safe；新增 `RuntimeServiceParts` 承载 observability 构造参数；RuntimeService 默认 no-op sink 保持既有 call sites 兼容。 |
| 剩余风险 | 已记录 | `wx_api_call_*` / `request_*` 当前只是 schema-supported planned bridge，未接入 `js-runtime-quickjs` / `component-runtime` / `anp-adapter` 的细粒度请求链路；06-02 metrics/tracing 必须补 trace propagation 和 request/token/VM hooks，release gates 不得宣称完整 QuickJS / RequestBroker 链路已覆盖。 |
| 新增或缺失测试 | 已补充 | 新增 observability unit test 覆盖 serialization/redaction/hashed user DID；新增 runtime facade tests 覆盖核心 flow 事件、sensitive field redaction 和 timeout/sandbox limit event。缺失测试：QuickJS / RequestBroker 细粒度 `wx_api_call_*` / `request_*` emit，归入 06-02。 |
| 已更新或缺失文档 | 已更新 | 已更新 Phase 6 计划、Release Gates、本 Step 与主 Plan 台账；文档明确 06-01 不接外部日志平台，06-02 接 metrics/tracing bridge。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 observability event model/hooks、direct tests 和相关文档。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 103]`，未提交变更仅包含 06-01 observability event model/hooks、direct tests 和相关文档。
- 纳入文件：`Cargo.lock`、`crates/dock-core/Cargo.toml`、`crates/dock-core/src/lib.rs`、`crates/dock-core/src/runtime.rs`、`crates/dock-core/src/observability.rs`、`crates/dock-core/tests/runtime_facade.rs`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-6-observability-release.md`、`docs/plan/production-readiness/steps/06-01-structured-observability-events.md`、`docs/runbook/release-gates.md`。
- Commit 后证据：implementation commit `3fb65f0 phase6: add structured observability events`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 104]`，工作区无未提交变更。
- 遗留未提交变更：无。
- 建议消息：`phase6: add structured observability events`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 无 | 无 | 无 | 当前步骤 / 整体计划 | 无 blocker，进入 06-02 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 06-01 小 Plan | 将结构化观测事件与脱敏日志拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：观测事件若记录 payload，会造成系统性隐私泄露。
- 回滚 / 回退：默认只记录 metadata；需要 debug payload 时必须 dev-only 且 redacted。
- 后续文档：Step 06-02 metrics/tracing 复用本 event model。
