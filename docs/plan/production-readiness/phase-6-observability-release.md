# Phase 6：观测、性能与发布运营实施计划

## 1. 阶段目标

Phase 6 让容器具备线上运行的可观测性、性能边界、发布门禁、灰度和回滚能力。完成后，线上问题能定位到具体 session、Skill、API、Host provider 或 merchant Agent，而不需要查看敏感 payload。

## 2. 观测模型

### 2.1 结构化事件

必须记录但默认脱敏：

- `skill_load_start/end`
- `api_call_start/end`
- `wx_api_call_start/end`
- `request_start/end`
- `consent_prompt/decision`
- `component_render_start/end`
- `component_event`
- `fallback_used`
- `audit_record_written`
- `sandbox_limit_hit`

Step 06-01 已在 `dock-core` 固化事件 schema `dock.observability.event.v1`、脱敏策略 `dock.observability.redaction.v1`、`NoopObservabilitySink` 和 `InMemoryObservabilitySink`，并在 `RuntimeService` 的 Skill load、API call、ConsentGate、Render IR render、component event、Host action、fallback、audit written 和 timeout/limit 边界发出结构化事件。`wx_api_call_*` 与 `request_*` 已作为稳定事件类型保留，后续 Step 06-02 可在 QuickJS / RequestBroker / metrics bridge 中复用同一事件模型接入更细粒度链路。

公共字段：

```text
traceId
sessionId
skillId
apiName?
componentPath?
merchantDid?
userDid? (可 hash)
runtimeVersion
renderIrVersion
outcome
latencyMs
```

公共字段落地规则：

- `userDid` 不直接记录，默认写入 `userDidHash = sha256:<hex>`。
- `merchantDid` 可按 policy 作为定位字段输出；涉及 token、Authorization、Signature、private key、手机号、地址、文件内容、精确位置、本机绝对路径的字段必须在 emit 前替换为 `[REDACTED]`。
- `fields` 只能放定位 metadata、计数、状态、risk level、release/readiness 信息，不放 raw arguments、HTTP body、文件内容或 Host provider 原始 payload。
- Runtime 默认使用 no-op sink；Host、CLI 或测试可以注入 sink。Step 06-01 已完成 structured event sink；Step 06-02 已补 `dock.observability.metric.v1`、`dock.observability.trace.v1`、in-memory/test metrics recorder、RuntimeService metrics hooks 和 QuickJS executor / `wx.request` / token path metrics hooks。当前不接入外部日志、Prometheus 或 OpenTelemetry vendor exporter。

### 2.2 Metrics

| 指标 | 目的 |
|---|---|
| API latency | 识别慢接口 |
| VM execution time | sandbox 性能 |
| render latency | 组件渲染性能 |
| request status | merchant/网络错误 |
| fallback rate | 兼容性质量 |
| consent approve/deny rate | 风控与 UX |
| unsupported API count | 迁移阻塞点 |
| sandbox timeout/memory hit | 恶意或低质量 Skill |
| token refresh/fail count | DID/auth 健康度 |

Step 06-02 当前实现的本地指标契约：

- `dock.skill_load.total`：Skill load outcome 与 supply-chain status。
- `dock.api_call.total`、`dock.api_latency_ms`：Runtime API call start/end、outcome、risk level 和 bounded API name。
- `dock.api_vm_execution_ms`：QuickJS Atomic API VM execution time，按 API name 和 outcome 记录。
- `dock.render_latency_ms`：Render IR render latency，按 manifest bounded component path 和 outcome 记录。
- `dock.request.total`、`dock.request_latency_ms`：QuickJS `wx.request` status，HTTP status 只记录 `2xx`/`3xx`/`4xx`/`5xx`/`other` 或低基数失败码，不记录 URL、query、headers 或 body。
- `dock.fallback.total`：fallback reason enum。
- `dock.consent.total`：prompt / decision outcome 和 risk level。
- `dock.unsupported_api.total`：Runtime API registry miss count。
- `dock.sandbox_limit.total`：timeout / sandbox limit 命中。
- `dock.token_refresh.total`、`dock.token_refresh_latency_ms`：`wx.login` / `wx.checkSession` token/session path outcome，不记录 raw token。
- `dock.audit_record.total`、`dock.audit_latency_ms`：audit record outcome、risk level 和 trace correlation。

