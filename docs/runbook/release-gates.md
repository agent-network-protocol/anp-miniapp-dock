# Release Gates Runbook

> 状态：Phase 6 本地 release gates 已完成并通过 final Review；Step 03-02 sandbox/resource、Step 03-03 permission/allowlist、Step 03-04 DID/token lifecycle、Step 03-05 consent/audit persistence、Step 03-06 Skill package integrity/supply-chain、Step 04-03 本地 registry/cache/version/rollback contract、Step 04-04 runtime config / secret boundary、Step 04-05 token cache persistence contract / restore policy、Step 04-06 scoped storage persistence contract / quota / restore / delete-scope、Step 04-07 audit persistence profile / retention-export report / unavailable fail-closed、Step 04-08 Skill cache cleanup / quarantine lifecycle、Step 04-09 Host adapter action contract、Step 04-10 Runtime concurrency / cancellation / retry / idempotency contract、Step 05-05 CLI doctor diagnostics、Step 05-07 developer docs / migration guides、Step 06-01 structured observability events / redaction、Step 06-02 metrics / tracing correlation、Step 06-03 performance baseline / stress smoke、Step 06-04 本地 release gate runner / JSON report、Step 06-05 canary / rollback / release notes dry-run、Step 06-06 operations / troubleshooting / privacy deletion runbook 已有本地 release gate 证据；真实 production Host secure store、encrypted storage/audit backend、deploy platform 和 provider conformance 仍是生产接入 blocker。
> 日期：2026-06-13
> 范围：定义 `anp-miniapp-dock` 每次进入 production-readiness milestone、release branch 或 production deployment 前需要执行或明确记录的验证、Review、红线和回滚条件。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 03-01。

## 1. 使用规则

每个 Step 仍按 roadmap 的小 Plan 执行；本 runbook 是跨 Step、跨 Phase 的 release gate 汇总。执行 release gates 时必须记录：

- 执行日期、分支和 commit range；
- 每个命令的 pass/fail/skip；
- skip 原因、影响和替代证据；
- Review 发现、修复或残余风险；
- 最终 `git status --short --branch`。

任何 gate 失败不得靠“文档说明”绕过。只有当 gate 标为 planned 且当前 Phase 尚未实现时，才能记录为 planned gap。

### 1.1 自动化入口

本地 vendor-neutral release gate runner：

```bash
./scripts/release-gates.sh
```

默认输出：

- JSON report：`target/release-gates/release-gates-report.json`
- schema：`dock.release-gates-report.v1`
- command logs：`target/release-gates/logs/`
- CLI JSON artifacts：`target/release-gates/artifacts/`

报告语义：

- `releaseDecision = "pass"`：required gates 均通过，且没有 skip。
- `releaseDecision = "needs-review"`：没有失败，但至少一个 gate skip；skip 不计 pass，必须 Review 原因和 residual risk。
- `releaseDecision = "blocked"`：required gate 或 hard blocker 失败；不得发布。
- hard blocker 包括 redaction failure、consent bypass、sandbox escape、token leakage、`Authorization` / `Signature` leakage。

快速脚本验证：

```bash
./scripts/release-gates.sh --quick
```

`--quick` 只用于验证脚本、report、文档链接、矩阵状态和 artifact redaction scan，不是 release approval。

Release notes completeness gate：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
# 或
RELEASE_NOTES_PATH=docs/runbook/releases/2026-06-14-local-canary.md ./scripts/release-gates.sh
```

release notes 文件必须包含版本、兼容变化、风险、回滚和 migration/breaking-change 说明。没有提供 release notes 路径时，runner 会把该 gate 记为 `skip` / `needs-review`，不能当作生产发布通过。

Canary / rollback 流程见 [`release-process.md`](release-process.md)。当前 `2026-06-14-local-canary.md` 只证明 Stage 0 headless/local dry-run；Stage 1 internal Host、Stage 2 allowlisted merchant 和 Stage 3 expansion 仍需要真实 Host/deploy 平台证据。

Operations / troubleshooting / privacy deletion 流程见 [`operations.md`](operations.md)、[`troubleshooting.md`](troubleshooting.md) 和 [`privacy-deletion.md`](privacy-deletion.md)。这些 runbook 定义通用诊断、故障域分流、scope deletion、audit evidence retention 和 Host-specific gap；真实生产 Host 仍必须补充 secure store、encrypted storage/audit backend、deploy platform 和 provider conformance 命令。

## 2. 基础命令 Gate

这些命令来自仓库 `AGENTS.md` 和 README，是当前 release 前基础 gate。

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p dock-cli --test coffee_order_flow
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- doctor
cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1
```

通过标准：

