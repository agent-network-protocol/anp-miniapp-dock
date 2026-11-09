# Operations Runbook

> 状态：Step 06-06 production-readiness 运维入口。本文提供通用诊断、事件处理、发布门禁和升级流程；真实部署平台、Host rollout UI、生产 secret store、加密 storage/audit backend 和生产 cache purge CLI 仍需由具体 Host/deploy 文档补充。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 06-06。
> 相关文档：[`troubleshooting.md`](troubleshooting.md)、[`privacy-deletion.md`](privacy-deletion.md)、[`release-gates.md`](release-gates.md)、[`release-process.md`](release-process.md)、[`security.md`](security.md)。

## 1. 适用范围

本文适用于 `anp-miniapp-dock` runtime、`dock-cli`、QuickJS Atomic API VM、Component Runtime、ANP DID/token adapter、wx Compatibility Layer、Skill registry/cache、Host adapter contract 和本地 release gate runner 的运维。

非目标：

- 不替代真实生产部署平台的启动、扩缩容、密钥轮换或告警配置文档。
- 不提供真实用户 DID、手机号、地址、文件内容、精确位置、raw token、`Authorization`、`Signature`、private key material/path 或商家 secret 示例。
- 不把 headless CLI、demo server、mock provider、in-memory backend、`localFileUnencrypted`、`localFileJsonl` 或本地 performance baseline 写成 production-ready。

## 2. 运维原则

- 先保留证据，再修复或回滚；不得删除 incident window 的 audit evidence、release gate report、rollback decision、cache quarantine record 或 privacy deletion approval。
- 诊断输出默认脱敏；只记录 scope summary、schema version、status、metric label、trace id、session id、Skill id、publisher DID/merchant DID 的必要定位信息。
- 高风险动作 fail closed：consent bypass、audit unavailable、sandbox escape、token leakage、package integrity mismatch、Host provider unavailable 或 unsupported action silent success 都不能继续 rollout。
- 当前可执行命令以 repository root 为工作目录；Host-specific 命令必须在真实 Host/deploy runbook 中补齐后才能用于生产。

## 3. 快速分流

| 信号 | 首查命令 / 证据 | 主要文档 | 立即动作 |
|---|---|---|---|
| 发布前门禁或 canary 异常 | `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md` | [`release-gates.md`](release-gates.md)、[`release-process.md`](release-process.md) | `releaseDecision != "pass"` 时停止 rollout。 |
| 本地/Host 环境异常 | `cargo run -p dock-cli -- doctor` | [`troubleshooting.md`](troubleshooting.md) | 先看 `dock.doctor-report.v1` 中 fail/warn/skip，skip 不计 pass。 |
| Skill 行为或 fixture 回归 | `cargo run -p dock-cli -- test-skill examples/coffee-skill` | [`troubleshooting.md`](troubleshooting.md) | 对比 fixture report 和 Render IR snapshot。 |
| Runtime/API/Host envelope 异常 | `cargo run -p dock-cli -- runtime-json examples/coffee-skill '{"apiVersion":"dock.runtime.v1","requestId":"ops-req-1","method":"runtime.negotiateVersion","params":{}}'` | [`../plan/production-readiness/phase-4-runtime-host-integration.md`](../plan/production-readiness/phase-4-runtime-host-integration.md) | 检查 `apiVersion`、`requestId`、`status`、`error.code` 和 redaction marker。 |
| 隐私删除请求 | 使用 [`privacy-deletion.md`](privacy-deletion.md) 的 dry-run checklist | [`privacy-deletion.md`](privacy-deletion.md) | 先确认 scope 和 legal/audit retention，再删除 storage/cache/token。 |

## 4. 标准事件流程

1. 记录 incident id、发现时间、当前 commit、branch、release notes path、release gate report path 和 `git status --short --branch`。
2. 确认影响范围：user DID hash 或 redacted DID summary、merchant DID、publisher DID、Skill id、Skill version、session id、component path、API name、Host channel。
3. 运行最小安全诊断：

```bash
cargo run -p dock-cli -- doctor
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/coffee-skill
```

4. 如果是 release/canary 范围，运行：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