所有 label 必须低 cardinality；`ObservabilityMetric` 和 `TraceSpan` 会对 URL query、header/token marker、DID 原文、本机绝对路径和过长 label 执行 redaction。Runtime 和 QuickJS 默认 no-op；测试与 Host 集成可注入 `InMemoryMetricsSink` 或未来 exporter sink。

### 2.3 Tracing

一条用户请求应串起：

```text
Host message
  -> model/intent decision
  -> Skill/API call
  -> wx.login/checkSession/request
  -> merchant response
  -> component render
  -> user action
  -> follow-up api/call
  -> audit
```

Step 06-02 当前 trace propagation：

- `RuntimeOperationOptions.trace` 可接收 Host/IPC 层传入的 `traceId` 和 parent span。
- `ApiCallContext.trace` 把 Runtime API trace 传入 Orchestrator、API executor、component action 和 nested `api/call`。
- RuntimeService 为 API call、render、component action、audit 和 IPC 记录 span。
- QuickJS executor 为 API VM、`wx.request`、`wx.login` / `wx.checkSession` 记录 span，并继承 `ApiCallContext.trace`。
- 显式 `runtime.renderComponent` 当前没有 operation trace 输入字段，因此独立生成 root trace；Host envelope 若需要把 render 与上一跳强绑定，应在后续 Host/IPC contract 扩展中为 render request 增加 operation trace 字段。

## 3. 性能基线

建议基准：

| 基准 | 初始目标 |
|---|---|
| Skill load | 本地 P50/P95 |
| API VM cold call | P50/P95 |
| API VM warm-ish call | P50/P95 |
| Component render | P50/P95 |
| Render IR size | P50/P95 |
| token lookup | P50/P95 |
| storage read/write | P50/P95 |
| memory per VM | max |

具体数值应在实现基准测试后写入 release notes，不在计划文档中凭空承诺。

Step 06-03 当前实现的本地性能基线契约：

- `dock-cli perf <skill> --iterations <n>` 输出 `dock.perf-baseline-report.v1` JSON，默认 smoke 模式；`--full` 可放大迭代数，仍是本地硬件相关证据。
- samples 覆盖 Skill load、API VM call、component render、Render IR size、token lookup、storage read/write、process RSS memory sample、concurrent sessions、多 Skill、多组件、dynamic timer/request、resource-limit fail-closed。
- `testdata/perf/coffee-smoke-baseline.json` 是 coffee smoke baseline artifact 和 schema 样例；包含环境 commit、rustc、os、arch、workingTreeDirty、P50/P95/max 或 size/gauge，不作为跨机器固定阈值。
- perf runner 复用 `dock-cli` headless/dev-only fixture provider、RuntimeService、Component Runtime 和 fixture snapshots；报告必须继续脱敏，不输出 raw token、Authorization、Signature、capabilityToken、private key、本机绝对路径、手机号、真实地址、文件内容或精确位置。

## 4. CI/CD Gates

基础 gates：

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

产品 gates：

- compatibility matrix coverage；
- sandbox escape regression；
- redaction regression；
- DID/token replay/scope tests；
- Render IR snapshots；
- fixture E2E；
- markdown link check；
- release notes completeness。

Step 06-04 当前实现的本地 release gate runner 契约：

- `./scripts/release-gates.sh` 是 vendor-neutral 本地入口，默认输出 `target/release-gates/release-gates-report.json`。
- report schema 为 `dock.release-gates-report.v1`，记录 `pass` / `fail` / `skip`、命令、日志路径、commit、dirty 状态、hard blockers 和 release decision。
- full 模式执行基础 Rust gates、coffee E2E、validate/doctor JSON、observability、metrics/tracing、sandbox/security、permission/allowlist、DID/token、consent/audit/supply-chain、snapshot、fixture、performance、Markdown link、兼容矩阵状态、artifact redaction 和 docs diff gates。
- `--quick` 只验证脚本/report/文档检查和 artifact redaction scan，结果必须是 `needs-review`，不能作为 release approval。
- release notes completeness 需要 `--release-notes <path>` 或 `RELEASE_NOTES_PATH=<path>`；未提供时记为 `skip`，后续 Step 06-05 必须提供 canary/release notes 后复用该 gate。

