# Phase 0：基线冻结与产品化门槛实施计划

## 1. 阶段目标

Phase 0 不新增运行时能力，目标是把当前 P0.5 Demo 状态转成可持续开发的产品化基线。完成后，团队应该能清楚回答：当前哪些能力已实现、哪些只是 host boundary、哪些是 demo-only、每个后续 Phase 如何验收。

## 2. 输入依据

- 总体 roadmap：[`../production-readiness-roadmap.md`](../production-readiness-roadmap.md)
- PRD：[`../../architecture/agentic-miniapp-container-prd.md`](../../architecture/agentic-miniapp-container-prd.md)
- 系统架构：[`../../architecture/anp-skill-dock-architecture.md`](../../architecture/anp-skill-dock-architecture.md)
- 兼容方案：[`../../architecture/miniapp-mcp-compatibility-mvp.md`](../../architecture/miniapp-mcp-compatibility-mvp.md)
- 组件方案：[`../../architecture/miniapp-mcp-component-runtime.md`](../../architecture/miniapp-mcp-component-runtime.md)
- 小程序 MCP 本地参考：[`../../weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt`](../../weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt)
- 本地 demo runbook：[`../../runbook/local-demo.md`](../../runbook/local-demo.md)
- 安全 runbook：[`../../runbook/security.md`](../../runbook/security.md)

## 3. 输出物

Phase 0 应新增或更新以下文档：

| 文档 | 目的 | 最低内容 |
|---|---|---|
| `docs/architecture/wx-api-compatibility-matrix.md` | API 支持矩阵 | API 名、环境、状态、实现模块、风险等级、测试证据、备注 |
| `docs/architecture/component-compatibility-matrix.md` | 组件支持矩阵 | 内置组件、Component JS、WXML、WXSS、事件、动态组件、Render IR 状态 |
| `docs/security/threat-model.md` | 安全模型 | 攻击面、攻击者、控制措施、测试证据、残余风险 |
| `docs/runbook/release-gates.md` | 发布门槛 | 必跑命令、fixture、红线、回滚条件 |
| `docs/plan/production-readiness/backlog.md`（可选） | issue 拆分索引 | Phase、任务、依赖、DoD、验收命令 |

## 4. 开发顺序

### 4.1 冻结当前能力清单

1. 从 workspace crate 出发建立能力到模块的映射：
   - `mcp-schema`：manifest/result/validation；
   - `skill-loader`：Skill 包加载和路径边界；
   - `js-runtime-quickjs`：Atomic API VM 与 JS bridge；
   - `wx-compat`：host capability profile、request/storage/model context；
   - `anp-adapter`：DID、challenge、token、signed request；
   - `component-runtime`：Component VM、WXML/WXSS、Render IR；
   - `dock-core`：Orchestrator、permission、consent、audit、render routing；
   - `consent-audit`：risk policy、consent proof、redaction；
   - `dock-cli` / `demo-server`：demo 与开发者入口。
2. 对每项能力标注状态：
   - `implemented`：已注入 runtime 且测试覆盖；
   - `host-boundary`：crate/trait 已有，但 JS bridge 或生产 provider 未完成；
   - `demo-only`：仅 coffee demo / localhost 可用；
   - `planned`：roadmap 中规划但未实现；
   - `unsupported-by-design`：明确不做。
3. 每个状态必须有证据：源文件、测试文件、命令或文档链接。

### 4.2 API 兼容矩阵

1. 从小程序 MCP 本地参考抽取三类环境：
   - 原子接口环境；
   - 原子组件环境；
   - 半屏页面环境（只作为未来 Host fallback 参考）。
2. 字段建议：

```text
category | api | atomic_api_status | component_status | target_phase | runtime_mapping | risk_level | owner_crate | tests | notes
```

3. 状态枚举固定：

```text
supported | host-boundary | planned-p1 | planned-p2 | demo-only | unsupported-by-design
```

4. 关键映射必须显式写清：
   - `wx.login` → ANP DID challenge/login；
   - `wx.checkSession` → capability token/session validation；
   - `wx.request` → RequestBroker + allowlist + DID signature/bearer；
   - `wx.requestPayment` → Payment Intent + ConsentGate + audit；
   - `wx.getPhoneNumber` / `wx.chooseAddress` → Host privacy provider + consent；
   - `wx.cloud.*`、微信社交、WiFi、蓝牙、TCP/UDP 等 → unsupported 或远期计划。

### 4.3 组件兼容矩阵

1. 分层记录：
   - Component JS；
   - WXML；
   - WXSS；
   - 内置组件；
   - 事件；
   - `wx.modelContext`；
   - 动态组件；
   - Render IR / Host adapter。
2. 每项能力必须说明：
   - 当前状态；
   - 目标 Phase；
   - 是否影响安全边界；
   - 是否需要 fixture；
   - fallback 策略。
3. 对不做完整 UI 的原则做显式备注：Host 可以选择 Flutter/SwiftUI/Web/native card，但容器只承诺 Render IR contract。

### 4.4 Release gates

建议 gates：

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p dock-cli --test coffee_order_flow
```

新增 Phase 0 文档后还要加入：

- Markdown link check；
- compatibility matrix coverage check（每个 API/组件至少有一行状态）；
- redaction fixture check；
- sandbox escape regression check；
- Render IR snapshot check（Phase 2 后启用）。

## 5. 阶段完成检查

Phase 0 完成前逐项确认：

- [ ] API 矩阵覆盖小程序 MCP 本地参考中列出的所有关键 API。
- [ ] 组件矩阵覆盖 P0/P1/P2 内置组件、事件、WXML/WXSS、动态能力。
- [ ] 每个 planned 项都有目标 Phase 和 owner crate。
- [ ] release gates 文档能被 CI 或本地脚本执行。
- [ ] security threat model 有初版攻击面和控制措施。
- [ ] roadmap 与本目录索引互相链接。
- [ ] 任何 demo-only 能力都已标注，不能被误认为 production-ready。
