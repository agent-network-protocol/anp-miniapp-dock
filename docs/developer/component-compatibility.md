# 组件兼容说明

本说明帮助开发者理解 `anp-miniapp-dock` 的 MiniApp MCP Component Runtime。完整清单以 [`../architecture/component-compatibility-matrix.md`](../architecture/component-compatibility-matrix.md) 为准。

## 容器边界

组件运行时目标是把交易型 MiniApp MCP 原子组件编译和执行为平台中立 Render IR，不是完整微信小程序页面 Runtime。当前不复刻 TabBar、完整页面路由、广告、社交能力、完整地图交互、完整 canvas script 或完整 WXML/WXSS。

组件输出的 Render IR 是数据结构。高风险动作不能由 Render IR 或 Host 直接执行；`api/call` 必须回到 Runtime Orchestrator。

## 状态枚举

组件文档、矩阵和 CLI report 使用同一组状态：

| 状态 | 开发者含义 |
|---|---|
| `supported` | 当前 runtime 已支持核心语义，并有 tests、fixtures 或 snapshots。 |
| `host-boundary` | Runtime 输出了 action、Render IR、trait 或 metadata，但真实 Host renderer/provider/card manager 仍需实现。 |
| `planned-p1` | 近期计划能力，当前不能依赖。 |
| `planned-p2` | 后续计划能力，迁移时应准备 fallback。 |
| `demo-only` | 仅本地 CLI/headless/demo 可用。 |
| `unsupported-by-design` | 不进入当前容器边界，默认 fallback 或 fail closed。 |

## 当前支持的组件主线

当前 Runtime 支持：

- `Component({})` 单组件定义；
- `data`、`properties`、`methods`、`created`、`attached`、`detached`；
- `this.setData()` 和 dotted path；
- `wx.modelContext.getContext(this)`、`getViewContext(this)`；
- `NotificationType.Input` / `Result` / `Expire`，`Overflow` 仍需要真实 Host renderer 测量；
- tap、image load、image error 事件；
- `sendFollowUpMessage`、`api/call`、`expirePreviousCards`、`expireAllCards`、`openDetailPage` action boundary；
- Render IR `schemaVersion: "dock.render-ir.v1"`；
- fallback reason enum 和 CardSpec fallback。

WXML/WXSS 支持的是受控子集：

- `view`、`text`、`image`、`button`、横向 `scroll-view`；
- `input`、`textarea`、`radio`、`checkbox`、`picker`；
- static `map` preview 和 static `canvas`；
- `{{ path }}` binding、`.length`、文本/属性插值；
- `wx:if`、`wx:elif`、`wx:else`、`wx:for`、`wx:key`；
- `bindtap` / `catchtap`、disabled button action 抑制；
- class/id/tag/simple descendant WXSS selector 和基础 style 字段。

复杂表达式、任意 JS、template/import/include、slots、完整自定义组件系统、复杂动画和完整 CSS 级联都不属于当前生产边界。

## Dynamic component

Dynamic 组件默认没有网络和 timer 能力。只有 manifest 声明 `permissions.scope.dynamic`，且通过 permission/sandbox/resource gate 后，Runtime 才会注入受控能力：

- `wx.request` 仍必须经过 RequestBroker、allowlist、token boundary、header redaction 和 audit summary；
- JS 传入 `Authorization`、`Signature`、`Signature-Input`、`Cookie` 必须 fail closed；
- timer 有数量限制，`clearTimeout` / `clearInterval` 生效，expire/detach 时必须 cleanup；
- 真实生产 Host transport、background scheduler、request audit persistence 和 resource metrics 仍是 release blocker。

验证示例：

```bash
cargo run -p dock-cli -- test-skill examples/fixtures/dynamic-status
```

## Host action 边界

Host 可以消费 Render IR 并处理非 API action，但不能绕过 Runtime：

| Action | 边界 |
|---|---|
| `api/call` | 固定回 `RuntimeService::call_api()`，重新经过 input schema、permission、ConsentGate、audit、concurrency 和 executor。Host adapter 不允许直接执行。 |
| `sendFollowUpMessage` | Host/Agent 消息层展示或上行；内容不得携带 token、Authorization、private key path 或隐私原文。 |
| `openDetailPage` | 只接受 Runtime canonicalized safe relative target；外部 URL、`javascript:`、`file:`、traversal 和敏感 query 都必须拒绝。 |
| `expirePreviousCards` / `expireAllCards` | 由 Runtime/Host card manager 处理，不能扩大到未声明可过期组件。 |

真实 Host renderer 不支持的 node/action 必须 fallback 或返回 unsupported outcome，不能 silent success。

## Fixture 与 snapshot

开发者可用以下 fixtures 做迁移参考：

```bash
cargo run -p dock-cli -- validate examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
cargo run -p dock-cli -- validate examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
cargo run -p dock-cli -- validate examples/fixtures/location-map-preview
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
```

每个 fixture 有 README、`expected-test-skill.json` 和 `testdata/render-ir/*.json` snapshot。Snapshot 必须保持稳定，不应包含 raw token、Authorization、signature、private key path、本机路径、手机号、真实地址、文件内容或精确经纬度。
