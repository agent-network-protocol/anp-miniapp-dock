# 安全开发指南

本指南面向 Skill 开发者、Host adapter 集成方和维护 CLI/report 的开发者。权威安全边界以 [`../security/threat-model.md`](../security/threat-model.md)、[`../runbook/release-gates.md`](../runbook/release-gates.md)、API 矩阵和组件矩阵为准。

## 默认红线

以下内容不得进入 model-visible output、`structuredContent`、Render IR、CLI JSON、日志、metrics label、trace label、audit export、snapshot 或开发者文档示例：

- raw token、capability token、refresh token；
- `Authorization`、HTTP Signature、`Signature`、`Signature-Input`、Cookie；
- DID private key、private key path、PEM material、credential file path；
- 真实手机号、地址、文件内容、精确位置或经纬度；
- 真实用户数据、真实商家 secret、真实 OpenAI/API key。

允许输出的是最小化、脱敏或 opaque 信息，例如 `[REDACTED]`、opaque file handle、opaque address handle、hashed user DID、policy id、audit id、snapshot id。

## Skill 侧规则

Skill JavaScript 不应保存或回传 secret：

- 不要把 token、Authorization header、DID proof、private key path 写入 `content`、`structuredContent` 或 `_meta`。
- 不要把手机号、地址、文件内容或位置放入模型可见字段；高风险 provider 应返回 opaque handle 或 Host-controlled summary。
- 不要在 `wx.request` options 中传入 `Authorization`、`Signature`、`Signature-Input` 或 `Cookie`。Runtime 会 fail closed。
- 不要依赖本地 absolute path。文件和媒体必须通过 opaque handle 或 Host provider。
- 不要把 demo/mock/headless provider 当成生产能力。

## 权限、ConsentGate 与 audit

L3/L4 能力必须走 Runtime/Host 边界：

| 能力 | 必须边界 |
|---|---|
| payment/order confirm/refund | permission decision、ConsentGate、audit、idempotency key。 |
| phone/address/location/media/file/scan/phone call | Host provider、explicit consent、least-privilege field shape、opaque handle、audit。 |
| dynamic component request/timer | manifest dynamic scope、permission policy、RequestBroker、allowlist、resource limit、cleanup。 |
| `openDetailPage` | Runtime canonicalization、Host policy、safe relative target、redacted query。 |

如果 provider 缺失、ConsentGate 不可用、audit sink 不可用或 policy deny，必须 fail closed。不得用 silent no-op、mock allow 或文档说明绕过。

## Host adapter 侧规则

Host adapter 必须：

- 通过 `runtime.hostContract` 声明 capabilities，未声明能力默认 unsupported。
- 让 `api/call` 回到 Runtime Orchestrator，不能在 Host 里直接调用 Skill API、支付、隐私 provider 或 merchant API。
- 对 `sendFollowUpMessage`、`openDetailPage`、`expirePreviousCards` 等 Host action 做二次 redaction。
- 对高风险 provider 执行 consent/audit，不把 raw provider result 直接传给 Skill 或模型。
- 区分 headless/dev/mock 和 production provider，`productionReady = false` 不能被改写成 pass。

## CLI/report 规则

CLI 输出是开发者证据，也是 CI/release gate 输入。新增 report 字段时：

- 输出相对路径、状态、schema、reason、suggestion，不输出源码内容和本机绝对路径。
- `status` / `reportStatus` 表示报告或 release-readiness；`commandStatus` 表示命令是否成功输出 JSON。
- `warning`、`skip`、`not-evaluated` 和 `demo-only` 不能写成 production-ready。
- failure diff 可以给 JSON path，但不能包含 token、headers、private path 或隐私原文。

建议每次文档或 report 变更后运行：

```bash
cargo run -p dock-cli -- validate examples/coffee-skill
cargo run -p dock-cli -- inspect examples/coffee-skill
cargo run -p dock-cli -- test-skill examples/coffee-skill
cargo run -p dock-cli -- doctor
```

再对输出和变更文件做敏感串抽样。

## 文档示例规则

文档示例必须使用 mock DID、mock URL、mock handle 或仓库相对路径。不要写真实用户数据、真实生产 URL query、真实 secret、真实本机路径或真实凭据文件路径。

可以使用：

```text
examples/coffee-skill
examples/fixtures/address-form
target/dock-import-coffee
did:wba:user.example
[REDACTED]
```

不应使用真实个人路径、真实手机号、真实地址、真实经纬度或可复用 secret。

## Release 前必须确认

- `validate` 没有把 demo-only、mock、local unencrypted backend 或 unsigned package 写成 production-ready。
- `inspect` 不输出源码内容、包外路径、token、Authorization、signature 或 private key path。
- `test-skill` report 和 Render IR snapshot 不含敏感 payload。
- `doctor` 的 warning/skip 没有被 CI policy 当成 pass。
- Host provider conformance、persistent audit/storage/token/cache、release gates 和 rollback runbook 已按目标环境完成。
