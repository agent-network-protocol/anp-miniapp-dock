# Host Adapter Guide

本指南说明 Host 如何接入 `anp-miniapp-dock` Runtime。它面向真实 Host adapter 开发者，而不是 coffee demo 或 headless CLI harness。当前 headless adapter 只是 conformance/dev evidence，`productionReady = false`。

## Runtime 接入面

当前 Runtime API version 是 `dock.runtime.v1`。非 Rust Host 可以先通过 headless local process envelope 验证流程：

```bash
cargo run -p dock-cli -- runtime-json examples/coffee-skill \
  '{"apiVersion":"dock.runtime.v1","requestId":"req-1","method":"runtime.negotiateVersion","params":{}}'
```

当前 IPC method 与 Runtime facade 对应如下：

| Method | Runtime facade |
|---|---|
| `runtime.negotiateVersion` | version negotiation |
| `runtime.validateSkill` | validate Skill package |
| `runtime.loadSkill` | load Skill summary |
| `runtime.hostContract` | query `dock.host-adapter.v1` capability contract |
| `runtime.concurrencyPolicy` | query `dock.runtime.concurrency.v1` |
| `runtime.callApi` | call Atomic API through Orchestrator |
| `runtime.renderComponent` | render component to Render IR |
| `runtime.dispatchComponentAction` | dispatch component action |
| `runtime.expireCards` | expire card filters |
| `runtime.getAuditRecords` | read redacted audit records |
| `runtime.cancelOperation` | mark operation cancellation token |
| `runtime.closeSession` | close local runtime session |

当前 `runtime-json` binding 是 `headless-cli-json` / `local-process-stdio`。它不是 HTTP/gRPC sidecar，也不是完整生产 Host SDK。真实 sidecar、socket security、process lifecycle、deployment config 和 Host UI provider 仍需后续生产接入。

## Host contract

Host 必须通过 `runtime.hostContract` 查询或声明 `dock.host-adapter.v1` capabilities。Capability 应区分 required、optional、unsupported-by-design 和 dev-only。未声明能力默认 unsupported。

Host action contract 当前覆盖：

| Action | Host 责任 |
|---|---|
| `sendFollowUpMessage` | 将受控内容交给 Host/Agent 消息层，并做 redaction。 |
| `openDetailPage` | 只处理 Runtime canonicalized safe relative target；不能打开外部 URL、`javascript:`、`file:`、traversal 或敏感 query。 |
| `expirePreviousCards` / `expireAllCards` | 交给 Host card manager 或返回 unsupported；不得影响未授权卡片。 |

`api/call` 不是 Host action。它必须固定回到 Runtime Orchestrator：

```text
component action -> RuntimeService::dispatch_component_action -> RuntimeService::call_api -> Orchestrator -> permission -> ConsentGate -> audit -> executor
```

Host 不允许直接把 `api/call` 变成 Skill API 调用、HTTP 请求、支付、电话、地址、位置、文件、扫码或任何高风险系统调用。

## Provider 边界

真实 Host provider 应覆盖目标产品需要的能力：

- consent prompt；
- phone/address/location/media/file/payment/scan/phone call/share/detail page provider；
- Render IR renderer；
- CardSpec fallback renderer；
- secure DID identity provider；
- encrypted token/storage/audit/cache backend；
- request transport and allowlist source；
- observability sink and release gate integration。

高风险 provider 必须返回最小化结果或 opaque handle，并写入 audit。provider unavailable、policy deny、consent deny、audit unavailable 都必须 fail closed。

## Redaction

Host adapter 需要在 Runtime redaction 后做出口二次检查。不得输出：

- token、Authorization、Signature、Signature-Input、Cookie；
- DID private key、credential path、private key material；
- phone/address/file content/location raw payload；
- local absolute paths or production secrets。

Host 侧 log/metrics/tracing 只能记录定位需要的 metadata，例如 traceId、sessionId、skillId、apiName、componentPath、outcome、latencyMs 和 hashed user DID。URL query、headers、request body、provider raw result 不能作为 label。

## Conformance checklist

接入真实 Host 前，至少执行：

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- inspect examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
cargo run -p dock-cli -- doctor
```

然后按 [`../runbook/release-gates.md`](../runbook/release-gates.md) 执行 release gates，并记录 pass/fail/skip、skip 原因和 residual risk。

## 当前 release blockers

以下能力不能因为 headless/demo 通过就写成 production-ready：

- production Host UI/provider conformance；
- HTTP/gRPC sidecar 或稳定 Host SDK；
- encrypted token/storage/audit/cache backend；
- production DID resolver/trust anchor and secret store；
- remote registry download and production signature verifier；
- distributed lock and durable idempotency store；
- observability/metrics/tracing/release gate automation；
- privacy deletion and operations runbook。

这些 blocker 在后续 Phase 6 和生产 Host 接入任务中继续收敛。
