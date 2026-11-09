# wx API 兼容说明

本说明帮助 Skill 开发者理解 `anp-miniapp-dock` 对 `wx.modelContext` 和关键 `wx.*` API 的支持口径。完整清单以 [`../architecture/wx-api-compatibility-matrix.md`](../architecture/wx-api-compatibility-matrix.md) 为准。

## 状态枚举

文档、矩阵和 CLI report 使用同一组状态：

| 状态 | 开发者含义 |
|---|---|
| `supported` | 当前 runtime 已支持核心语义，可以在本地 tests/fixtures 中验证。 |
| `host-boundary` | Skill 可以声明或触发边界，但真实能力必须由 Host provider、renderer、card manager 或持久化 backend 完成。 |
| `planned-p1` | 近期计划能力；当前应按缺口处理。 |
| `planned-p2` | 后续计划能力；迁移时应先准备 fallback 或替代路径。 |
| `demo-only` | 只适用于 coffee demo、localhost、mock/headless 或 dev fixture，不是生产能力。 |
| `unsupported-by-design` | 容器不会复刻该微信能力；应改造成 merchant Agent API、Host native capability 或 CardSpec fallback。 |

## 当前开发者可用主线

当前迁移 Skill 时，优先走以下能力：

- `wx.modelContext.createSkill()`、`registerAPI()`、`skill.use()`：用于 Atomic API 注册和 middleware。
- `wx.modelContext.getSessionId()`：返回 Runtime session id，不暴露 token、challenge id 或 credential path。
- `wx.login()` / `wx.checkSession()`：Host DID 配置下走 ANP DID session；无 Host DID 配置时的 localhost receipt 仍是 `demo-only`。
- `wx.request()`：只能通过 RequestBroker、allowlist、capability token boundary 和 redaction；当前生产 Host transport 和 request audit persistence 仍是 release blocker。
- `wx.getStorage()` / `wx.setStorage()` / `wx.removeStorage()` / `wx.clearStorage()` 及同步版本：按 user DID、merchant DID、Skill id 和 namespace 做 scoped storage；本地文件 backend 是 dev/test only。
- `wx.getDeviceInfo()` / `wx.getAppBaseInfo()`：只返回最小 runtime/host 信息，不返回真实设备指纹。

## 高风险 API

phone、address、location、media/file、payment、scan、phone call 等 L3/L4 能力都属于 `host-boundary`。默认规则是：

- 没有 Host provider 时 fail closed。
- 需要用户授权时必须经过 ConsentGate。
- 需要写入 audit，不允许绕过 Runtime。
- 返回值只能是最小化字段或 opaque handle，不能把手机号、地址、文件内容、精确位置、支付密码或本地路径交给 Skill。
- headless/mock provider 必须标记为 `dev-only` 或 `productionReady = false`。

对应示例：

```bash
cargo run -p dock-cli -- test-skill examples/fixtures/address-form
cargo run -p dock-cli -- test-skill examples/fixtures/media-review
cargo run -p dock-cli -- test-skill examples/fixtures/location-map-preview
```

这些示例只证明边界和 fallback，不证明真实 provider 已生产可用。

## Unsupported API 处理

不支持的 API 必须返回稳定 fail shape，而不是 `undefined is not a function` 或静默成功。典型返回：

```json
{
  "errMsg": "wx.cloud.callFunction:fail unsupported",
  "code": "unsupported",
  "reason": "wx.cloud.* is unsupported by anp-miniapp-dock production runtime",
  "suggestion": "Expose this capability as a merchant Agent API and call it through wx.request"
}
```

迁移时可以用：

```bash
cargo run -p dock-cli -- inspect <skill-dir>
cargo run -p dock-cli -- validate <skill-dir>
```

`inspect` 会报告静态 `wx.*` 使用痕迹；动态 property access 不能只靠静态扫描确认，必须继续运行 `test-skill` 或补 fixture。

## 常见迁移建议

| 原微信能力 | 推荐迁移路径 |
|---|---|
| 微信登录态 | 改为 ANP DID runtime session；token 由 Host/runtime 持有。 |
| 直接 HTTP / 云开发 | 改为 merchant Agent API，通过 `wx.request` / RequestBroker / allowlist 访问。 |
| 手机号、地址、文件、位置、支付 | 改为 Host provider + ConsentGate + audit + opaque handle。 |
| 微信生态能力、广告、社交、完整路由 | 改为 Host native flow、merchant Agent API 或明确 fallback。 |

上线前必须同时通过 `validate`、`inspect`、`test-skill`、`doctor`、release gates 和 Host provider conformance；单个 `supported` API 不代表整个 Skill production-ready。