## 5. 发布策略

### 5.1 版本化对象

- Runtime API version；
- Render IR schema version；
- capability token version；
- Skill package contract version；
- Host adapter contract version。

### 5.2 灰度流程

1. headless fixture 全量通过；
2. internal Host canary；
3. allowlisted merchant Skill；
4. expand by publisher DID / skill version；
5. monitor fallback/error/consent/token metrics；
6. rollback on gate breach。

Step 06-05 当前实现的本地发布流程契约：

- [`../../runbook/release-process.md`](../../runbook/release-process.md) 定义 release candidate、canary stage、rollback condition/action、cache purge procedure 和 dry-run checklist。
- [`../../runbook/releases/2026-06-14-local-canary.md`](../../runbook/releases/2026-06-14-local-canary.md) 是当前本地 Stage 0 release notes dry-run，包含版本、兼容变化、安全变化、风险、migration、rollback、gate evidence 和 canary plan。
- `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md` 必须让 release notes completeness gate 通过；没有 release notes 路径时仍是 `needs-review`，不能生产发布。
- Stage 0 只证明 headless/local gates；Stage 1 internal Host、Stage 2 allowlisted merchant、Stage 3 expansion 仍需真实 Host/deploy platform 证据。

### 5.3 回滚条件

- token leakage regression；
- consent bypass；
- sandbox escape；
- fallback rate 超阈值；
- auth failure rate 激增；
- Host crash / Render IR incompatible；
- audit write failure。

## 6. 运维 Runbook

Step 06-06 当前实现的运维 runbook 契约：

- [`../../runbook/operations.md`](../../runbook/operations.md) 定义事件流程、日常 gate、观测字段、升级路径和收尾 checklist。
- [`../../runbook/troubleshooting.md`](../../runbook/troubleshooting.md) 按 DID 验签、token scope、allowlist、component render、sandbox、storage quota、audit unavailable、Host provider、merchant Agent、Skill signature 和 rollback/cache purge 故障域定义症状、event/metric、检查命令、处理步骤、升级/回滚条件。
- [`../../runbook/privacy-deletion.md`](../../runbook/privacy-deletion.md) 定义 user/merchant/Skill/session scope 下 token revoke、storage delete-scope、audit redacted export/retention、Skill cache cleanup 和 release evidence retention 顺序。
- 当前 runbook 只提供 repository-local 和 Host-agnostic 运维流程；真实 production Host secure store、encrypted storage/audit backend、deploy platform、traffic router 和 provider conformance 命令仍需 Host-specific 文档补齐。

覆盖项：

- DID 验签失败；
- token scope mismatch；
- allowlist deny；
- component render failed；
- sandbox timeout；
- storage quota exceeded；
- audit sink unavailable；
- Host provider unavailable；
- merchant Agent unavailable；
- Skill package signature mismatch；
- rollback and cache purge。
- privacy deletion。

## 7. 阶段完成检查

- [x] 结构化日志和 metrics 默认脱敏。
- [x] trace 能串起 Runtime API、QuickJS VM、`wx.login` / `wx.checkSession` / `wx.request`、render、action、nested `api/call` 和 audit 的本地测试链路；完整 Host message / model decision 侧 trace 仍需真实 Host adapter 注入。
- [x] 性能基准有自动化脚本。
- [x] CI gates 覆盖安全、兼容、snapshot、文档。
- [x] canary/rollback runbook 可执行。
- [x] release notes 包含版本、兼容变化、风险和回滚方式。
- [x] operations/troubleshooting/privacy deletion runbook 覆盖主要故障域、scope deletion、audit evidence retention 和 Host-specific gap。