- `cargo metadata` 成功，workspace 成员与文档一致。
- `cargo fmt --check` 无格式 diff。
- `cargo clippy` 无 warning。
- `cargo test --workspace` 全部通过。
- coffee CLI E2E 通过，并继续断言 capability token、Authorization、Signature、Signature-Input、private key path/material 不出现在 JSON 输出。
- `dock-cli validate` 输出 JSON，包含 `schemaVersion: "dock.validate-report.v1"`、`status` / `reportStatus`、`commandStatus`、`compatibilityLevel`、顶层和 `compatibilityReport` 内的 `apis/components/permissions/risks/fallbacks/releaseBlockers/repairSuggestions/releaseReadiness`；当前 coffee Skill 仍应因 demo-only localhost DID/request metadata 和 unsigned package 被标为 `status: "warning"`、`compatibilityLevel: "demo-only"`，不得误标 `supported` 或 production-ready。
- `dock-cli doctor` 输出 JSON，包含 `schemaVersion: "dock.doctor-report.v1"`、`status` / `reportStatus`、`commandStatus`、`summary.pass/warn/fail/skip`、`skipCountsAsPass: false` 和每个 check 的 `status/evidence/suggestion`；默认本地开发配置可以是 `warning`，但不得输出 signing credential material、raw token、Authorization、signature、secret、本机绝对路径或隐私原文。
- `dock-cli perf` 输出 JSON，包含 `schemaVersion: "dock.perf-baseline-report.v1"`、`mode`、`baseline.iterationsPerCase`、环境 commit/rustc/os/arch/workingTreeDirty、Skill load、API VM call、component render、Render IR size、token lookup、storage read/write、process RSS memory sample、concurrent sessions、多 Skill、多组件、dynamic timer/request 和 resource-limit fail-closed 证据；输出是硬件相关 baseline，不得写成 production SLO。

如果环境无法运行全量命令，必须记录原因、失败命令、影响范围和替代检查；不能把未运行命令写成通过。

## 3. 文档 Gate

当前文档 gate：

```bash
git diff --check -- README.md AGENTS.md docs/architecture docs/developer docs/runbook docs/security docs/plan
```

手工检查：

| 检查项 | 当前要求 | planned 提升 |
|---|---|---|
| Markdown 链接 | 新增或修改文档的相对链接必须指向存在文件；`scripts/release-gates.sh` 已自动检查本地相对 Markdown 链接 | 后续可接入 CI vendor |
| 兼容矩阵状态 | `supported`、`host-boundary`、`planned-p1`、`planned-p2`、`demo-only`、`unsupported-by-design` 不得混用或写成未知；`scripts/release-gates.sh` 已自动检查 API / Component 矩阵状态枚举 | 后续可扩展列级 schema |
| Step 台账 | 每个 Step 有 status、Review evidence、verification evidence、commit hash | Codex Goal 长跑每步必填 |
| Validate 报告 | `dock-cli validate examples/coffee-skill` 的 `schemaVersion` 为 `dock.validate-report.v1`，`status` 为 `warning`，`commandStatus` 为 `ok`，顶层和 `compatibilityReport` 均包含 API 注册、组件加载、权限、风险、fallback、release blocker、repair suggestion 和 release readiness 字段 | Phase 6 将报告 schema 纳入自动 release gate |
| Plan 变更 | 改范围、顺序、验收、安全边界或验证策略前先更新 Plan 变更记录 | Phase 6 可纳入 PR checklist |
| README 索引 | 新增架构、安全、runbook 文档必须有入口链接 | 当前手工检查 |
| Developer docs 索引 | `docs/developer/README.md` 必须链接 import、API compatibility、component compatibility、security guidelines 和 Host adapter guide | 当前手工检查 |

## 4. 安全 Gate

本节把安全 gate 分成三类：

- **当前必须执行**：当前代码库已有自动化或手工 gate，任何 Step / release 都必须执行或记录无法执行原因。
- **Phase 3 required**：Phase 3 的 Step 03-02 至 03-06 必须实现或升级为 required 的 gate；在对应 Step 完成前不得写成“已通过”。Step 03-02 已升级为本地 required release gate；CI 自动化仍待 Phase 6。
- **Phase 4/5 后续 gate**：依赖 production Host、registry、持久化 backend、开发者工具或 CI 自动化的 gate；当前仍是 production release blocker，但不作为 03-01 的已实现证据。

### 4.1 当前必须执行

当前已有自动化测试覆盖以下安全主线，随 `cargo test --workspace` 执行：