5. 对照 [`troubleshooting.md`](troubleshooting.md) 的故障域处理；若命中 rollback 条件，进入 [`release-process.md`](release-process.md) 的 rollback actions。
6. 修复后重跑对应 gate，记录 pass/fail/skip、残余风险和下一步 owner。
7. 关闭事件前确认没有新增敏感输出：

```bash
rg -n "Authorization|Signature|capabilityToken|Bearer |private key|phone|address|latitude|longitude" target/release-gates testdata/render-ir testdata/perf
```

预期：没有真实泄漏命中；若命中文档红线或 mock 字样，必须在事件记录中说明为什么不是泄漏。

## 5. 日常操作 Gate

| 操作 | 命令 / 检查 | 通过标准 |
|---|---|---|
| 工作区基础健康 | `cargo metadata --format-version 1 --no-deps`、`cargo fmt --check` | workspace 可解析且格式无 diff。 |
| 全量回归 | `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` | 全部通过，无 clippy warning。 |
| 本地环境诊断 | `cargo run -p dock-cli -- doctor` | JSON 可解析；`status = "warning"` 可作为本地默认状态，但 production rollout 需消除相关 blocker。 |
| Release gates | `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md` | `releaseDecision = "pass"`、`requiredFailed = 0`、`hardBlockerFailed = 0`、无 skip。 |
| Performance smoke | `cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1` | `dock.perf-baseline-report.v1` 可解析；数值只作为本地硬件相关 baseline。 |
| Fixture smoke | `cargo test -p dock-cli fixture`、`cargo test -p dock-cli example` | coffee 和四个 compatibility fixtures 通过。 |

## 6. 观测字段

排障时优先使用这些低基数字段：

| 类别 | 字段 / 指标 | 禁止内容 |
|---|---|---|
| Trace | `traceId`、`spanId`、`parentSpanId`、`sessionId`、`skillId`、`apiName`、`componentPath` | URL query、headers、body、raw arguments。 |
| Metrics | `dock.api_latency_ms`、`dock.request.total`、`dock.fallback.total`、`dock.consent.total`、`dock.sandbox_limit.total`、`dock.token_refresh.total`、`dock.audit_record.total` | raw DID、raw token、`Authorization`、`Signature`、private key path、隐私 payload。 |
| Events | `skill_load_*`、`api_call_*`、`request_*`、`component_render_*`、`fallback_used`、`audit_record_written`、`sandbox_limit_hit` | 手机号、地址、文件内容、精确经纬度。 |
| Reports | `dock.validate-report.v1`、`dock.inspect-report.v1`、`dock.test-skill-report.v1`、`dock.doctor-report.v1`、`dock.release-gates-report.v1` | 本机绝对路径、生产 secret、真实用户数据。 |

## 7. 升级路径

| 条件 | Owner | 升级动作 |
|---|---|---|
| DID/token/scope 失败 | Runtime/auth owner + Host identity owner | 检查 resolver/trust anchor、token restore policy、revocation/replay store 和 Host secure store。 |
| allowlist 或 merchant Agent unavailable | Runtime/network owner + merchant owner | 检查 allowlist source、Host RequestBroker、merchant health 和 request audit。 |
| render/component/action 失败 | Component Runtime owner + Host renderer owner | 检查 Render IR schema、fallback reason、Host adapter contract 和 unsupported action fail-closed。 |
| sandbox/resource hit | Runtime security owner | 停止 rollout，运行 sandbox gates，确认无 escape 或 fail-open。 |
| storage/audit/cache 问题 | Runtime persistence owner + ops owner | 检查 backend profile、quota、retention/export、cleanup dry-run 和 rollback pin。 |
| privacy deletion | Privacy owner + security owner + ops owner | 按 [`privacy-deletion.md`](privacy-deletion.md) 审批、dry-run、执行和留证。 |

## 8. 收尾 Checklist

| 项 | 记录 |
|---|---|
| incident id / release notes path / gate report path |  |
| affected scope summary |  |
| commands run and result |  |
| metrics/events/traces checked |  |
| rollback or stop rollout decision |  |
| privacy deletion required? |  |
| audit evidence retained/exported |  |
| residual risk and owner |  |
| final `git status --short --branch` or deploy state |  |
