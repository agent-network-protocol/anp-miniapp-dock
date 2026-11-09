# Step 06-02：Metrics / Tracing 与请求链路关联

主 Plan：[../../production-readiness-roadmap.md](../../production-readiness-roadmap.md)
Step index：06-02
状态：done

## 1. 执行状态

| 字段 | 值 |
|---|---|
| Status | done |
| Branch | `main` |
| Started | 2026-06-14 04:05:17 +0800 |
| Completed | 2026-06-14 04:43:25 +0800 |
| Commit | `7fa8aee` |
| Review evidence | 2026-06-14 04:35:38 +0800 commit 前 Review 已记录：修复初版缺失 QuickJS VM / `wx.request` / token path metrics hook、缺失 audit span、unsupported API 专项 count、`runtime.negotiateVersion` IPC 分支类型错误和 metrics label 临时借用测试问题；确认 labels 只记录 API/component/risk/outcome/status bucket 等低基数字段，不记录 URL query、headers、body、raw arguments、token、Authorization、Signature、DID 原文或本机绝对路径。 |
| Verification evidence | 启动前 `git status --short --branch` = `## main...origin/main [ahead 105]`，工作区无未提交变更；已读取主 Plan、Step 06-02 文档、Phase 6 计划、06-01 closure 证据和 Release Gates；确认 06-01 implementation commit `3fb65f0` 与 closure commit `2e899b0`；`cargo check -p dock-core`、`cargo check -p js-runtime-quickjs`、`cargo fmt --check`、`cargo test -p dock-core metrics`、`cargo test -p dock-core trace`、`cargo test -p js-runtime-quickjs quickjs_executor_records_vm_request_and_token_metrics_with_trace`、`cargo test -p dock-core`、`cargo test -p js-runtime-quickjs`、`cargo test -p dock-cli --test coffee_order_flow`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 均通过；`git diff --check -- crates/dock-core crates/js-runtime-quickjs crates/anp-adapter crates/component-runtime crates/consent-audit docs/runbook docs/plan` 无输出；敏感串扫描仅命中测试假值、redaction marker、文档红线和既有 mock 说明，未发现真实凭据、raw token、Authorization、Signature、DID 原文、本机绝对路径或隐私 payload 泄露。 |
| Next action | 进入 06-03 |

状态取值：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`。

## 2. 目标

- 结果：建立 metrics 和 tracing correlation，覆盖 API latency、VM time、render latency、request status、fallback rate、consent rate、unsupported count、sandbox limit、token refresh/fail。
- 用户 / 系统可见行为：一次用户请求能通过 traceId 串起 Host message、Skill/API call、wx.login/request、merchant response、render、action、audit。
- 非目标：不绑定具体 APM vendor；不上传敏感 payload。
- 完成标准：metrics 名称、label、cardinality、trace propagation、redaction 和 tests 完成。

## 3. 设计方法

- 设计边界：metrics/traces 用于健康度和性能定位，不记录业务隐私内容。
- 核心决策：低 cardinality labels；DID 默认 hash 或按 policy redacted；traceId 在 Runtime API/IPC/Host adapter/request broker 中传递。
- 契约 / API / 数据流：Runtime request -> trace context -> operations -> metrics recorder -> export/sink boundary。
- 兼容性：可以先用 in-memory/test recorder，Phase 6 后续再接 Prometheus/OpenTelemetry 或 Host sink。
- 风险控制：禁止把 URL query、headers、request body、address/phone/file content 放进 labels。

## 4. 实现方法

1. 阅读 Step 06-01 event model 和 Phase 6 metrics/tracing 计划。
2. 定义 metrics registry、labels 和 trace context propagation。
3. 在 API call、VM execution、render、request、fallback、consent、sandbox、token refresh/fail 路径记录 metrics。
4. 建立 tracing span 关联：Host message -> Runtime API -> broker/provider/render/action/audit。
5. 增加 tests：metrics increments、latency recording、label redaction/cardinality、trace propagation。
6. 更新 Phase 6 文档、runbook 和 release gates。
7. 回填本 Step 和主 Plan 执行台账。

## 5. 路径

| 仓库 / 模块 / 文件 | 计划变更 | 备注 |
|---|---|---|
| `anp/anp-miniapp-dock/crates/dock-core` | metrics recorder、trace context、tests | 代码实现 |
| `anp/anp-miniapp-dock/crates/anp-adapter` | request/token metrics hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/js-runtime-quickjs` | VM metrics hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/component-runtime` | render metrics hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/crates/consent-audit` | consent/audit metrics hooks | 视当前结构修改 |
| `anp/anp-miniapp-dock/docs/runbook/release-gates.md` | metrics/tracing gate | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/phase-6-observability-release.md` | 同步 metrics/tracing contract | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` | 回填执行台账 | 必须 |
| `anp/anp-miniapp-dock/docs/plan/production-readiness/steps/06-02-metrics-tracing-correlation.md` | 回填状态、证据、Review、commit | 必须 |

## 6. 依赖

- 前置步骤：Step 04-02、Step 06-01。
- 外部文档或决策：Runtime API/IPC envelope、observability event model。
- 环境前提：Rust toolchain 1.88.0；可先使用 in-memory recorder。

## 7. 验收标准