| Gate | 证据 |
|---|---|
| Skill package path escape / absolute path / outside symlink / zip slip deny | [`coffee_skill_load.rs`](../../crates/skill-loader/tests/coffee_skill_load.rs) |
| Skill package digest/signature contract、trusted publisher allowlist、quarantine、validate supply-chain report | [`integrity.rs`](../../crates/skill-loader/src/integrity.rs)、[`coffee_skill_load.rs`](../../crates/skill-loader/tests/coffee_skill_load.rs)、[`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs)、[`commands.rs`](../../crates/dock-cli/src/commands.rs) |
| Skill registry/cache contract：local/package URL/registry id reference、digest-keyed cache、source/cache digest verify、readonly cache、latest/pinned/prerelease/rollback、rollback pin eviction、cache audit redaction、cleanup dry-run/report、publisher/merchant/Skill/version/digest scope cleanup、rollback/active retain protection、quarantine reload fail-closed | [`registry.rs`](../../crates/skill-loader/src/registry.rs)、[`skill_registry_cache.rs`](../../crates/skill-loader/tests/skill_registry_cache.rs) |
| Runtime config / secret boundary：`dock.runtime.config.v1` schema、profile、load priority、provider handle、secret reference、production blockers、redacted diagnostics | [`config.rs`](../../crates/dock-core/src/config.rs)、[`runtime_config.rs`](../../crates/dock-core/tests/runtime_config.rs) |
| Token cache persistence contract：backend profile、restart restore、expiry/revocation/replay/scope/trust rejection、redacted restore report、dev-only in-memory backend | [`token.rs`](../../crates/anp-adapter/src/token.rs) |
| Scoped storage persistence contract：DID/merchant/Skill/namespace scope、backend profile、restart restore、quota rejection、invalid entry cleanup、remove/clear/delete scope、redacted restore report、local file backend dev-only | [`storage.rs`](../../crates/wx-compat/src/storage.rs)、[`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs) |
| Persistent audit sink contract：backend profile、redacted export/retention report、Runtime persistent reader、corrupt backend `audit_unavailable`、L3/L4 executor 前 audit unavailable fail closed、local JSONL backend dev-only | [`audit.rs`](../../crates/consent-audit/src/audit.rs)、[`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs)、[`runtime_facade.rs`](../../crates/dock-core/tests/runtime_facade.rs) |
| Host adapter action contract：`dock.host-adapter.v1` capability declaration、headless mock dev-only、`api/call` 固定回 Orchestrator、Host action unsupported fail closed、`openDetailPage` safe relative target canonicalization、Host action redaction | [`host.rs`](../../crates/dock-core/src/host.rs)、[`runtime.rs`](../../crates/dock-core/src/runtime.rs)、[`host_adapter_contract.rs`](../../crates/dock-core/tests/host_adapter_contract.rs) |
| Runtime concurrency / cancellation / idempotency contract：`dock.runtime.concurrency.v1`、`RuntimeOperationOptions`、`runtime.cancelOperation`、session close fail-closed、高风险 in-flight 串行、低风险并发、显式 idempotency key forward/replay、RequestBroker 非幂等业务失败不自动重试 | [`runtime.rs`](../../crates/dock-core/src/runtime.rs)、[`runtime_facade.rs`](../../crates/dock-core/tests/runtime_facade.rs)、[`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) |
| Structured observability events / redaction：`dock.observability.event.v1`、`skill_load_*`、`api_call_*`、`consent_*`、`component_render_*`、`component_event`、`fallback_used`、`audit_record_written`、`sandbox_limit_hit`，`userDidHash` 默认 hash，事件 fields 不含 raw payload | [`observability.rs`](../../crates/dock-core/src/observability.rs)、[`runtime.rs`](../../crates/dock-core/src/runtime.rs)、[`runtime_facade.rs`](../../crates/dock-core/tests/runtime_facade.rs) |
| Metrics / tracing correlation：`dock.observability.metric.v1`、`dock.observability.trace.v1`、Runtime API latency、QuickJS VM execution time、render latency、`wx.request` status、fallback、consent、unsupported API、sandbox limit、token refresh/fail、audit metrics 和 trace spans，labels 低基数且默认脱敏 | [`observability.rs`](../../crates/dock-core/src/observability.rs)、[`runtime.rs`](../../crates/dock-core/src/runtime.rs)、[`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs)、[`runtime_facade.rs`](../../crates/dock-core/tests/runtime_facade.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| Performance baseline / stress smoke：`dock.perf-baseline-report.v1`、Skill load、API VM call、component render、Render IR size、token lookup、storage read/write、RSS memory sample、并发 session、多 Skill、多组件、dynamic timer/request、resource-limit fail-closed | [`commands.rs`](../../crates/dock-cli/src/commands.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs)、[`coffee-smoke-baseline.json`](../../testdata/perf/coffee-smoke-baseline.json) |
| Manifest component metadata、input `format:image/file`、production warning 分层 | [`mcp_validation.rs`](../../crates/mcp-schema/tests/mcp_validation.rs) |
| `dock-cli validate` 兼容报告、API 注册 mismatch blocker、demo-only release blocker | [`commands.rs`](../../crates/dock-cli/src/commands.rs)、[`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) |
| Atomic API sandbox、unsafe require、timeout、WebSocket/timer globals deny、Promise job drain、console/result size limit | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`bridge.rs`](../../crates/js-runtime-quickjs/src/bridge.rs)、[`api_vm.rs`](../../crates/js-runtime-quickjs/src/api_vm.rs) |
| Component sandbox、default no network/timer、native bridge hidden、snapshot size limit、expire 后事件失败 | [`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`component_vm.rs`](../../crates/component-runtime/src/component_vm.rs) |
| Component profile 默认 deny request/timer，dynamic 才可表达 request/timer boundary | [`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) |
| Request allowlist deny by default / miss deny without transport | [`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) |
| token scope isolation、HTTP Signature fallback、401 retry | [`capability_token_scope.rs`](../../crates/anp-adapter/tests/capability_token_scope.rs) |
| DID auth session cache 隔离、过期 refresh、clear/revoke 语义 | [`session.rs`](../../crates/anp-adapter/src/session.rs) |
| Atomic API `wx.login` receipt、`wx.checkSession`、JS auth header fail closed、response header redaction | [`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| deterministic unsupported API stub、sync throw、nested `wx.cloud.*`、unknown root fallback、safe reason/suggestion | [`unsupported.rs`](../../crates/wx-compat/src/unsupported.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| Atomic API scoped storage JS bridge、sync/async shape、DID/merchant/skill scope、JSON-safe validation、model-visible 隔离 | [`storage.rs`](../../crates/wx-compat/src/storage.rs)、[`scoped_storage.rs`](../../crates/wx-compat/tests/scoped_storage.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| Atomic API 与 Component VM device/app info 最小字段、防指纹和 shared default 防漂移 | [`model_context.rs`](../../crates/wx-compat/src/model_context.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs) |
| Atomic API 高风险 Host boundary：无 provider fail closed、未 consent 不执行 provider、dev-only mock 标识、opaque handle、本地路径拒绝、payment 不收集密码 | [`high_risk.rs`](../../crates/wx-compat/src/high_risk.rs)、[`high_risk_provider.rs`](../../crates/wx-compat/tests/high_risk_provider.rs)、[`middleware_chain.rs`](../../crates/js-runtime-quickjs/tests/middleware_chain.rs) |
| Component dynamic request/timer 最小 gate：`scope.dynamic` 驱动注入、默认 deny、auth header deny、response header redaction、timer limit/clear/expire cleanup | [`component_vm.rs`](../../crates/component-runtime/src/component_vm.rs)、[`component_lifecycle.rs`](../../crates/component-runtime/tests/component_lifecycle.rs)、[`component_permissions.rs`](../../crates/wx-compat/tests/component_permissions.rs) |
| L3 payment consent、Host consent adapter、provider unavailable audit、persistent audit redaction | [`payment_requires_consent.rs`](../../crates/consent-audit/tests/payment_requires_consent.rs)、[`api_call_flow.rs`](../../crates/dock-core/tests/api_call_flow.rs) |
| CLI/demo redaction | [`coffee_order_flow.rs`](../../crates/dock-cli/tests/coffee_order_flow.rs) |

补充红线抽样：

```bash
rg -n "token|Authorization|signature|private key|ConsentGate|audit|sandbox|allowlist|fail closed" docs/security docs/runbook/release-gates.md
```

预期：命中安全文档、runbook、测试说明和 redaction 规则；不得发现真实 secret、真实 DID private key、真实 bearer token 或生产凭据。

观测事件 Gate：

```bash
cargo test -p dock-core observability
cargo test -p dock-core runtime_observability
```

通过标准：

- `ObservabilityEvent` 序列化包含 `schemaVersion: "dock.observability.event.v1"`、`eventId`、`kind`、`severity`、`traceId`、`sessionId`、`skillId`、`runtimeVersion`、`outcome`、`latencyMs` 和 redaction policy。
- Runtime flow 至少覆盖 Skill load、API call、ConsentGate、Render IR render、component event、audit written 和 timeout/limit 事件。
- `userDid` 不得明文出现在事件 JSON 中；只能出现 `userDidHash`。
- 事件 JSON 不得包含 raw token、`Authorization`、HTTP signature、private key path/material、手机号、真实地址、文件内容、精确位置或本机绝对路径。
- `wx_api_call_*` 和 `request_*` 若当前 Step 没有真实 emit hook，必须保持为 schema 支持的 planned bridge，不得在 release notes 中宣称已经覆盖完整 QuickJS / RequestBroker 链路。

Metrics / Tracing Gate：

```bash
cargo test -p dock-core metrics
cargo test -p dock-core trace
cargo test -p js-runtime-quickjs quickjs_executor_records_vm_request_and_token_metrics_with_trace
```

通过标准：

- `ObservabilityMetric` 序列化包含 `schemaVersion: "dock.observability.metric.v1"`、metric name、kind、value、labels、traceId 和 redaction policy。
- `TraceSpan` 序列化包含 `schemaVersion: "dock.observability.trace.v1"`、traceId、spanId、parentSpanId、kind、outcome、latencyMs、attributes 和 redaction policy。
- RuntimeService metrics 覆盖 API latency、render latency、component event、fallback、consent、unsupported API、sandbox timeout、audit record 和 skill load。
- QuickJS metrics 覆盖 Atomic API VM execution、`wx.request` status、`wx.login` / `wx.checkSession` token/session outcome。
- labels 只能包含低基数 API name、component path、risk level、outcome、reason、status bucket 等定位字段；不得包含 URL query、headers、HTTP body、raw arguments、DID 原文、capability token、Authorization、Signature、private key path/material、手机号、真实地址、文件内容、精确位置或本机绝对路径。
- Runtime / QuickJS 默认 no-op sink；`InMemoryMetricsSink` 只作为 test/local recorder。Prometheus/OpenTelemetry 或 Host exporter 接入必须另行 Review，不得在当前 gate 中误标为已完成。

Performance Baseline / Stress Gate：

```bash
cargo test -p dock-cli perf
cargo test -p dock-cli --test coffee_order_flow perf_smoke_reports_baselines_and_stress
cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1
python3 -m json.tool testdata/perf/coffee-smoke-baseline.json >/tmp/coffee-smoke-baseline.json
```

通过标准：

- `dock-cli perf` 输出 `dock.perf-baseline-report.v1`，`status = "ok"`，`commandStatus = "ok"`，`mode = "smoke"` 或 `full`。
- samples 至少覆盖 `skill_load`、`api_vm_call`、`component_render`、`render_ir_size`、`token_lookup`、`storage_read`、`storage_write`、`stress.concurrent_sessions`、`stress.multi_skill`、`stress.multi_component_render`、`stress.dynamic_timer_request`、`resource_limit.result_size_fail_closed`。
- `baseline` 记录 case 数、pass/fail 数、迭代数；`environment` 记录 commit、rustc、os、arch 和 working tree 是否 dirty；`resource.memoryPerVm` 使用 process RSS 抽样并标明不是 production SLO。
- `testdata/perf/coffee-smoke-baseline.json` 只作为本地 smoke baseline artifact 和 schema 样例；数值随硬件和负载变化，不得用于跨机器固定阈值。
- perf report 和 artifact 不得包含 token、Authorization、Signature、capabilityToken、private key material/path、本机绝对路径、手机号、真实地址、文件内容或精确位置。

### 4.2 Phase 3 required security gates

以下 gate 是 Phase 3 必须补齐的 release blocker。完成对应 Step 后，必须把实际命令、测试文件和残余风险回填到本 runbook、Threat Model、主 Plan 台账和 Step 文档。

| Gate | Step | 当前处理 | 完成后必须记录 |
|---|---|---|---|
| sandbox escape 专项回归集：constructor/prototype/process/fetch/WebSocket/timer/result size/console size | 03-02 | 已升级为本地 required release gate：Atomic API VM 禁用 WebSocket/timer globals，限制 Promise job drain、console 和 result size；Component VM 扩展 escape regression 并限制 snapshot output size；dynamic 例外仍只由 component capability profile 开放 | `cargo test -p js-runtime-quickjs sandbox`、`cargo test -p js-runtime-quickjs limit`、`cargo test -p js-runtime-quickjs console`、`cargo test -p js-runtime-quickjs invalid_atomic`、`cargo test -p js-runtime-quickjs pending_job`、`cargo test -p component-runtime sandbox`、`cargo test -p component-runtime dynamic`、`cargo test -p component-runtime snapshot_size`、`cargo test -p js-runtime-quickjs`、`cargo test -p component-runtime`；CI 自动化和 resource metrics 仍待 Phase 6 |
| permission policy engine：`Allow` / `Deny` / `Prompt` / `MockAllowed(dev_only)`，Host deny override，manifest permission，dynamic scope，merchant trust policy | 03-03 | 已补齐本地 required gate：`wx-compat::PermissionPolicyEngine` 统一 decision，Host deny override 优先，Host allow 不能替代 manifest/meta/dynamic 声明，mock provider 只能 dev/headless；`dock-core::AuditEvent` 记录脱敏 `permissionDecision` | `cargo test -p wx-compat permission`、`cargo test -p dock-core permission`；生产 Host policy UI/config、provider conformance 和 persistent audit 仍待 Phase 4/03-05 |
| network allowlist：scheme、host、port、path prefix、method、scope，默认 deny | 03-03 | 已补齐本地 required gate：`anp-adapter::NetworkAllowlistRule` 支持 scheme/host/port/path prefix/method/scope，默认空 policy deny，mismatch 在 transport 前失败 | `cargo test -p anp-adapter allowlist`；Host registry 配置来源、生产 transport 和 persistent request audit 仍待 Phase 4/03-05 |
| DID/token lifecycle：token claims version、refresh、revoke/logout、cache eviction、jti replay、challenge nonce 一次性、resolver cache/trust anchor | 03-04 | 已补齐本地 required gate：`CapabilityTokenLifecycleStore` / `InMemoryTokenLifecycleStore` 支持 revoke、expired prune 和 high-risk `ConsumeOnce` jti gate；`DidAuthSessionManager` 支持 revoke/logout 和 expired eviction；`ChallengeNonceStore`、`TrustedDidDocumentResolver` 覆盖 nonce 一次性、cache TTL、trust anchor、unknown/mismatch fail closed；demo-server 登录尝试开始即消费 challenge，服务端 bearer 校验检查 revoked jti | `cargo test -p anp-adapter token`、`cargo test -p anp-adapter session`、`cargo test -p anp-adapter challenge`、`cargo test -p anp-adapter`、`cargo test -p demo-server token`、`cargo test -p demo-server`、`cargo test -p js-runtime-quickjs wx_login`、`cargo test -p dock-cli --test coffee_order_flow`；生产 token cache/revocation restore、跨进程 replay store、DID network/rotation、secret store 待 Phase 4/6 |
| Host consent adapter、ConsentProof policy version/prompt digest/decision actor/timestamp/parameter digest | 03-05 | 已补齐本地 required gate：`consent-audit::HostConsentAdapter`、`dock-core::HostConsentGateAdapter`、`ConsentProof` policy/prompt/actor/digest 字段；provider unavailable fail closed 并记录 blocked consent audit | `cargo test -p consent-audit consent`、`cargo test -p dock-core consent`；真实 Host UI/conformance 待 Phase 4 |
| persistent audit sink、retention、query/export redaction | 03-05 / 04-07 | 03-05 已补齐本地 `FileAuditSink` JSONL 持久化；04-07 已补齐 `AuditPersistenceProfile`、`AuditExportReport`、`AuditRetentionReport`、Runtime persistent reader、corrupt backend `audit_unavailable` 和 L3/L4 executor 前 unavailable fail-closed gate；`localFileJsonl` 明确 `productionReady = false` | `cargo test -p consent-audit audit`、`cargo test -p dock-core audit`；部署级 Host/encrypted audit backend、backend config、migration、access control、export approval、privacy deletion 和 durability/alerting 待 Phase 4/6 |
| Skill package digest/signature、publisher DID、trusted publisher allowlist、quarantine、remote require/path/symlink/zip slip deny | 03-06 | 已补齐本地 required gate：`skill-loader` 计算 normalized package `sha256` digest，`mcp-schema` 校验 `_meta.anp.supplyChain` contract，production integrity policy 对 unsigned、digest mismatch、signature mismatch、unknown publisher fail closed/quarantine，QuickJS CommonJS 拒绝 remote require，`dock-cli validate` 输出 redacted supply-chain report 和 release blocker；未签名本地包仍是 dev/demo-only | `cargo test -p skill-loader package`、`cargo test -p mcp-schema -p dock-cli validate`、`cargo test -p js-runtime-quickjs remote_require_is_rejected`、`cargo run -p dock-cli -- validate examples/coffee-skill`；真实 registry/cache、生产签名 verifier、publisher allowlist 配置来源和 CI 自动化仍待 Phase 4/6 |

### 4.3 Phase 4/5 后续 security gates

以下 gate 依赖生产 Host、registry/cache、持久化配置、开发者工具或 CI 自动化。它们仍是 production release blocker，但不应在 Phase 3 前半段误标为已自动化。

| Gate | 启用阶段 | 当前处理 |
|---|---|---|
| production Host RequestBroker transport、registry allowlist、request audit persistence | Phase 4 | Step 01-04 已把 Atomic API `wx.request` 收敛到 `wx-compat::RequestBroker` trait 的 loopback DID broker；Step 02-05 已给 dynamic component request 接入 injected broker boundary；Step 04-03 已冻结本地 registry/cache contract，但真实远端 registry download、Host registry allowlist 和 request audit persistence 仍未完成；demo-only/unsupported transport 仍不得 production release |
| 真实远端 registry download、生产签名 verifier、publisher trust policy 配置来源 | Phase 4/6 | Step 03-06 已完成 package integrity gate；Step 04-03 已完成本地 registry/cache/version/rollback gate；Step 04-08 已完成本地 cleanup/quarantine lifecycle gate。真实 HTTPS/DID registry discovery/download、生产签名算法 verifier、publisher allowlist 配置来源、部署级 quarantine feed、CI release report 仍是 production release blocker。 |
| Host provider conformance：phone/address/location/file/payment/scan/phone call/share/detail page | Phase 4 | Step 04-09 已冻结 Host adapter capability/action contract、`api/call` Orchestrator boundary、Host action unsupported fail-closed 和 detail-page canonicalization；真实 provider UI、least-privilege field shape、生产 renderer 和 provider E2E 仍是 production release blocker。 |
| distributed concurrency / durable idempotency / provider cancellation | Phase 4/6 | Step 04-10 已冻结本地 RuntimeService 内的 session 关闭、pre-dispatch cancellation、deadline check、高风险 in-flight 串行、显式 idempotency key forward/replay 和非幂等业务 no-retry gate；跨进程/跨 Host lock、merchant/provider 侧耐久 idempotency store、同步 executor 抢占式取消、真实 Host background lifecycle 和 metrics/alerting 仍是 production release blocker。 |
| secret store、token cache 持久化、scoped storage 持久化、audit retention/export 配置化、Skill cache cleanup | Phase 4 | Step 04-04 已冻结 runtime config schema、secret reference、provider/path handle、production profile release blockers 和 redacted diagnostics；Step 04-05 已冻结 token cache persistence trait、restore policy、redacted report 和 dev-only in-memory backend gate；Step 04-06 已冻结 scoped storage persistence trait、namespace scope、quota/restore/delete-scope/redaction gate 和未加密 local file dev/test backend；Step 04-07 已冻结 audit profile、redacted export/retention report、Runtime persistent reader 和 L3/L4 audit unavailable fail-closed gate；Step 04-08 已冻结 Skill cache cleanup sidecar metadata、dry-run report、scope cleanup、rollback protection 和 quarantine fail-closed gate。真实 secret resolve、生产 Host secure store/encrypted token/storage/audit backend、storage/audit migration/access control、export approval、privacy deletion、CLI/ops cache cleanup command 仍由后续 Host/ops gate 承接 |
| CLI compatibility / inspect / test-skill / import / doctor 报告 schema、developer self-certification | Phase 5 | Step 05-01 已固定 `dock-cli validate` 报告 schema `dock.validate-report.v1`，输出顶层和嵌套 compatibility report、repair suggestions、release readiness 和 redacted local path；Step 05-02 已固定 `dock-cli inspect` 报告 schema `dock.inspect-report.v1`；Step 05-03 已固定 `dock-cli test-skill` 报告 schema `dock.test-skill-report.v1`，复用 RuntimeService / Component Runtime 执行 coffee 与 Render IR fixture gate，并明确 headless provider 为 dev-only；Step 05-04 已固定 `dock-cli import-wechat-mcp` 报告 schema `dock.import-wechat-mcp-report.v1`，默认 dry-run，safe copy 拒绝 symlink/unsafe overwrite/source-dest 包含关系，并只输出 ANP patch 建议；Step 05-05 已固定 `dock-cli doctor` 报告 schema `dock.doctor-report.v1`，覆盖 toolchain/workspace/runtime config/DID/signing credential permission/resolver/allowlist/storage/audit/Host provider/sandbox/server health，`skip` 不计 pass，`--ci` 仅在 fail 时返回非零；Step 05-06 已将 `examples/fixtures/address-form`、`media-review`、`dynamic-status`、`location-map-preview` 整理为开发者可运行示例，补 README、expected JSON、snapshot 证据和 `cargo test -p dock-cli example` 回归 |
| CI/CD 自动 gate runner、link checker、matrix schema checker、snapshot gate、privacy deletion runbook | Phase 6 | Step 06-04 已新增 `scripts/release-gates.sh` 本地 runner、Markdown link checker、矩阵状态 checker、snapshot / fixture / perf / redaction gates 和 `dock.release-gates-report.v1`；Step 06-05 已新增 release/canary/rollback runbook 和本地 release notes dry-run；Step 06-06 已新增 operations、troubleshooting 和 privacy deletion runbook。真实 production Host secure store、encrypted storage/audit backend、deploy platform 和 provider conformance 命令仍是生产接入 blocker。 |

## 4.4 Canary / Rollback Gate

每次 release candidate 必须先通过：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

通过标准：

- release notes completeness gate 为 `pass`，不能是 `skip`。
- `releaseDecision = "pass"` 才能进入下一 canary stage。
- `token leakage`、`consent bypass`、`sandbox escape`、`Authorization` / `Signature` leakage、audit failure、Host crash 或 package integrity mismatch 均为 hard stop。
- rollback plan 必须记录 runtime revert、Skill version rollback/disable、cache purge、token revoke、rollout stop 和 audit evidence retention。
- Stage 1-3 没有真实 Host/deploy 平台证据时，release notes 必须明确为 blocked，不得宣称 production rollout 完成。

## 5. 兼容矩阵 Gate

每次新增或改变 `wx.*`、`wx.modelContext`、Component Runtime、Render IR、Host provider 或 fallback 行为时，必须同步更新：

- [`wx-api-compatibility-matrix.md`](../architecture/wx-api-compatibility-matrix.md)
- [`component-compatibility-matrix.md`](../architecture/component-compatibility-matrix.md)
- 相关 Step 文档和 roadmap 执行台账

检查项：

| 检查项 | 通过标准 |
|---|---|
| 状态准确 | demo-only、host-boundary、planned 和 supported 与源码/测试一致 |
| owner 明确 | 每项 planned/supported 有 owner crate 或 Host adapter |
| high-risk 明确 | L3/L4 API 或 action 写明 ConsentGate/audit/fail closed |
| unsupported 明确 | 不支持能力有 reason/suggestion，不出现 silent success |
| Phase 决策点 | callback/Promise、Render IR schema、dynamic、Host provider 等待决策项可追踪 |

## 6. Fixture 与 Render IR Gate

当前必须执行：

```bash
cargo test -p component-runtime
cargo test -p component-runtime snapshot
cargo test -p dock-cli fixture
cargo test -p dock-cli example
cargo test -p dock-cli --test coffee_order_flow
cargo run -p dock-cli -- test-skill examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
```

当前通过标准：

- coffee 三组件能 mount、render、dispatch tap、触发 `api/call` 和 expire。
- `preview-component` 和 `call-api` render payload 输出 `schemaVersion: "dock.render-ir.v1"`。
- component manifest `relatedPage`、`scope.dynamic`、`expirable`、`expiredText` 进入 redacted runtime metadata / validate report，且不进入 JS state 或 model-visible result。
- render failure 可以 fallback 到 CardSpec，并输出稳定 fallback reason enum string。
- dynamic 组件只有声明 `scope.dynamic` 后才注入受限 request/timer；默认 deny、auth header deny、timer limit/clear/expire cleanup 已有 focused tests。
- address-form、media-review、dynamic-status、location-map-preview fixture packages 可被 `dock-cli validate` / `inspect` / `test-skill` 读取，且每个 fixture 有 README、expected JSON summary 和 Render IR snapshot。
- `dock-cli test-skill` 对 coffee 输出 3 个 pass case，对 Render IR fixtures 输出 snapshot `match`，并在 report 中标记 headless/mock provider `productionReady = false`。
- `testdata/render-ir/*.json` golden snapshots 包含 render、actions、warnings、metadata、state 和 audit summary，且不含真实 token、Authorization、signature、private key path、本机路径、手机号、真实地址、经纬度。

Planned gates：

| Gate | 启用阶段 |
|---|---|
| Host renderer unknown node/action conformance | Phase 4 |

## 7. Demo-only 禁止项

以下能力不得作为 production-ready 发布：

- localhost `wx.request` bridge。
- 无 Host DID 配置时 `wx.login` 返回 `dock-login-code-localhost` 的 fallback。
- mock coffee payment / mock merchant data。
- CLI auto approval / `DecisionConsentProvider::approved()` 作为生产 consent。
- in-memory token/storage/audit 作为生产持久化；token cache 的 `inMemoryDev` profile、scoped storage 的 `localFileUnencrypted` profile、audit 的 `localFileJsonl` profile 只能作为 dev/test/local evidence backend。
- production profile 中启用 mock provider、dev-only Host provider、inline secret 或缺失 required provider。
- FastAPI/Rust demo 默认 token issuer secret。
- Mac app `dock-cli` process boundary 作为稳定 Host SDK。

若 release 范围仍包含这些能力，release notes 必须写成 demo/dev only，且不得面向生产用户启用。

## 8. 回滚与失败处理

| 失败类型 | 处理 |
|---|---|
| 基础命令失败 | 停止 release；修复代码或明确环境问题后重跑。 |
| redaction 失败 | 立即阻塞；删除泄露输出，补回归测试，再重跑全量安全相关测试。 |
| consent bypass | 阻塞；修复 Orchestrator/permission/Host provider，补 L3/L4 测试。 |
| allowlist / network deny 失败 | 阻塞；禁止发布任何网络相关变更。 |
| matrix 与实现不一致 | 阻塞当前 Step；先修矩阵或实现，再 Review。 |
| 后续阶段 gate 未实现 | 不阻塞当前 Step 的文档基线，但必须记录为 planned gap 或后续阶段 release blocker；进入对应 Phase 前升级为 required。 |
| demo-only 被误标 production-ready | 阻塞；修正文档、配置或代码路径。 |

## 9. Release Review Checklist

发布前 Review 必须确认：

- 当前 commit range 中每个 Step 都有 focused commit、Review evidence、verification evidence 和 commit hash。
- `git status --short --branch` 干净，或所有剩余改动都有明确归属和风险记录。
- 所有新增 API/组件/Host/provider 文档已同步到兼容矩阵。
- 所有 L3/L4 行为有 ConsentGate/audit/fail closed 证据。
- 所有 demo-only/mock 行为不会进入 production profile。
- 用户可见输出、日志、audit、Render IR 和 model-visible result 已检查敏感字段。
- 若最终全局 Review 修改文件，必须再执行相关验证并创建单独最终集成 commit。
