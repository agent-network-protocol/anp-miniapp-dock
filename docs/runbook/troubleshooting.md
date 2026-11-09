# Troubleshooting Runbook

> 状态：Step 06-06 故障处理索引。本文按故障域列出症状、观测信号、检查命令、处理步骤、升级路径和回滚/关闭条件。
> 上游计划：[`../plan/production-readiness-roadmap.md`](../plan/production-readiness-roadmap.md) Step 06-06。
> 相关文档：[`operations.md`](operations.md)、[`privacy-deletion.md`](privacy-deletion.md)、[`release-gates.md`](release-gates.md)、[`release-process.md`](release-process.md)。

## 1. 通用诊断命令

从 repository root 执行：

```bash
cargo run -p dock-cli -- doctor
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/coffee-skill
cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1
```

Release/canary 相关问题执行：

```bash
./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md
```

这些命令输出的 JSON 是本地 evidence。真实生产 Host 仍必须补充 Host-specific 健康检查、deploy 状态、traffic router 状态、secure store 状态和 provider conformance 证据。

## 2. 故障域矩阵

| 故障域 | 症状 | Event / Metric | 检查命令 | 处理步骤 | 升级 / 回滚条件 |
|---|---|---|---|---|---|
| DID 验签失败 | `wx.login`、challenge/login、signed request 返回 auth error；doctor identity/resolver fail。 | `dock.token_refresh.total{outcome="fail"}`、request auth failure span。 | `cargo run -p dock-cli -- doctor --did-document examples/identity/did_document.json --private-key examples/identity/key-1-private.pem` | 检查 DID document id、trusted resolver、challenge audience、clock skew、credential permission；确认输出不含 private key path/material。 | unknown DID、resolver mismatch 或 replay challenge 持续出现时停止 rollout，升级 Host identity owner。 |
| token scope mismatch | `wx.request` 或 merchant API 返回 scope/audience/session mismatch；`checkSession` fail。 | `dock.token_refresh.total`、`dock.request.total{status="4xx"}`。 | `cargo test -p anp-adapter token && cargo test -p anp-adapter session` | 检查 token claims version、issuer/audience、merchant DID、user DID、Skill id、session id、route scope、revocation/replay store；不要输出 raw token。 | 任意 scope 越权或 raw token 出现在报告中，立即 rollback/revoke affected token。 |
| allowlist deny | request 在 transport 前失败；非 allowlisted host/scheme/path/method/scope 被拒绝。 | `dock.request.total{outcome="fail"}`、permission decision audit。 | `cargo test -p anp-adapter allowlist`；`cargo run -p dock-cli -- doctor` | 检查 Runtime config allowlist provider、merchant registry policy、method/path prefix 和 component dynamic scope。 | allowlist 被绕过或 silent success 时 hard stop；升级 security owner。 |
| component render failed | Render IR 缺 node/action、fallback 到 CardSpec、snapshot mismatch。 | `component_render_start/end`、`dock.render_latency_ms`、`dock.fallback.total`。 | `cargo test -p component-runtime snapshot`；`cargo run -p dock-cli -- test-skill examples/fixtures/address-form` | 检查 WXML/WXSS、component path、manifest metadata、Render IR schemaVersion、fallback reason 和 Host renderer support。 | Host crash、unknown action silent success 或 sensitive Render IR output 时 rollback。 |
| sandbox timeout/resource hit | API VM 或 Component VM timeout、result size limit、timer limit、snapshot size limit。 | `sandbox_limit_hit`、`dock.sandbox_limit.total`。 | `cargo test -p js-runtime-quickjs sandbox && cargo test -p component-runtime sandbox` | 确认 timeout/resource limit fail closed；检查是否有 dynamic request/timer 扩权；不要临时放宽限制发布。 | sandbox escape、resource-limit fail-open 或 native bridge exposed 时 hard stop。 |
| storage quota exceeded | `wx.setStorage` fail，restore rejection reason 为 quota exceeded。 | storage error、`dock.api_call.total{outcome="fail"}`。 | `cargo test -p wx-compat storage` | 检查 `StorageScope = user DID + merchant DID + Skill id + namespace`，确认 key/value size 和 aggregate quota；用 scope-level deletion 前先做 privacy/delete approval。 | 跨 scope 读取、quota fail-open 或 raw storage value 进入 report 时 stop rollout。 |
| audit sink unavailable | L3/L4 API 返回 `audit_unavailable`，`runtime.getAuditRecords` 失败。 | `dock.audit_record.total{outcome="fail"}`、audit span error。 | `cargo test -p consent-audit audit && cargo test -p dock-core audit` | 检查 audit backend profile、write permission、corrupt JSONL/encrypted DB、retention/export config；高风险 executor 前必须 fail closed。 | audit unavailable 影响 L3/L4 时停止相关高风险动作；不得删除 incident window audit。 |
| Host provider unavailable | phone/address/location/file/payment/scan/openDetailPage 等 Host boundary 返回 unavailable/unsupported。 | consent decision、Host action span、fallback count。 | `cargo run -p dock-cli -- doctor`；Host-specific provider conformance command。 | 检查 `dock.host-adapter.v1` capability declaration，确认 unsupported-by-design 与 required provider 区分，mock provider 不能 production-ready。 | 高风险 provider silent success、mock provider 进入 production profile 或 consent bypass 时 rollback。 |
| merchant Agent unavailable | merchant `/health` fail、business API 5xx/timeout、request retry exhausted。 | `dock.request.total{status="5xx"}`、request latency、auth failure spike。 | `cargo run -p dock-cli -- doctor --server http://127.0.0.1:3000` | 检查 merchant health、allowlist、DID audience、route scope、timeout/retry/idempotency policy。 | 5xx/error spike 影响 canary 时 stop rollout；非幂等 API 不做自动业务重试。 |
| Skill package signature mismatch | validate/load 报 digest/signature/publisher mismatch，cache quarantine。 | supply-chain status、release gate package tests。 | `cargo test -p skill-loader package`；`cargo run -p dock-cli -- validate examples/coffee-skill` | 检查 package digest、publisher DID、trusted allowlist、registry cache metadata、quarantine reason；不要复用 quarantined cache。 | signature mismatch、unknown publisher 或 digest mismatch 是 release blocker；按 rollback/cache purge 流程处理。 |
| rollback/cache purge | release gate blocked、fallback spike、auth/audit/sandbox hard blocker。 | release gate report、canary metrics。 | `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md` | 使用 [`release-process.md`](release-process.md) stop rollout、runtime revert、Skill version rollback/disable、cache purge dry-run、token revoke、audit preservation。 | 任一 hard blocker 立即 stop rollout；真实 purge 前必须保留 rollback pin 和 audit evidence。 |

## 3. 关闭条件

每个故障关闭前必须满足：

- 影响 scope 已记录，且不包含隐私原文。
- 根因归类到 Skill、merchant Agent、Host provider、identity/token、storage/audit/cache 或 container runtime。
- 对应命令或 Host-specific gate 已通过；无法运行的命令有原因、影响和替代证据。
- release/canary 问题已经重跑 release gate。
- 需要 privacy deletion 时已按 [`privacy-deletion.md`](privacy-deletion.md) 执行或记录合法保留原因。

## 4. Redaction Review

排障材料中不得出现：

- raw capability token、bearer value、`Authorization`、`Signature`、private key material/path、merchant secret；
- 手机号、真实地址、文件内容、精确经纬度；
- 本机私有绝对路径；
- raw consent proof 或未脱敏 Host provider payload。

可以出现：

- schema version、status、error code、risk level、scope summary、hash/digest、redaction marker、mock/demo-only 标注；
- 文档中的 forbidden marker 名称，但不能是实际值。