- [x] Metrics 覆盖 Phase 6 列出的核心指标。
- [x] Labels 低 cardinality，不包含敏感 payload、headers、URL query 或隐私原文。
- [x] Trace context 能串起 Runtime API、QuickJS VM、`wx.request`、token/session、render、action、nested `api/call`、audit 的核心本地路径。
- [x] In-memory/test recorder 有 unit tests，未来 exporter 有明确 boundary。
- [x] Phase 6 文档和 runbook 与实现状态同步。
- [x] Review 发现已经修复或明确记录。
- [x] 本步骤在进入下一步之前已经创建 focused commit，并回填主 Plan 执行台账。

## 8. 验证方式

| 检查项 | 命令 / 方法 | 预期证据 |
|---|---|---|
| 格式 | `cd anp/anp-miniapp-dock && cargo fmt --check` | 通过 |
| Metrics tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core metrics` | metrics/labels tests 通过 |
| Trace tests | `cd anp/anp-miniapp-dock && cargo test -p dock-core trace` | trace propagation tests 通过；若 filter 不匹配，记录实际命令 |
| Coffee E2E | `cd anp/anp-miniapp-dock && cargo test -p dock-cli --test coffee_order_flow` | 通过并可观察 trace/metrics evidence 或记录替代 |
| 文档/空白 | `cd anp/anp-miniapp-dock && git diff --check -- crates/dock-core crates/anp-adapter crates/js-runtime-quickjs crates/component-runtime crates/consent-audit docs/runbook docs/plan` | 无空白错误 |
| 安全抽样 | 手工检查 metrics labels/traces | 不含 token、headers、query、phone/address/file/location 原文 |

如果某个命令不能运行，必须记录原因、影响和替代证据。

## 9. Review 环节

- Review 时机：实现、测试、文档同步完成后、commit 前。
- Review 重点：label cardinality 是否可控；trace 是否覆盖关键路径；是否泄露 headers/query/body；是否能定位失败阶段。

| Review 项 | 结果 | 备注 |
|---|---|---|
| 发现问题 | 已发现并修复 | 初版只在 RuntimeService 外层记录 metrics，未覆盖 QuickJS VM execution、`wx.request` status、token/session path；缺少 audit span 和 unsupported API 专项 count；IPC `runtime.negotiateVersion` error branch 类型不一致；测试中 metrics 临时 Vec 借用不稳定。 |
| 已修复问题 | 已修复 | 新增 `MetricsSink` / `InMemoryMetricsSink` / `TraceContext` / `TraceSpan`，RuntimeService 支持 metrics sink；QuickJS executor 支持可注入 metrics sink；`wx.login` / `wx.checkSession` / `wx.request` 记录 token/request metrics 和 spans；audit 和 unsupported API 补 metrics/spans；IPC 分支和测试借用问题已修复。 |
| 剩余风险 | 已记录 | 当前只提供 no-op 和 in-memory/test recorder，不接 Prometheus/OpenTelemetry/Host exporter；显式 `runtime.renderComponent` 尚无 operation trace 输入字段，因此生成独立 root trace，完整 Host render trace 需后续 Host/IPC contract 扩展。 |
| 新增或缺失测试 | 已覆盖核心路径 | 新增 `dock-core` metrics/trace tests 覆盖 API、render、component action、nested `api/call`、audit、sandbox timeout、unsupported API 和 label redaction；新增 `js-runtime-quickjs` test 覆盖 VM/request/token metrics hook 与 trace propagation。 |
| 已更新或缺失文档 | 已更新 | 更新 Phase 6 metrics/tracing contract、release gates metrics/tracing gate、本 Step 和主 Plan 台账；后续 06-03/06-05/06-06 将复用这些 metrics 作为性能、canary 和 runbook 信号。 |

## 10. Commit 要求

- Commit 时机：实现、验证、Review、文档同步完成后。
- Commit 范围：只包含 metrics/tracing、direct tests 和相关文档。
- Commit 前状态：`git status --short --branch` = `## main...origin/main [ahead 105]`，纳入文件为 `crates/dock-core/src/lib.rs`、`crates/dock-core/src/observability.rs`、`crates/dock-core/src/orchestrator.rs`、`crates/dock-core/src/runtime.rs`、`crates/dock-core/tests/api_call_flow.rs`、`crates/dock-core/tests/runtime_facade.rs`、`crates/js-runtime-quickjs/src/api_vm.rs`、`crates/js-runtime-quickjs/src/lib.rs`、`crates/js-runtime-quickjs/tests/middleware_chain.rs`、`docs/plan/production-readiness-roadmap.md`、`docs/plan/production-readiness/phase-6-observability-release.md`、`docs/plan/production-readiness/steps/06-02-metrics-tracing-correlation.md`、`docs/runbook/release-gates.md`。
- Commit 后证据：implementation commit `7fa8aee phase6: add metrics tracing correlation`；commit 后 `git status --short --branch` = `## main...origin/main [ahead 106]`，工作区无未提交代码变更。
- 遗留未提交变更：仅本 closure 文档更新，单独提交后进入 06-03。
- 建议消息：`phase6: add metrics tracing correlation`

## 11. Blocked 处理

| Blocker | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

## 12. Plan 变更记录

| 日期 | 变更 | 原因 | 主 Plan 变更记录链接 |
|---|---|---|---|
| 2026-06-12 | 创建 Step 06-02 小 Plan | 将 Metrics / Tracing 与请求链路关联拆成可执行 Step | `anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md` |

## 13. 风险、回滚与后续文档

- 风险：高 cardinality label 会造成运行成本和隐私风险。
- 回滚 / 回退：不确定字段只进 redacted event，不进入 metric label。
- 后续文档：Step 06-05 canary/rollback 使用这些 metrics 作为决策信号。
