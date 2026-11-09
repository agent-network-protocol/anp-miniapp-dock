# anp-miniapp-dock 产品化迭代总体计划

> 状态：计划文档
> 日期：2026-06-11
> 范围：仅补齐后续开发计划文档，不在本步骤开发代码。
> 依据：`docs/architecture/`、`docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt`、`docs/runbook/`、`docs/plan/did-wx-python-integration-plan.md`、当前 Cargo workspace 与测试现状。

## 1. 目标与边界

本计划的目标是把当前 coffee Demo 原型逐步演进为可线上使用的稳定 **Agentic MiniApp Container**。核心原则保持不变：

1. **智能体原生**：容器服务于 Agent 对话场景中的 Skill 调用，而不是复刻完整微信小程序运行时。
2. **MCP 接口兼容优先**：尽量对齐小程序 MCP 的 `SKILL.md`、`mcp.json`、原子接口、原子组件、`wx.modelContext` 与关键 `wx.*` 能力。
3. **ANP DID 替换底层身份与网络**：登录、鉴权、签名、请求、token、商家 Agent 访问使用 ANP DID / ANP Rust SDK 能力承载。
4. **核心能力补齐，不做完整 UI**：组件运行时要对齐原子组件契约与 Render IR 输出，但不进入完整页面路由、TabBar、半屏页面、小程序宿主 UI 复刻。
5. **安全默认开启**：沙箱隔离、权限声明、allowlist、capability token、human authorization、审计与脱敏都必须成为默认路径。

### 1.1 当前已具备的基线能力

当前代码已经不是空 Demo，已具备 P0/P0.5 基线：

- `mcp-schema`：`mcp.json.apis[]`、`components[]`、`inputSchema`、`outputSchema`、`AtomicApiResult`、模型可见字段与 `_meta` 隔离的基础模型和校验。
- `skill-loader`：加载 `SKILL.md`、`mcp.json`、`index.js`、`apis/*.js` 与组件包，阻断路径穿越和跨包 require。
- `js-runtime-quickjs`：QuickJS 原子接口 VM、受限 CommonJS、`createSkill`、`registerAPI`、`skill.use`、middleware、超时、日志、禁用 `fetch` / `process` / `eval` / `Function` 等全局逃逸入口；当前已注入 demo 级 `wx.login` / `wx.request` localhost DID 登录桥。
- `component-runtime`：`Component({})` 子集、`data` / `properties` / `methods`、`created` / `attached` / `detached`、`setData`、`Input` / `Result` / `Expire`、`sendFollowUpMessage`、`api/call`、`expirePreviousCards` / `expireAllCards`、tap / image load / image error、WXML/WXSS 子集与 Render IR JSON。
- `wx-compat`：capability profile、request broker trait、scoped storage、model context、card expiration、device/app info helper。
- `anp-adapter`：DID credential provider、HTTP signature helper、challenge proof、scoped capability token、token cache、allowlist request broker。
- `consent-audit`：风险分级、mock consent provider、consent proof、审计记录与敏感字段脱敏。
- `card-spec`：结构化 fallback card。
- `demo-server` / `dock-cli`：coffee merchant Agent demo、真实 DID challenge proof、scoped capability token、coffee order E2E、组件 action 驱动、卡片过期验证。
- `examples/coffee-fastapi-server` 与 `mac-app/`：用于演示 Python 远端服务和 Mac Chatbot host 的辅助链路。

### 1.2 仍需补齐的产品化缺口

为了达到线上稳定产品，后续不能只扩展 demo 业务，而要补齐以下系统能力：

- **接口对齐缺口**：`wx.modelContext` 与微信标准 `wx.*` API 还没有完整的能力分层、JS 注入、错误语义、callback / Promise 兼容、权限声明和测试矩阵。
- **组件对齐缺口**：当前组件运行时覆盖交易型卡片 P0 子集，尚未系统支持小程序 MCP 的组件支持列表、`relatedPage`、`expirable`、`openDetailPage` fallback、`Overflow`、动态组件、表单类组件、map/canvas 静态能力和更完整 WXML/WXSS。
- **安全缺口**：需要正式 threat model、生产级沙箱资源限制、包完整性/签名、权限策略引擎、token 轮换/撤销、真实审计落盘、敏感输出审计和供应链治理。
- **运行时产品缺口**：缺少稳定公共 API、持久化 session/token/storage/audit、Skill 获取与版本管理、Host 接入协议、生产部署形态、观测指标和发布门禁。
- **开发者生态缺口**：需要导入/校验/迁移工具、兼容性报告、golden fixtures、示例 Skill 集、文档和 release certification。

## 2. 三层计划总览

本计划按三个层级组织：

1. **总体计划**：从 Demo 原型到线上容器的阶段路线图。
2. **每个阶段的整体计划**：说明阶段目标、主要工作包、产出物和验收门槛。
3. **每个阶段中的具体细分小阶段及实施方案**：拆成可执行的开发迭代单元。

### 2.1 详细阶段文档

每个阶段的可执行开发计划已展开到独立目录：[`production-readiness/`](production-readiness/README.md)。后续开发应以该目录中的阶段文档作为 issue 拆分和验收依据。

| 阶段 | 详细计划 |
|---|---|
| Phase 0 | [基线冻结与产品化门槛](production-readiness/phase-0-baseline-and-gates.md) |
| Phase 1 | [接口对齐与 wx Capability Broker](production-readiness/phase-1-wx-capability-broker.md) |
| Phase 2 | [组件运行时对齐](production-readiness/phase-2-component-runtime-alignment.md) |
| Phase 3 | [安全增强与可信执行](production-readiness/phase-3-security-hardening.md) |
| Phase 4 | [生产运行时与 Host 接入](production-readiness/phase-4-runtime-host-integration.md) |
| Phase 5 | [开发者体验与生态兼容](production-readiness/phase-5-developer-experience.md) |
| Phase 6 | [观测、性能与发布运营](production-readiness/phase-6-observability-release.md) |

### 2.2 总体路线图

| 阶段 | 名称 | 核心目标 | 主要产出 | 完成标志 |
|---|---|---|---|---|
| Phase 0 | 基线冻结与产品化门槛 | 把当前 P0.5 能力变成可追踪基线，建立兼容矩阵和 release gates | 兼容缺口台账、测试基线、里程碑拆分 | 所有后续开发都有明确 scope、DoD 和验收命令 |
| Phase 1 | 接口对齐与 wx Capability Broker | 补齐原子接口环境中关键 `wx.modelContext` / `wx.*` 能力，对齐小程序 MCP 接口语义 | wx API 兼容层、权限映射、JS bridge、测试矩阵 | 核心交易型 Skill 可不改或少改运行 |
| Phase 2 | 组件运行时对齐 | 补齐小程序 MCP 原子组件核心能力，保持 Render IR 主线 | 组件兼容矩阵、更多 WXML/WXSS/内置组件、动态组件受控能力 | 多个真实交易型组件 fixture 通过快照和交互测试 |
| Phase 3 | 安全增强与可信执行 | 让容器默认满足线上安全边界 | threat model、沙箱加固、权限策略、token 生命周期、审计落盘、包签名 | 安全审计 checklist 全部通过，高风险动作无法绕过 |
| Phase 4 | 生产运行时与 Host 接入 | 从 CLI demo 变成可集成、可部署、可升级的容器 | Runtime API、进程/SDK 形态、持久化、Skill registry/cache、Host adapter contract | Runtime/Host contract、headless conformance、持久化边界和 release blockers 可审计；真实 production Host 仍需后续接入 |
| Phase 5 | 开发者体验与生态兼容 | 让 Skill 开发者能迁移、调试、认证兼容性 | CLI/SDK、导入工具、示例 Skill、兼容报告、文档站 | 外部 Skill 可自助完成本地验证 |
| Phase 6 | 观测、性能与发布运营 | 达到线上可运维、可回滚、可持续发布 | metrics/logs/traces、性能基线、CI/CD gates、runbook | 可灰度发布并定位线上问题 |

> 推荐执行顺序：Phase 0 必须先做；Phase 1、Phase 2 可并行小步推进，但 Phase 3 的安全设计应在 Phase 1/2 开始前冻结关键原则；Phase 4 依赖 Phase 1/2/3 的稳定接口；Phase 5/6 与各阶段同步补齐。

### 2.3 Codex Goal 执行控制

本节把 roadmap 补强为可由 Codex Goal 长跑执行的主 Plan。执行者必须把本文作为唯一规划入口；阶段设计仍以第 3 至第 12 节和 [`production-readiness/`](production-readiness/README.md) 下的阶段文档为准，具体执行状态以本节台账和小 Step 文档为准。

主 Plan 路径：`anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md`

Step 文档目录：`anp/anp-miniapp-dock/docs/plan/production-readiness/steps/`

Harness 入口：`awiki-harness/context/00-context-map.md`、`awiki-harness/context/40-verification.md`、`awiki-harness/context/50-task-workflow.md`

执行状态：`pending`、`in_progress`、`review`、`blocked`、`committed`、`done`

#### 2.3.1 Resume From Here

当前恢复指针：当前 Phase 5/6 Goal 范围内没有非 `done` Step；Step 05-01 至 Step 06-07 已全部完成。若后续继续 production Host/deploy/provider 接入，必须通过 Plan 变更记录新增 Step 或创建新 Plan，不依赖聊天历史继续追加范围。

恢复规则：

1. 启动或恢复前，读取本文、当前第一个非 `done` 的 Step 文档、执行台账、本节执行协议、Blocked 处理、Plan 变更记录和当前 `git status --short --branch`。
2. 从执行台账中第一个状态不是 `done` 的 Step 继续，不依赖聊天历史判断进度。
3. 同一时间只允许一个 active Step，除非本文明确标记多个 Step 彼此独立且 parallel-safe。当前 Step 批次没有标记 parallel-safe。
4. 如果当前 Step 状态为 `blocked`，先读取该 Step 的 Blocked 记录；只有依赖允许且风险已记录时，才能转入下一个独立 Step。
5. 每个 Step 完成实现、验证、Review、必要修复和聚焦 commit 后，才能把状态标为 `done` 并进入下一个 Step。

#### 2.3.2 Step 拆分

Phase 0、Phase 1 首批 Step 00-01 至 01-04、以及 Step 01-05 至 02-07 均已完成。Step 01-05 至 02-07 覆盖剩余 Phase 1 `planned-p1` API 能力、Phase 2 组件运行时 P1 起步能力，以及进入 Phase 3 前必须完成的批次最终 Review gate；Step 03-01 至 06-07 覆盖 Phase 3 至 Phase 6 的安全、运行时/Host、开发者体验和发布运营里程碑。后续新增阶段或范围变化仍必须按同一模板拆分小 Step，并通过 Plan 变更记录补充到本表。

| Step | 标题 | 依赖 | 主要产出 | 小 Plan 文档 | Commit gate | 状态 |
|---|---|---|---|---|---|---|
| 00-01 | 当前能力盘点与基线固化 | 无 | 当前能力清单、证据表、demo-only 标注 | [production-readiness/steps/00-01-baseline-inventory.md](production-readiness/steps/00-01-baseline-inventory.md) | 必须 | done |
| 00-02 | wx API 兼容矩阵 | 00-01 | `docs/architecture/wx-api-compatibility-matrix.md` | [production-readiness/steps/00-02-wx-api-compatibility-matrix.md](production-readiness/steps/00-02-wx-api-compatibility-matrix.md) | 必须 | done |
| 00-03 | 组件兼容矩阵 | 00-01 | `docs/architecture/component-compatibility-matrix.md` | [production-readiness/steps/00-03-component-compatibility-matrix.md](production-readiness/steps/00-03-component-compatibility-matrix.md) | 必须 | done |
| 00-04 | Threat model 与 release gates 初版 | 00-01, 00-02, 00-03 | `docs/security/threat-model.md`、`docs/runbook/release-gates.md` | [production-readiness/steps/00-04-threat-model-release-gates.md](production-readiness/steps/00-04-threat-model-release-gates.md) | 必须 | done |
| 01-01 | wx API Bridge Contract 冻结 | 00-02 | bridge 契约、错误语义、callback/Promise 决策 | [production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md](production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md) | 必须 | done |
| 01-02 | Skill package 与 manifest 对齐 | 00-02, 00-03, 01-01 | manifest 校验、兼容报告、测试 | [production-readiness/steps/01-02-skill-package-manifest-alignment.md](production-readiness/steps/01-02-skill-package-manifest-alignment.md) | 必须 | done |
| 01-03 | `wx.modelContext` 原子接口桥接 | 01-01, 01-02 | `getSessionId`、`expireAllCards`、NotificationType、card event | [production-readiness/steps/01-03-model-context-bridge.md](production-readiness/steps/01-03-model-context-bridge.md) | 必须 | done |
| 01-04 | DID 会话与 RequestBroker 收敛 | 01-01, 01-02 | `DidAuthSessionManager`、`wx.login`、`wx.checkSession`、`wx.request` 正式路径 | [production-readiness/steps/01-04-did-session-request-broker.md](production-readiness/steps/01-04-did-session-request-broker.md) | 必须 | done |
| 01-05 | Unsupported API Registry 与统一 fail shape | 01-01, 01-04 | unsupported registry、统一 fail shape、矩阵同步 | [production-readiness/steps/01-05-unsupported-api-registry-fail-shape.md](production-readiness/steps/01-05-unsupported-api-registry-fail-shape.md) | 必须 | done |
| 01-06 | Storage JS Bridge | 01-01, 01-04, 01-05 | `wx.getStorage` / `setStorage` / `removeStorage` / `clearStorage` 及同步版本 | [production-readiness/steps/01-06-storage-js-bridge.md](production-readiness/steps/01-06-storage-js-bridge.md) | 必须 | done |
| 01-07 | Device/App Info Atomic API | 01-01；独立于 01-06，可在 01-06 blocked 时按 Blocked 规则串行跳转 | `wx.getDeviceInfo`、`wx.getAppBaseInfo` Atomic API 最小实现 | [production-readiness/steps/01-07-device-app-info-atomic-api.md](production-readiness/steps/01-07-device-app-info-atomic-api.md) | 必须 | done |
| 01-08 | 高风险 API Host Boundary 与 fail-closed | 01-01, 01-04, 01-05 | phone/address/location/media/payment/scan/phone call Host boundary、ConsentGate、fail closed | [production-readiness/steps/01-08-high-risk-api-host-boundary.md](production-readiness/steps/01-08-high-risk-api-host-boundary.md) | 必须 | done |
| 02-01 | Render IR schemaVersion 与 fallback reason enum | 00-03, 01-03 | `dock.render-ir.v1`、fallback reason enum | [production-readiness/steps/02-01-render-ir-schema-fallback-reasons.md](production-readiness/steps/02-01-render-ir-schema-fallback-reasons.md) | 必须 | done |
| 02-02 | Component manifest metadata runtime flow | 01-02, 01-03, 02-01 | `relatedPage`、`scope.dynamic`、`expirable`、`expiredText` runtime metadata | [production-readiness/steps/02-02-component-manifest-metadata-runtime-flow.md](production-readiness/steps/02-02-component-manifest-metadata-runtime-flow.md) | 必须 | done |
| 02-03 | WXML/WXSS P1 语法增强 | 02-01 | `wx:elif` / `wx:else`、`catchtap`、disabled、简单表达式、P1 WXSS | [production-readiness/steps/02-03-wxml-wxss-p1-syntax.md](production-readiness/steps/02-03-wxml-wxss-p1-syntax.md) | 必须 | done |
| 02-04 | 表单与静态媒体节点 | 01-08, 02-01, 02-03 | `input`、`textarea`、`radio`、`checkbox`、`picker`、`map-preview`、`canvas-static` | [production-readiness/steps/02-04-form-static-media-nodes.md](production-readiness/steps/02-04-form-static-media-nodes.md) | 必须 | done |
| 02-05 | Dynamic component controls | 01-04, 02-02；开放前必须通过本 Step sandbox/resource gate | component sandbox escape/resource-limit gate 前置验证、dynamic `wx.request`、timer、expire/detach cleanup | [production-readiness/steps/02-05-dynamic-component-controls.md](production-readiness/steps/02-05-dynamic-component-controls.md) | 必须 | done |
| 02-06 | Fixture 与 Render IR snapshots | 02-01, 02-02, 02-03, 02-04, 02-05 | address-form、media-review、dynamic-status、location-map-preview、golden snapshots | [production-readiness/steps/02-06-fixtures-render-ir-snapshots.md](production-readiness/steps/02-06-fixtures-render-ir-snapshots.md) | 必须 | done |
| 02-07 | 01-05 至 02-06 批次最终 Review 与整体验证 | 01-05, 01-06, 01-07, 01-08, 02-01, 02-02, 02-03, 02-04, 02-05, 02-06 | 批次全局 Review 记录、整体验证证据、Phase 3 启动 gate | [production-readiness/steps/02-07-batch-final-review-verification.md](production-readiness/steps/02-07-batch-final-review-verification.md) | 必须 | done |
| 03-01 | Threat Model 与安全分级收敛 | 00-04, 01-08, 02-07 | 安全控制矩阵、L0-L4 风险分级、release gate 收敛 | [production-readiness/steps/03-01-threat-model-security-classification.md](production-readiness/steps/03-01-threat-model-security-classification.md) | 必须 | done |
| 03-02 | QuickJS 沙箱逃逸回归与资源限制 | 02-05, 03-01 | API VM / Component VM sandbox escape tests、resource limits | [production-readiness/steps/03-02-quickjs-sandbox-resource-limits.md](production-readiness/steps/03-02-quickjs-sandbox-resource-limits.md) | 必须 | done |
| 03-03 | 权限策略引擎与 allowlist decision | 01-08, 02-05, 03-01 | `PermissionDecision`、Host override、network allowlist、decision audit | [production-readiness/steps/03-03-permission-policy-engine-allowlist.md](production-readiness/steps/03-03-permission-policy-engine-allowlist.md) | 必须 | done |
| 03-04 | DID / Token 生命周期与 Resolver 信任锚 | 01-04, 03-01, 03-03 | refresh/revoke/logout、jti replay、resolver cache/trust anchor | [production-readiness/steps/03-04-did-token-lifecycle-resolver.md](production-readiness/steps/03-04-did-token-lifecycle-resolver.md) | 必须 | done |
| 03-05 | Consent Adapter 与持久化 Audit Sink | 01-08, 03-01, 03-03 | Host consent adapter、ConsentProof、persistent audit、redacted export | [production-readiness/steps/03-05-consent-adapter-persistent-audit.md](production-readiness/steps/03-05-consent-adapter-persistent-audit.md) | 必须 | done |
| 03-06 | Skill 包完整性与供应链 Gate | 01-02, 03-01, 03-03 | digest、signature、publisher DID、trusted allowlist、quarantine | [production-readiness/steps/03-06-skill-package-integrity-supply-chain.md](production-readiness/steps/03-06-skill-package-integrity-supply-chain.md) | 必须 | done |
| 03-07 | Phase 3 最终 Review 与整体验证 | 03-01, 03-02, 03-03, 03-04, 03-05, 03-06 | Phase 3 全局 Review 记录、整体验证证据、Phase 4 启动 gate | [production-readiness/steps/03-07-phase3-final-review-verification.md](production-readiness/steps/03-07-phase3-final-review-verification.md) | 必须 | done |
| 04-01 | Runtime API Facade 与版本化 | 02-06, 03-07 | public Runtime API、stable DTO/error、version、CLI 收敛 | [production-readiness/steps/04-01-runtime-api-facade-versioning.md](production-readiness/steps/04-01-runtime-api-facade-versioning.md) | 必须 | done |
| 04-02 | IPC / SDK 形态与 Host 进程边界 | 04-01 | local IPC/headless JSON/Rust SDK envelope、version/error/redaction | [production-readiness/steps/04-02-ipc-sdk-host-process-boundary.md](production-readiness/steps/04-02-ipc-sdk-host-process-boundary.md) | 必须 | done |
| 04-03 | Skill Registry / Cache 与版本回滚 | 03-06, 04-01 | registry ref、digest-keyed cache、version pin、rollback、eviction | [production-readiness/steps/04-03-skill-registry-cache-versioning.md](production-readiness/steps/04-03-skill-registry-cache-versioning.md) | 必须 | done |
| 04-04 | Runtime Config 与 Secret Store 边界 | 04-01 | non-secret runtime config、secret provider boundary、redaction | [production-readiness/steps/04-04-runtime-config-secret-store.md](production-readiness/steps/04-04-runtime-config-secret-store.md) | 必须 | done |
| 04-05 | Token Cache 持久化与恢复 | 03-04, 04-04 | secure token cache backend、TTL/revocation restore policy | [production-readiness/steps/04-05-token-cache-persistence.md](production-readiness/steps/04-05-token-cache-persistence.md) | 必须 | done |
| 04-06 | Scoped Storage 持久化与 quota | 01-06, 04-04 | scoped storage backend、quota、scope cleanup | [production-readiness/steps/04-06-scoped-storage-persistence.md](production-readiness/steps/04-06-scoped-storage-persistence.md) | 必须 | done |
| 04-07 | Persistent Audit Sink retention/export | 03-05, 04-04 | audit persistence、retention、redacted export | [production-readiness/steps/04-07-persistent-audit-retention-export.md](production-readiness/steps/04-07-persistent-audit-retention-export.md) | 必须 | done |
| 04-08 | Skill Cache cleanup 与版本清理 | 04-03, 04-04 | digest cache cleanup、eviction、privacy/delete scope hooks | [production-readiness/steps/04-08-skill-cache-cleanup.md](production-readiness/steps/04-08-skill-cache-cleanup.md) | 必须 | done |
| 04-09 | Host Adapter Contract 与 Action Protocol | 01-08, 02-06, 03-05, 04-01 | Host renderer/provider/action conformance、headless adapter | [production-readiness/steps/04-09-host-adapter-contract-action-protocol.md](production-readiness/steps/04-09-host-adapter-contract-action-protocol.md) | 必须 | done |
| 04-10 | 并发、取消、重试与幂等策略 | 02-05, 03-03, 03-05, 04-01, 04-05, 04-06, 04-07, 04-09 | session manager、cancellation、retry policy、idempotency key | [production-readiness/steps/04-10-concurrency-cancellation-idempotency.md](production-readiness/steps/04-10-concurrency-cancellation-idempotency.md) | 必须 | done |
| 04-11 | Phase 4 最终 Review 与整体验证 | 04-01, 04-02, 04-03, 04-04, 04-05, 04-06, 04-07, 04-08, 04-09, 04-10 | Phase 4 全局 Review 记录、整体验证证据、Phase 5 启动 gate | [production-readiness/steps/04-11-phase4-final-review-verification.md](production-readiness/steps/04-11-phase4-final-review-verification.md) | 必须 | done |
| 05-01 | CLI validate 兼容报告增强 | 01-05, 02-06, 03-06, 04-01, 04-11 | JSON compatibility report、releaseBlockers、修复建议 | [production-readiness/steps/05-01-cli-validate-compatibility-report.md](production-readiness/steps/05-01-cli-validate-compatibility-report.md) | 必须 | done |
| 05-02 | CLI inspect Skill package | 05-01 | package 文件、API/registration 对照、组件/权限/risk/wx usage | [production-readiness/steps/05-02-cli-inspect-skill-package.md](production-readiness/steps/05-02-cli-inspect-skill-package.md) | 必须 | done |
| 05-03 | CLI test-skill 与 Fixture Runner | 02-06, 04-01, 05-01 | fixture runner、snapshot compare、action/audit report | [production-readiness/steps/05-03-cli-test-skill-fixture-runner.md](production-readiness/steps/05-03-cli-test-skill-fixture-runner.md) | 必须 | done |
| 05-04 | CLI import-wechat-mcp | 05-01, 05-02 | dry-run/safe copy、兼容报告、ANP `_meta` patch 建议 | [production-readiness/steps/05-04-cli-import-wechat-mcp.md](production-readiness/steps/05-04-cli-import-wechat-mcp.md) | 必须 | done |
| 05-05 | CLI doctor 环境诊断 | 03-04, 04-04, 04-05, 04-06, 04-07, 05-01 | toolchain/DID/resolver/allowlist/storage/audit/provider diagnostics | [production-readiness/steps/05-05-cli-doctor-environment.md](production-readiness/steps/05-05-cli-doctor-environment.md) | 必须 | done |
| 05-06 | 示例 Skill 与兼容测试集 | 02-06, 05-01, 05-03 | address/media/dynamic/location 示例、README、expected JSON、snapshots | [production-readiness/steps/05-06-example-skills-compatibility-fixtures.md](production-readiness/steps/05-06-example-skills-compatibility-fixtures.md) | 必须 | done |
| 05-07 | 开发者文档与迁移指南 | 05-01, 05-02, 05-03, 05-04, 05-05, 05-06 | import/API/component/security/Host adapter developer docs | [production-readiness/steps/05-07-developer-docs-migration-guides.md](production-readiness/steps/05-07-developer-docs-migration-guides.md) | 必须 | done |
| 05-08 | Phase 5 最终 Review 与整体验证 | 05-01, 05-02, 05-03, 05-04, 05-05, 05-06, 05-07 | Phase 5 全局 Review 记录、整体验证证据、Phase 6 启动 gate | [production-readiness/steps/05-08-phase5-final-review-verification.md](production-readiness/steps/05-08-phase5-final-review-verification.md) | 必须 | done |
| 06-01 | 结构化观测事件与脱敏日志 | 04-01, 04-04, 05-08 | structured events、traceId/sessionId、redaction | [production-readiness/steps/06-01-structured-observability-events.md](production-readiness/steps/06-01-structured-observability-events.md) | 必须 | done |
| 06-02 | Metrics / Tracing 与请求链路关联 | 04-02, 06-01 | metrics registry、trace propagation、low-cardinality labels | [production-readiness/steps/06-02-metrics-tracing-correlation.md](production-readiness/steps/06-02-metrics-tracing-correlation.md) | 必须 | done |
| 06-03 | 性能基线与 Stress Tests | 04-10, 06-02 | benchmarks、stress tests、baseline artifact | [production-readiness/steps/06-03-performance-baselines-stress.md](production-readiness/steps/06-03-performance-baselines-stress.md) | 必须 | done |
| 06-04 | CI/CD Release Gates 自动化 | 02-06, 03-06, 05-03, 06-03 | gate runner、CI workflow、release report、docs link/redaction/snapshot gates | [production-readiness/steps/06-04-ci-cd-release-gates-automation.md](production-readiness/steps/06-04-ci-cd-release-gates-automation.md) | 必须 | done |
| 06-05 | Canary 发布、版本化与回滚策略 | 04-03, 04-09, 06-02, 06-03, 06-04 | release notes、canary stages、rollback/cache purge | [production-readiness/steps/06-05-canary-release-rollback.md](production-readiness/steps/06-05-canary-release-rollback.md) | 必须 | done |
| 06-06 | 运维 Runbook 与隐私删除流程 | 04-04, 04-06, 04-07, 04-08, 05-05, 06-01, 06-02, 06-04, 06-05 | operations/troubleshooting/privacy deletion runbooks | [production-readiness/steps/06-06-operations-runbook-privacy-deletion.md](production-readiness/steps/06-06-operations-runbook-privacy-deletion.md) | 必须 | done |
| 06-07 | Phase 6 最终 Review 与整体验证 | 06-01, 06-02, 06-03, 06-04, 06-05, 06-06 | Phase 6 全局 Review 记录、整体验证证据、当前 Goal closure | [production-readiness/steps/06-07-phase6-final-review-verification.md](production-readiness/steps/06-07-phase6-final-review-verification.md) | 必须 | done |

独立跳转说明：Step 01-07 只依赖 Step 01-01，和 Step 01-06 storage 工作没有实现依赖。若 Step 01-06 被标记为 `blocked`，执行者可以在记录 blocker、确认工作区无未提交完成工作后，按 Blocked 规则串行转入 Step 01-07；这不是并行执行授权。

#### 2.3.3 执行台账

执行者必须在 Step 开始、进入 Review、被阻塞、完成 commit 和标记 done 时更新本台账。证据字段必须指向 Step 文档中的 Review/验证记录或实际命令输出摘要，不得只写“已完成”。

| Step | 状态 | 分支 | 开始时间 | 完成时间 | Commit | Review 证据 | 验证证据 | 下一步 |
|---|---|---|---|---|---|---|---|---|
| 00-01 | done | `main` | 2026-06-12 10:05:15 +0800 | 2026-06-12 10:16:44 +0800 | `de4c3e2` | Step 文档 Review 环节已记录：未发现需修复问题，demo-only/host-boundary 未被误标为 production-ready | `cargo metadata --format-version 1 --no-deps` 成功；`git diff --check -- docs/architecture README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-01-baseline-inventory.md` 无输出；新增基线文档 Markdown 链接手工检查无破链；post-commit `git status --short --branch` = `## main...origin/main [ahead 2]` | 进入 00-02 |
| 00-02 | done | `main` | 2026-06-12 10:18:26 +0800 | 2026-06-12 10:31:32 +0800 | `22e7f25` | Step 文档 Review 环节已记录：未发现阻塞问题，`demo-only`/`host-boundary` 未被误标为 production-ready，高风险 API 均要求 ConsentGate/audit 或 fail closed | `git diff --check -- docs/architecture/wx-api-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-02-wx-api-compatibility-matrix.md` 无输出；协议覆盖抽样命中协议参考和矩阵；status 列结构化检查无非法枚举；矩阵 Markdown 链接检查无破链；L3/L4 与敏感字段抽样通过；post-commit `git status --short --branch` = `## main...origin/main [ahead 4]` | 进入 00-03 |
| 00-03 | done | `main` | 2026-06-12 10:33:02 +0800 | 2026-06-12 10:40:11 +0800 | `fea9d35` | Step 文档 Review 环节已记录：未发现阻塞问题，Runtime/Host 责任分离，dynamic、Host renderer 和完整小程序能力未被误标为 production-ready | `git diff --check -- docs/architecture/component-compatibility-matrix.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-03-component-compatibility-matrix.md` 无输出；覆盖抽样命中矩阵、架构和 Phase 2 计划；status 列结构化检查无非法枚举；矩阵 Markdown 链接检查无破链；安全边界和 Host 边界抽样通过；post-commit `git status --short --branch` = `## main...origin/main [ahead 6]` | 进入 00-04 |
| 00-04 | done | `main` | 2026-06-12 10:41:43 +0800 | 2026-06-12 10:51:58 +0800 | `04448a1` | Step 文档 Review 环节已记录：未发现阻塞问题，planned gate 未被误写成当前已自动化，demo-only/mock 能力仍为 production release blocker | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 7]`；`git diff --check -- docs/security docs/runbook/release-gates.md README.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/00-04-threat-model-release-gates.md` 无输出；安全红线抽样命中 threat model、release gates 和 redaction 规则；README、threat model、release gates Markdown 相对链接检查无破链；release gate 命令与 `AGENTS.md` 一致；post-commit `git status --short --branch` = `## main...origin/main [ahead 8]` | 进入 01-01 |
| 01-01 | done | `main` | 2026-06-12 10:53:13 +0800 | 2026-06-12 10:59:18 +0800 | `10db676` | Step 文档 Review 环节已记录：未发现阻塞问题，callback/Promise、`wx.request` HTTP status、unsupported shape、JS-provided auth header、sync API 和 redaction 均已有冻结行为 | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 9]`；`git diff --check -- docs/plan/production-readiness/phase-1-wx-api-bridge-contract.md docs/architecture/wx-api-compatibility-matrix.md docs/plan/production-readiness-roadmap.md docs/plan/production-readiness/steps/01-01-wx-api-bridge-contract-freeze.md` 无输出；未决行为检查无命中；脱敏和契约抽样命中敏感字段、HTTP response、4xx/5xx、Promise reject、unsupported、错误码和 WxApiCall/WxApiOutcome；contract 与矩阵 Markdown 链接检查无破链；post-commit `git status --short --branch` = `## main...origin/main [ahead 10]` | 进入 01-02 |
| 01-02 | done | `main` | 2026-06-12 11:00:47 +0800 | 2026-06-12 11:22:56 +0800 | `ec46e1f` | Step 文档 Review 环节已记录：修复 registration mismatch 报告测试缺口和根级 `format:file` warning path 稳定性问题，未发现阻塞问题 | `cargo fmt --check` 通过；`cargo test -p mcp-schema` 13 passed；`cargo test -p skill-loader` 7 passed；`cargo test -p dock-cli validate` 2 passed；`cargo run -p dock-cli -- validate examples/coffee-skill` 输出 `compatibilityLevel: demo-only` 和完整 `compatibilityReport`；`git diff --check -- crates/mcp-schema crates/skill-loader crates/dock-cli docs/architecture docs/runbook docs/plan` 无输出；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；post-commit `git status --short --branch` = `## main...origin/main [ahead 12]` | 进入 01-03 |
| 01-03 | done | `main` | 2026-06-12 11:24:05 +0800 | 2026-06-12 11:45:53 +0800 | `1504eff` | Step 文档 Review 环节已记录：补齐 `createSkill(skillPath)` 路径校验、脱敏 `expireAllCards` invalid options 错误、`match: all` 覆盖和矩阵同步；生产 card manager/持久化 audit 仍按 Phase 2/4 边界记录 | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs -p wx-compat -p dock-core model_context` 通过，实际 js-runtime 6 passed、wx-compat 2 passed、dock-core 0 tests under filter；`cargo test -p js-runtime-quickjs create_skill` 1 passed；`cargo test -p component-runtime` 25 passed；`cargo test -p dock-core` 9 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`git diff --check -- crates/js-runtime-quickjs crates/wx-compat crates/dock-core crates/component-runtime docs/architecture docs/plan` 无输出；`cargo clippy --workspace --all-targets -- -D warnings` 通过；post-commit `git status --short --branch` = `## main...origin/main [ahead 14]` | 进入 01-04 |
| 01-04 | done | `main` | 2026-06-12 11:47:39 +0800 | 2026-06-12 12:23:36 +0800 | `e9cbdbe` | Step 文档 Review 环节已记录：修复 callback exception 改变 Promise outcome/跳过 `complete` 的契约问题；确认 token 只在 Host/runtime 边界、JS auth header fail closed、response auth/token headers 已脱敏；production Host RequestBroker transport 和 persistent audit 按 Phase 4 残余风险记录 | pre-flight: `git status --short --branch` = `## main...origin/main [ahead 15]`；`cargo fmt --check` 通过；`cargo test -p anp-adapter session` 8 passed；`cargo test -p js-runtime-quickjs wx_login/check_session/wx_request/wx_callback_exception/model_context_expire_all_cards` 均通过；`cargo test -p js-runtime-quickjs` 28 passed；`cargo test -p anp-adapter` 41 passed；`cargo test -p wx-compat` 9 passed；`cargo test -p demo-server token` 4 passed under filter；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/js-runtime-quickjs crates/wx-compat crates/dock-core crates/demo-server crates/dock-cli examples/coffee-fastapi-server docs/architecture docs/runbook docs/security docs/plan` 无输出；post-commit `git status --short --branch` = `## main...origin/main [ahead 16]` | 首批 Step 已全部完成，进入最终全局 Review |
| 01-05 | done | `main` | 2026-06-12 20:02:26 +0800 | 2026-06-12 20:21:27 +0800 | `8e475dd` | Step 文档 Review 环节已记录：修复 focused `unsupported` filter 未覆盖 unknown root fallback 的测试命名；确认 registry/stub 不覆盖已支持 API，Proxy intrinsic 不暴露给 Skill，真实 provider 状态未误标 supported | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs unsupported` 5 passed；`cargo test -p wx-compat unsupported` 4 passed；`cargo test -p js-runtime-quickjs wx_` 13 passed；`cargo test -p js-runtime-quickjs` 33 passed；`cargo test -p wx-compat` 11 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs docs/architecture docs/runbook docs/plan` 无输出；敏感词抽样仅命中安全说明、测试夹具假值和 redaction 断言 | 进入 01-06 |
| 01-06 | done | `main` | 2026-06-12 20:22:41 +0800 | 2026-06-12 20:43:49 +0800 | `1599294` | Step 文档 Review 环节已记录：未发现阻塞问题；确认 scope 使用 `userDid + merchantDid + skillId` 且不含 `sessionId`，sync/async 语义与 Step 01-01 契约一致，storage 内容不自动进入 model-visible result，未引入生产持久化承诺 | `cargo fmt --check` 通过；`cargo test -p wx-compat storage` 8 passed；`cargo test -p js-runtime-quickjs storage` 6 passed；`cargo test -p js-runtime-quickjs wx_` 20 passed；`cargo test -p js-runtime-quickjs` 39 passed；`cargo test -p wx-compat` 16 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/runbook docs/security docs/plan` 无输出；`cargo clippy --workspace --all-targets -- -D warnings` 通过；敏感词抽样仅命中文档规则、测试假值和 redaction 断言；post-commit `git status --short --branch` = `## main...origin/main [ahead 25]` | 进入 01-07 |
| 01-07 | done | `main` | 2026-06-12 20:45:17 +0800 | 2026-06-12 20:53:52 +0800 | `50cc245` | Step 文档 Review 环节已记录：未发现阻塞问题；确认 Atomic API 与 Component VM 使用 `wx-compat` shared defaults，字段最小化，不返回真实设备指纹或 Host credential 信息，sync API 不返回 Promise | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs info` 2 passed；`cargo test -p wx-compat device` 1 passed；`cargo test -p component-runtime` 26 passed；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/component-runtime docs/architecture docs/plan docs/runbook` 无输出；`cargo clippy --workspace --all-targets -- -D warnings` 通过；敏感字段抽样仅命中文档红线、测试假值、禁用字段断言和既有 redaction 代码；post-commit `git status --short --branch` = `## main...origin/main [ahead 27]` | 进入 01-08 |
| 01-08 | done | `main` | 2026-06-12 20:55:30 +0800 | 2026-06-12 21:09:39 +0800 | `33591f0` | Step 文档 Review 环节已记录：未发现阻塞问题；确认默认 Atomic API runtime 只接入 unavailable provider、dev-only provider 未进入 production 默认路径、ConsentGate 在 provider 前阻断、本地路径和支付密码不会回显 | `cargo fmt --check` 通过；`cargo test -p wx-compat provider` 3 passed；`cargo test -p wx-compat high_risk` 2 passed；`cargo test -p js-runtime-quickjs high_risk` 3 passed；`cargo test -p wx-compat unsupported` 4 passed；`cargo test -p consent-audit -p dock-core consent` 8 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/dock-core crates/consent-audit crates/js-runtime-quickjs docs/architecture docs/runbook docs/security docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值和 redaction 断言；post-commit `git status --short --branch` = `## main...origin/main [ahead 29]` | 进入 02-01 |
| 02-01 | done | `main` | 2026-06-12 21:11:25 +0800 | 2026-06-12 21:20:27 +0800 | `0cfea24` | Step 文档 Review 环节已记录：未发现阻塞问题；确认所有 Component Runtime Render IR 输出带 `schemaVersion`，fallback reason 对外为稳定枚举值，旧自由字符串只在内部 normalize，未向 CLI/Host payload 泄露路径或错误细节 | `cargo fmt --check` 通过；`cargo test -p component-runtime render` 5 passed；`cargo test -p component-runtime render_output_serializes_schema_version` 1 passed；`cargo test -p card-spec fallback` 1 passed；`cargo test -p dock-core fallback` 1 passed；`cargo test -p dock-cli preview_card` 2 passed；`cargo test -p card-spec -p dock-core -p component-runtime` 40 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime crates/dock-core crates/card-spec crates/dock-cli docs/architecture docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假路径和文档安全说明；post-commit `git status --short --branch` = `## main...origin/main [ahead 31]` | 进入 02-02 |
| 02-02 | done | `main` | 2026-06-12 21:22:16 +0800 | 2026-06-12 21:37:07 +0800 | `79417d5` | Step 文档 Review 环节已记录：修复 validate report 泄露原始 `relatedPage.query.secretToken` 的问题和 metadata 初版从 JS snapshot 回传的错误设计；确认 metadata 来自 manifest，dynamic request/timer 仍默认关闭 | `cargo fmt --check` 通过；`cargo test -p mcp-schema -p skill-loader component` 5 passed；`cargo test -p component-runtime metadata` 1 passed；`cargo test -p dock-cli metadata` 2 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo test -p component-runtime -p dock-cli -p mcp-schema -p skill-loader` 73 passed；`cargo run -p dock-cli -- validate examples/coffee-skill` 输出 `demo-only` 且 components 含 `runtimeMetadata`；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/mcp-schema crates/skill-loader crates/component-runtime crates/wx-compat crates/dock-cli docs/architecture docs/plan docs/runbook` 无输出；敏感词抽样仅命中 redaction 规则、测试假值和文档红线；post-commit `git status --short --branch` = `## main...origin/main [ahead 33]` | 进入 02-03 |
| 02-03 | done | `main` | 2026-06-12 21:38:22 +0800 | 2026-06-12 21:54:00 +0800 | `c8bb813` | Step 文档 Review 环节已记录：修复复杂 selector 静默吞掉的问题；确认 expression evaluator 为 allowlist-only，不执行任意 JS；disabled button 不产生 tap/catchtap action；`catchtap` 只扩展 Render IR 事件语义并仍映射为受控 tap；未引入 02-04 表单节点或 02-05 dynamic request/timer | `cargo fmt --check` 通过；`cargo test -p component-runtime wx` 14 passed under filter；`cargo test -p component-runtime` 36 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime docs/architecture docs/plan` 无输出；本步骤 diff 敏感词抽样未新增真实 secret、本机绝对路径或隐私数据；post-commit `git status --short --branch` = `## main...origin/main [ahead 35]` | 进入 02-04 |
| 02-04 | done | `main` | 2026-06-12 21:54:59 +0800 | 2026-06-12 22:03:40 +0800 | `cc7b3b8` | Step 文档 Review 环节已记录：修复 `maxlength` 等数值 props 初版以字符串输出的问题；确认新增表单节点只是 Render IR 数据，disabled 会抑制 `input` / `change` / tap event，`map-preview` 不透传精确经纬度/markers，`canvas-static` 不开放 script/touch 交互，未绕过 Orchestrator input validation、ConsentGate 或 Host provider | `cargo fmt --check` 通过；`cargo test -p component-runtime node` 7 passed under filter；`cargo test -p component-runtime` 40 passed；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime crates/dock-core docs/architecture docs/plan` 无输出；敏感词抽样仅命中本步骤精确经纬度/markers 拒绝测试、文档安全说明和既有台账文字；post-commit `git status --short --branch` = `## main...origin/main [ahead 37]` | 进入 02-05 |
| 02-05 | done | `main` | 2026-06-12 22:05:11 +0800 | 2026-06-12 22:32:24 +0800 | `7baca29` | 2026-06-12 22:30:25 +0800 commit 前 Review 已记录：修复 native request bridge 全局暴露、component `wx.request` callback 语义与 Atomic API bridge 不一致、`setInterval` 退化为一次性 flush、resource-limit 缺少 focused timeout 测试；剩余 Host transport/background scheduler/persistent audit 按 Phase 4 边界记录 | `cargo fmt --check` 通过；`cargo test -p component-runtime dynamic` 5 passed；`cargo test -p component-runtime sandbox` 2 passed；`cargo test -p component-runtime` 46 passed；`cargo test -p wx-compat` 22 passed；`cargo test -p anp-adapter request` 2 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/component-runtime crates/wx-compat crates/js-runtime-quickjs crates/anp-adapter docs/architecture docs/runbook docs/security docs/plan` 无输出；敏感词抽样仅命中 redaction 规则、测试假值和安全文档；post-commit `git status --short --branch` = `## main...origin/main [ahead 39]` | 进入 02-06 |
| 02-06 | done | `main` | 2026-06-12 22:33:56 +0800 | 2026-06-12 22:59:08 +0800 | `f778a14` | 2026-06-12 22:56:36 +0800 commit 前 Review 已记录：修复 dynamic snapshot `brokerCalls` 取值时机和 dynamic policy 过期文案；确认 snapshots 稳定、mock-only、无禁用敏感串 | `cargo fmt --check` 通过；`cargo test -p component-runtime snapshot` 通过；`cargo test -p dock-cli fixture` 通过；`cargo test -p mcp-schema` 13 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；严格 fixture/snapshot 敏感串扫描无命中；post-commit `git status --short --branch` = `## main...origin/main [ahead 41]` | 进入 02-07 |
| 02-07 | done | `main` | 2026-06-12 23:00:42 +0800 | 2026-06-12 23:06:53 +0800 | `2f0d122`；closure `8cd9b80` | 2026-06-12 23:04:43 +0800 批次最终 Review 已记录：修复 Phase 2 子文档误把全部 P1 Component JS 能力标为完成的问题；确认 01-05 至 02-06 evidence、git history、dynamic sandbox gate、Render IR snapshots、release gates 和安全边界可审计；closure commit 已回填 02-07 done 和停止于 Phase 2 的台账状态 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test -p component-runtime snapshot` 通过；`cargo test -p dock-cli fixture` 通过；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；post-closure `git status --short --branch` = `## main...origin/main [ahead 44]` | 本 Goal 停止在 02-07，不进入 03-01 |
| 03-01 | done | `main` | 2026-06-13 13:42:54 +0800 | 2026-06-13 13:51:12 +0800 | `a61a7e7` | 2026-06-13 13:49:52 +0800 commit 前 Review 已记录：修复 Threat Model 中 Render IR schemaVersion/snapshot 与 unsupported registry 的旧 planned 表述；确认 L0-L4、L3/L4 高风险能力、Phase 3 required gates、Phase 4/5 后续 gate 和 demo-only/mock 禁止项未冲突 | `git diff --check -- docs/security docs/runbook docs/architecture docs/plan` 无输出；风险等级抽样命中 L3/L4、ConsentGate、audit、redaction、fail closed 控制说明；敏感词抽样仅命中文档红线、mock/dev-only 示例、测试说明和计划台账；旧状态残留搜索无命中；Markdown 链接目标手工检查存在；post-commit `git status --short --branch` = `## main...origin/main [ahead 47]` | 进入 03-02 |
| 03-02 | done | `main` | 2026-06-13 13:52:49 +0800 | 2026-06-13 14:11:33 +0800 | `1c4e784` | 2026-06-13 14:06:07 +0800 commit 前 Review 已记录：修复 API VM console trace 丢失和 invalid result payload 回显风险；确认 Atomic API VM WebSocket/timer globals deny、Promise job drain、console/result size、Component VM snapshot size、dynamic timer cleanup 与文档 gate 一致 | `cargo fmt --check` 通过；`cargo test -p js-runtime-quickjs sandbox` 通过；`cargo test -p js-runtime-quickjs limit` 2 passed；`cargo test -p js-runtime-quickjs console` 1 passed；`cargo test -p js-runtime-quickjs invalid_atomic` 1 passed；`cargo test -p js-runtime-quickjs pending_job` 1 passed；`cargo test -p component-runtime sandbox` 2 passed；`cargo test -p component-runtime dynamic` 5 passed + snapshot dynamic 2 passed；`cargo test -p component-runtime snapshot_size` 1 passed；`cargo test -p js-runtime-quickjs` 47 passed；`cargo test -p component-runtime` 53 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/js-runtime-quickjs crates/component-runtime docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值和 redaction 断言；post-commit `git status --short --branch` = `## main...origin/main [ahead 49]` | 进入 03-03 |
| 03-03 | done | `main` | 2026-06-13 14:12:53 +0800 | 2026-06-13 14:48:46 +0800 | `32ada09` | 2026-06-13 14:38:45 +0800 commit 前 Review 已记录：修复 Host allow 可绕过未声明敏感权限、通用 boolean permission 可能误声明任意 capability 的问题；确认 Host deny override 优先、mock 仅 dev/headless、Prompt 进入 ConsentGate、decision audit 脱敏、allowlist mismatch 在 transport 前失败 | `cargo fmt --check` 通过；`cargo test -p wx-compat permission` 7 passed；`cargo test -p dock-core permission` 2 passed；`cargo test -p anp-adapter allowlist` 5 passed；`cargo test -p wx-compat` 29 passed；`cargo test -p anp-adapter` 44 passed；`cargo test -p dock-core` 11 passed；`cargo test -p js-runtime-quickjs wx_request` 4 passed；`cargo test -p component-runtime dynamic` 7 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/dock-core crates/anp-adapter crates/consent-audit crates/js-runtime-quickjs crates/component-runtime crates/dock-cli docs/architecture docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假值、文档安全说明和 `AuthMode::HttpSignatures` 常量；pre-commit `git status --short --branch` = `## main...origin/main [ahead 50]`；post-commit `git status --short --branch` = `## main...origin/main [ahead 51]` | 进入 03-04 |
| 03-04 | done | `main` | 2026-06-13 14:49:53 +0800 | 2026-06-13 15:11:49 +0800 | `24a8f10` | 2026-06-13 15:10:32 +0800 commit 前 Review 已记录：修复 `TrustedDidDocumentResolver` 仅校验 DID document `id` 而未校验完整 trust anchor 内容的问题；确认 token host-only、普通 `verify()` 兼容、lifecycle/replay API 显式、challenge 登录尝试即消费、resolver/cache/replay failure fail closed | `cargo fmt --check` 通过；`cargo test -p anp-adapter token` 18 passed；`cargo test -p anp-adapter session` 10 passed；`cargo test -p anp-adapter challenge` 15 unit + 1 integration passed；`cargo test -p anp-adapter` 44 unit + 11 integration passed；`cargo test -p demo-server token` 5 unit + 1 integration passed；`cargo test -p demo-server` 7 lib + 4 main + 6 integration passed；`cargo test -p demo-server demo_signature_and_replayed_challenge_are_rejected` 1 passed；`cargo test -p js-runtime-quickjs wx_login` 3 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/demo-server crates/js-runtime-quickjs docs/security docs/runbook docs/plan` 无输出；敏感信息抽样仅命中测试假值、文档安全说明、redaction 断言和 `AuthMode::HttpSignatures` 常量；post-commit `git status --short --branch` = `## main...origin/main [ahead 53]` | 进入 03-05 |
| 03-05 | done | `main` | 2026-06-13 15:11:49 +0800 | 2026-06-13 15:34:39 +0800 | `e7c9f49` | 2026-06-13 15:32:32 +0800 commit 前 Review 已记录：修复 `FileAuditSink` export 只信任已持久化 redacted record、可能导出 legacy/raw JSONL 的问题；补充 Host adapter denied fail-closed audit 测试；确认 ConsentGate 在 executor/provider 前、provider unavailable/denied fail closed 且可审计、dev/headless mock 有显式 provider/actor、JSONL audit record/export 默认脱敏 | `cargo fmt --check` 通过；`cargo test -p consent-audit consent` 3 unit + 3 integration passed；`cargo test -p consent-audit audit` 2 unit + 4 integration passed；`cargo test -p dock-core consent` 9 passed；`cargo test -p consent-audit` 5 unit + 7 integration passed；`cargo test -p dock-core` 15 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/consent-audit crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假值、文档安全说明、redaction 断言、`AuthMode::HttpSignatures` 常量和 demo-only secret placeholder，未发现真实 secret/token/proof/private key path 输出；pre-commit `git status --short` 只包含 03-05 代码、测试和文档变更；post-commit `git status --short --branch` = `## main...origin/main [ahead 55]` | 进入 03-06 |
| 03-06 | done | `main` | 2026-06-13 15:34:39 +0800 | 2026-06-13 16:02:50 +0800 | `b9c767b` | 2026-06-13 15:54:50 +0800 commit 前 Review 已记录：修复 validate report package signature value redaction 测试缺口，并将 digest contract 收紧为 64 位小写 hex；确认 production policy 对 unsigned/digest mismatch/signature mismatch/unknown publisher quarantine/fail closed，local coffee demo 仍为 dev/demo-only | `cargo fmt --check` 通过；`cargo test -p skill-loader package` 通过，filter 实际命中 symlink test 1 passed；`cargo test -p skill-loader` 14 passed；`cargo test -p mcp-schema -p dock-cli validate` 通过；`cargo test -p js-runtime-quickjs remote_require_is_rejected` 1 passed；`cargo run -q -p dock-cli -- validate examples/coffee-skill` 输出 demo-only 且 supplyChain.status = demo-unsigned、releaseBlockers 含 supply_chain；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/skill-loader crates/mcp-schema crates/dock-cli crates/js-runtime-quickjs docs/security docs/runbook docs/plan Cargo.toml Cargo.lock` 无输出；敏感词抽样仅命中测试假值、redaction 断言、安全文档、runbook 和 demo-only placeholder，未发现真实 secret/token/proof/private key path 或 package signature value 输出；post-commit `git status --short --branch` = `## main...origin/main [ahead 57]` | 进入 03-07 |
| 03-07 | done | `main` | 2026-06-13 16:04:15 +0800 | 2026-06-13 16:10:22 +0800 | `e888b24`；closure `9f884ac` | 2026-06-13 16:10:22 +0800 Phase 3 最终 Review 已记录：修复 roadmap 恢复指针和通用 Codex Goal 提示词仍指向 03-01、Phase 3 子文档 03-07 未关闭的文档漂移；确认 03-01 至 03-06 safety gates、release blockers、demo-only 边界和 redaction 口径一致，未发现阻塞问题 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo run -q -p dock-cli -- validate examples/coffee-skill` 输出 `demo-only`、`supplyChain.status = demo-unsigned`、releaseBlockers 含 `supply_chain`；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；Phase 3 commit hash 均可解析；敏感词抽样仅命中测试假值、redaction 断言、安全文档和 demo-only placeholder，未发现真实 secret/token/proof/private key path、package signature value 或隐私原文输出 | 本 Goal 停止在 03-07，不进入 04-01 |
| 04-01 | done | `main` | 2026-06-13 18:28:18 +0800 | 2026-06-13 18:53:14 +0800 | `1b470d5` | Step 文档 Review 环节已记录：修复 RuntimeSkillSummary 输出本机绝对路径、含 capability_token request DTO 派生 Debug、validation report 未二次脱敏的问题；确认 expire_cards/close_session 只冻结稳定边界，不冒充生产 card/session store | `cargo fmt --check` 通过；`cargo test -p dock-core runtime` 4 passed；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/dock-cli crates/component-runtime crates/skill-loader docs/runbook docs/plan` 无输出；Runtime API / CLI 输出脱敏抽样未命中真实 token、Authorization、signature、private key path 或隐私原文；post-commit `git status --short --branch` = `## main...origin/main [ahead 63]` | 进入 04-02 |
| 04-02 | done | `main` | 2026-06-13 18:54:30 +0800 | 2026-06-13 19:08:57 +0800 | `53e71be` | 2026-06-13 19:07:40 +0800 commit 前 Review 已记录：确认 `dock-cli runtime-json` 只作为 `headless-cli-json` / `local-process-stdio` 传输层复用 `RuntimeService`，未绕过 permission、ConsentGate、audit、redaction 或 package integrity；修复 request envelope parse/schema error 未走 IPC redaction envelope 的问题；确认当前未声明 HTTP/gRPC sidecar 或 production Host UI | `cargo fmt --check` 通过；`cargo test -p dock-cli ipc` 4 passed；`cargo test -p dock-core runtime` 4 passed；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/dock-cli crates/demo-server docs/runbook docs/plan` 无输出；手工 `runtime-json` success/error 抽样敏感串扫描无命中；post-commit `git status --short --branch` = `## main...origin/main [ahead 65]` | 进入 04-03 |
| 04-03 | done | `main` | 2026-06-13 19:10:50 +0800 | 2026-06-13 19:29:51 +0800 | `81c32c9` | 2026-06-13 19:28:30 +0800 commit 前 Review 已记录：修复 cache 命中未重新强制 readonly、unknown publisher 可能先复制进 cache、版本字符串排序、package URL query/token audit summary、测试 readonly cache 清理问题；确认本 Step 只冻结本地 registry/cache contract，不声明真实远端 registry download、生产签名 verifier 或 deployment cache hardening 已完成 | `cargo fmt --check` 通过；`cargo test -p skill-loader cache` 3 passed；`cargo test -p skill-loader registry` 5 passed；`cargo test -p skill-loader package` 1 package + 3 registry-related tests under filter passed；`cargo test -p skill-loader` 14 package + 7 registry/cache tests passed；`cargo test -p dock-cli validate` 4 unit + 1 integration passed；`cargo test --workspace` 通过；`cargo clippy -p skill-loader --all-targets -- -D warnings` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/skill-loader crates/anp-adapter crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` 无输出；敏感串抽样仅命中测试假值和安全文档规则，未命中本机绝对路径；post-commit `git status --short --branch` = `## main...origin/main [ahead 67]` | 进入 04-04 |
| 04-04 | done | `main` | 2026-06-13 19:33:40 +0800 | 2026-06-13 19:49:21 +0800 | `189ad87` | 2026-06-13 19:47:25 +0800 commit 前 Review 已记录：修复 diagnostics 未脱敏 `issuer` 中 `merchant secret` 文本、`schemaVersion` 可回显异常敏感串、`cargo test -p dock-core config` 初始只命中 2 个新增测试的问题；确认本 Step 只冻结 config/secret contract，不实现真实 secret resolve 或 token/storage/audit/cache 持久化 backend | 启动前 `git status --short --branch` = `## main...origin/main [ahead 68]`；`cargo fmt --check` 通过；`cargo test -p dock-core config` 8 passed；`cargo test -p dock-core` 27 passed；`cargo test -p dock-cli config` 2 passed under filter，CLI 未新增参数，仅复用既有 credential redaction/config tests；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/dock-cli crates/anp-adapter docs/runbook docs/security docs/plan` 无输出；敏感串扫描仅命中文档红线、redaction marker/test 假值和既有 redaction 回归测试；post-commit `git status --short --branch` = `## main...origin/main [ahead 69]` | 进入 04-05 |
| 04-05 | done | `main` | 2026-06-13 19:51:13 +0800 | 2026-06-13 20:14:30 +0800 | `f742304` | 2026-06-13 20:12:35 +0800 commit 前 Review 已记录：修复 raw token entry 可被 JSON diagnostics 误序列化、fallible persistence API 先改内存后落盘导致失败后状态污染、entry metadata 未显式绑定 issuer/audience/jti、clippy bool assert warning；确认 restore policy fail closed，rejected entry 清出 backend snapshot，report 只含 scope summary/reason/redaction metadata，in-memory profile 明确 dev-only | `cargo fmt --check` 通过；`cargo test -p anp-adapter token_cache` 9 unit + 1 integration under filter passed；`cargo test -p anp-adapter session` 10 passed；`cargo test -p anp-adapter token` 26 unit + 4 integration under filter passed；`cargo test -p anp-adapter` 53 unit + 11 integration + doctests passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/anp-adapter crates/dock-core docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值、redaction 代码和既有测试断言，未发现真实 token、Authorization、signature、private key material 或生产凭据；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 71]` | 进入 04-06 |
| 04-06 | done | `main` | 2026-06-13 20:17:07 +0800 | 2026-06-13 20:35:46 +0800 | `67237ba` | 2026-06-13 20:33:19 +0800 commit 前 Review 已记录：修复 local file backend 对单条非法 persisted record 直接让整个 restore 失败、无法按脱敏 rejection 清理 snapshot 的问题；补充 `StoragePersistenceSnapshot` public re-export；确认 scope 覆盖 user DID、merchant DID、Skill id、namespace，persistent write/cleanup 先落 backend snapshot 再更新内存，quota fail closed，restore report/Debug 不输出 raw key/value，`localFileUnencrypted` 不是 production-ready | `cargo fmt --check` 通过；`cargo test -p wx-compat storage` 14 passed；`cargo test -p js-runtime-quickjs storage` 6 passed；`cargo test -p wx-compat` 通过；`cargo test -p js-runtime-quickjs` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/wx-compat crates/js-runtime-quickjs crates/dock-core docs/architecture docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中文档红线、测试假值、redaction 断言和 dev/local backend 状态，未发现真实 storage 隐私 value、token、Authorization、signature、private key material 或生产凭据；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 73]` | 进入 04-07 |
| 04-07 | done | `main` | 2026-06-13 20:39:10 +0800 | 2026-06-13 21:00:20 +0800 | `c8f4a96` | 2026-06-13 20:58:33 +0800 commit 前 Review 已记录：修复 persistent audit reader 读取损坏后端时静默返回空列表的问题，改为稳定 `audit_unavailable`；确认 audit record/export/retention report 默认脱敏，`localFileJsonl` 明确不是 production-ready，L3/L4 consent 通过后 executor 前 audit unavailable fail closed；剩余风险：生产 Host/encrypted audit backend、durability/alerting、export approval、privacy deletion 仍待后续 Phase 4/6 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 74]`，工作区无未提交变更；`cargo fmt --check` 通过；`cargo test -p consent-audit audit` 2 unit + 5 integration passed；`cargo test -p dock-core audit` 7 api_call_flow + 3 runtime_facade passed；`cargo test -p consent-audit` 5 unit + 8 integration + doctests passed；`cargo test -p dock-core` 16 api_call_flow + 8 runtime_config + 6 runtime_facade + doctests passed；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/consent-audit crates/dock-core crates/dock-cli crates/anp-adapter docs/security docs/runbook docs/plan` 无输出；敏感词抽样仅命中测试假值、redaction 断言、安全文档、配置红线和既有代码标识，未发现真实 token、Authorization、signature、private key material、手机号、地址、文件内容或生产凭据；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 75]` | 进入 04-08 |
| 04-08 | done | `main` | 2026-06-13 21:01:51 +0800 | 2026-06-13 21:22:02 +0800 | `01a0cec` | 2026-06-13 21:20:37 +0800 commit 前 Review 已记录：确认 sidecar metadata 写在 cache root 下而非 Skill 包目录内，不影响包 digest；dry-run/report 不输出 cache root、本机绝对路径或 package URL secret/query；delete scope 只删除匹配 package dir 与对应 sidecar；rollback pin 与 active retain 会保留；quarantined sidecar 会让后续 reload fail closed；legacy cache 无 sidecar 时只被全量 cleanup 匹配；本 Step 未新增 CLI 命令，CLI/ops cleanup surface 留给 Phase 5/6 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 76]`，工作区无未提交变更；已读取主 Plan、Step 04-08 文档、Phase 4 章节、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-07 closure evidence；`cargo fmt --check` 通过；`cargo test -p skill-loader cache` 7 passed；`cargo test -p skill-loader` 14 package/path tests + 11 registry/cache tests + doctests passed；`cargo test -p dock-cli cache` 通过但 filter 命中 0 tests，本 Step 未触及 CLI surface；`cargo clippy -p skill-loader --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/skill-loader crates/dock-core crates/dock-cli docs/security docs/runbook docs/plan` 无输出；敏感词扫描仅命中文档红线、测试假值、redaction 断言和既有计划文本，未发现真实 token、Authorization、signature、private key material、本机绝对路径或生产凭据进入 cleanup report；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 77]` | 进入 04-09 |
| 04-09 | done | `main` | 2026-06-13 21:23:49 +0800 | 2026-06-13 21:53:01 +0800 | `b6120cc` | 2026-06-13 21:50:47 +0800 commit 前 Review 已记录：修复 Host action outcome 只依赖 custom Host 自行脱敏的问题，改为 Runtime 出口统一二次脱敏；修复 `openDetailPage` 初版只在 headless Host 内 canonicalize 的问题，改为 Runtime 先拒绝 unsafe target，再把 canonical relative target 交给 Host；确认 `api/call` 不进入 Host adapter，仍走 Orchestrator、permission、ConsentGate、audit 和 executor；确认 custom Host unknown action 默认 unsupported fail closed，headless mock `productionReady = false` | `cargo fmt --check` 通过；`cargo test -p dock-core host` 11 matched tests passed；`cargo test -p dock-core` 37 passed；`cargo test -p component-runtime action` 3 matched tests passed；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`cargo clippy -p dock-core --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/component-runtime crates/card-spec crates/dock-cli docs/architecture docs/runbook docs/security docs/plan` 无输出；fixed-string 敏感词扫描仅命中文档红线、redaction marker、测试假值和既有安全计划文本，未发现真实凭据泄露；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 79]` | 进入 04-10 |
| 04-10 | done | `main` | 2026-06-13 21:54:25 +0800 | 2026-06-13 22:28:04 +0800 | `932e2e5` | 2026-06-13 22:22:30 +0800 commit 前 Review 已记录：修复 `requiredForHighRisk` 公开 policy 与实现不一致、session close 未清理本地 replay cache、`concurrency` filter 覆盖不足的问题；确认剩余风险为本地内存级串行/replay、pre-dispatch cancellation/deadline、无分布式 lock 或 provider 侧耐久幂等，均已同步到文档 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 80]`，工作区无未提交变更；已读取主 Plan、Step 04-10 文档、Phase 4 并发/取消/重试与幂等章节、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-09 closure evidence；`cargo fmt --check` 通过；`cargo test -p dock-core concurrency` 初次在测试重命名前只命中 1 test，已重跑后 5 passed；`cargo test -p anp-adapter retry` 1 passed；`cargo test -p component-runtime cleanup` 1 passed；`cargo test -p dock-core` 43 passed；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`cargo clippy -p dock-core --all-targets -- -D warnings` 通过；`cargo clippy -p anp-adapter --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-core crates/anp-adapter crates/component-runtime crates/consent-audit crates/dock-cli docs/security docs/runbook docs/plan docs/architecture` 无输出；敏感词抽样仅命中源码 redaction 逻辑、测试假值和安全/计划文档条目，未发现真实凭据泄露；实现 commit 后 `git status --short --branch` = `## main...origin/main [ahead 81]` | 进入 04-11 |
| 04-11 | done | `main` | 2026-06-13 22:29:59 +0800 | 2026-06-13 22:41:28 +0800 | `c3be4c5`；closure `e149d1c` | 2026-06-13 22:38:26 +0800 Phase 4 最终 Review 已记录：修复 roadmap 顶层 Phase 4 完成标志误导为真实 production Host 已接入的问题，修复 Phase 4 阶段完成检查仍全部未勾选的问题，修复通用 Codex Goal 提示词硬编码 04-01 起点的问题；确认 04-01 至 04-10 的 Runtime API、IPC/headless、registry/cache、config/secret、token/storage/audit/cache、Host adapter/action、concurrency/cancellation/idempotency 证据齐全，未发现需要修改 Phase 4 代码的阻塞问题。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 82]`，工作区无未提交变更；已读取主 Plan、Step 04-11 文档、Phase 4 章节、Phase 4 详细计划、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理、Plan 变更记录和 04-10 closure evidence；04-01 至 04-10 在主台账均为 `done`；04-01 至 04-10 implementation/closure commit hash 均可解析；`cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；敏感词扫描仅命中源码 redaction 逻辑、测试假值、安全/计划文档和 demo-only 示例，未发现真实 token、Authorization、signature、private key material、本机私有路径或生产凭据泄露；final review commit 为 `c3be4c5 docs: record phase4 final review`；closure commit 为 `e149d1c docs: close phase4 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 84]`，工作区无未提交变更。 | 本 Goal 停止在 04-11，不进入 05-01；后续若启动新 Goal，应从 05-01 开始 |
| 05-01 | done | `main` | 2026-06-13 23:39:11 +0800 | 2026-06-13 23:53:11 +0800 | `153027c` | 2026-06-13 23:51:14 +0800 commit 前 Review 已记录：修复 validate `status` / command success 语义歧义和 `skillRoot` 本机绝对路径输出风险；确认 Host provider、persistence backend、snapshot gate 只报告为 not-evaluated 或 requires-fixture-gate，不误标 production-ready。 | `cargo fmt --check` 通过；`cargo test -p dock-cli validate` 4 unit + 1 integration passed；`cargo test -p mcp-schema compatibility` 通过但 filter 命中 0 tests；`cargo run -p dock-cli -- validate examples/coffee-skill` 输出 `dock.validate-report.v1`、`status = warning`、`commandStatus = ok`、`compatibilityLevel = demo-only`，releaseBlockers 含 `production_warning` / `supply_chain`；validate JSON 脱敏抽样未命中 `/home/`、Authorization、Signature、capabilityToken、private key path/material 或 package signature secret；`git diff --check -- crates/dock-cli crates/mcp-schema crates/skill-loader docs/runbook docs/plan README.md` 无输出；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 87]`。 | 进入 05-02 |
| 05-02 | done | `main` | 2026-06-13 23:54:31 +0800 | 2026-06-14 00:13:03 +0800 | `ed5599f` | 2026-06-14 00:10:41 +0800 commit 前 Review 已记录：确认 `inspect` 只做 loader/registration trace/静态扫描，不调用 Skill API 或 Host provider；修复 validation issue 输出脱敏不足和 redaction placeholder 回显敏感 marker 的问题；确认文件树只输出相对路径、类型和大小，`wx.*` static scan 限制已标注 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 88]`；已读取主 Plan、Step 05-02 文档、Phase 5 文档、执行台账、Codex Goal 执行协议、Review/提交门禁、Blocked 处理和 Plan 变更记录；已确认 05-01 implementation commit `153027c` 与 closure commit `d8ae27f`；`cargo fmt --check` 通过；`cargo test -p dock-cli inspect` 2 unit + 1 integration passed；`cargo test -p skill-loader` 14 package/path tests + 11 registry/cache tests + doctests passed；`cargo run -p dock-cli -- inspect examples/coffee-skill` 输出 `dock.inspect-report.v1`、`status = warning`、`commandStatus = ok`、`registeredApisSource = api-vm-registration-trace`；inspect JSON 脱敏抽样未命中 `/home/`、Authorization、Signature、capabilityToken、private、secret 或 token；`git diff --check -- crates/dock-cli crates/skill-loader crates/mcp-schema docs/plan README.md` 无输出；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 89]` | 进入 05-03 |
| 05-03 | done | `main` | 2026-06-14 00:16:15 +0800 | 2026-06-14 02:18:21 +0800 | `aab9653` | 2026-06-14 02:15:15 +0800 commit 前 Review 已记录：修复 fixture report 参数过多、snapshot component 名称推导错误、snapshot normalization、dynamic `brokerCalls` 硬编码、fixture/audit `skillId` 固定 coffee，以及 validate/inspect `skillId` 回归；确认 `test-skill` 复用 RuntimeService / Component Runtime，headless provider 明确 dev-only，report 不输出敏感 marker、本机路径或 fixture token。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 90]`；已读取主 Plan、Step 05-03 文档、Phase 5 文档、Release Gates fixture gate、现有 `dock-cli` command/runtime harness、coffee E2E、`examples/fixtures/*` 和 `testdata/render-ir/*.json`；已确认 05-02 implementation commit `ed5599f` 与 closure commit `31ac65c`；`cargo fmt --check` 通过；`cargo test -p dock-cli fixture` 通过，实际命中 3 个 fixture/test-skill 集成用例；`cargo test -p dock-cli test_skill` 1 unit + 2 integration passed；`cargo test -p dock-cli --test coffee_order_flow` 11 passed；手工 `dock-cli test-skill` 覆盖 coffee、address-form、media-review、dynamic-status、location-map-preview，JSON parse 全部通过；报告敏感串抽样未命中本机路径、Authorization、Signature、capabilityToken、private、secret、fixture-token、Bearer、手机号、真实地址或经纬度；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 91]`。 | 进入 05-04 |
| 05-04 | done | `main` | 2026-06-14 02:19:43 +0800 | 2026-06-14 02:37:52 +0800 | `ac47ba2` | 2026-06-14 02:35:54 +0800 commit 前 Review 已记录：修复目标目录解析无法创建多级目标目录的问题；修复导入后的 coffee 包因目录名变化导致 `test-skill` 降级为通用空参数 fixture 并失败的问题；确认 import 默认 dry-run、safe copy fail closed、patch 仅为人工建议且不标 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 92]`；已读取主 Plan、Step 05-04 文档、Phase 5 文档、Release Gates、现有 `dock-cli` validate/inspect/test-skill 结构、skill-loader path/supply-chain gate 和 05-03 closure evidence；`cargo fmt --check` 通过；`cargo test -p dock-cli import` 7 unit tests passed；`cargo test -p skill-loader` 14 package/path + 11 registry/cache tests passed；`cargo test -p dock-cli --test coffee_order_flow` 11 passed；手工 `import-wechat-mcp examples/coffee-skill --dry-run` 输出 `dock.import-wechat-mcp-report.v1`、`status = dry-run`、`commandStatus = ok`；手工 safe copy 后 `validate` 输出 `dock.validate-report.v1`、`test-skill` 输出 `dock.test-skill-report.v1`、`status = ok`、`fixtureSet = coffee`、`failed = 0`；JSON 脱敏抽样未命中本机路径、Authorization、Signature、capabilityToken、private、secret、fixture-token、Bearer、手机号、真实地址或经纬度；`git diff --check -- crates/dock-cli crates/skill-loader docs/developer docs/plan docs/runbook README.md` 无输出；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 93]`。 | 进入 05-05 |
| 05-05 | done | `main` | 2026-06-14 02:43:16 +0800 | 2026-06-14 02:58:25 +0800 | `9d19744` | 2026-06-14 02:57:06 +0800 commit 前 Review 已记录：修复 `doctor` 在仓库子目录运行时 toolchain/sandbox gate 使用相对路径可能误报的问题；确认 `dock.doctor-report.v1` 覆盖 toolchain/workspace/runtime config/Skill/DID/signing credential permission/resolver/allowlist/storage/audit/Host provider/sandbox/server health，默认 warning/skip 不被误标 production-ready，`--ci` 只在 fail 时返回非零且先输出 JSON。 | `cargo fmt --check` 通过；`cargo test -p dock-cli doctor` 4 passed；`cargo run -p dock-cli -- doctor` 输出 `dock.doctor-report.v1`、`status = warning`、`commandStatus = ok`、summary 为 5 pass / 7 warn / 1 skip / 0 fail；`python3 -m json.tool /tmp/dock-doctor.json` 通过；`cargo test -p dock-cli --test coffee_order_flow` 11 passed；`cargo clippy -p dock-cli --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`git diff --check -- crates/dock-cli crates/anp-adapter crates/dock-core docs/runbook docs/plan README.md` 无输出；doctor JSON 敏感串扫描未命中 `/home/`、Authorization、Signature、capabilityToken、Bearer、raw token、private key material、PEM header 或 secret；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 95]`。 | 进入 05-06 |
| 05-06 | done | `main` | 2026-06-14 02:59:58 +0800 | 2026-06-14 03:16:21 +0800 | `f3d97cc` | 2026-06-14 03:14:20 +0800 commit 前 Review 已记录：确认复用既有 `examples/fixtures/*`，避免重复创建 `examples/address-skill` 等包；修复本地 fixture 在 `validate` / `inspect` 中缺少 manifest `id` 时回退为默认 `coffee` 的报告问题，同时保留 coffee fixture 形状继续输出 `coffee`；确认 expected JSON 只记录稳定摘要，不包含易变 audit 时间戳、本机路径、token、Authorization、Signature、fixture-token、真实手机号、地址、文件内容或经纬度；确认 headless provider 仍标 `productionReady = false`，未将 Host provider 或动态网络能力写成 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 96]`；已读取主 Plan、Step 05-06 文档、Phase 5 文档、现有 `examples/fixtures/*` 和 `testdata/render-ir/*`；确认 05-05 implementation commit `9d19744` 与 closure commit `56daf6f`；实现决策：复用 Step 02-06 已存在的 compatibility fixtures 作为可复制示例体系，补 README/expected JSON/回归测试/文档入口，而不是创建重复 Skill 包；`cargo fmt --check`、`cargo test -p dock-cli example`、`cargo test -p dock-cli validate`、`cargo test -p dock-cli inspect`、四个 fixture 手工 validate/test-skill、`git diff --check -- examples testdata crates/dock-cli docs/architecture docs/runbook docs/plan README.md`、fixture JSON 敏感串扫描、`cargo test -p dock-cli --test coffee_order_flow`、`cargo clippy -p dock-cli --all-targets -- -D warnings`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 均通过；计划敏感词扫描仅命中 mock handle、示例命令、文档红线、测试假值和既有安全说明；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 97]`。 | 进入 05-07 |
| 05-07 | done | `main` | 2026-06-14 03:20:36 +0800 | 2026-06-14 03:29:22 +0800 | `72f00df` | 2026-06-14 03:27:50 +0800 commit 前 Review 已记录：修复导入指南使用 `/tmp` 目标路径的可移植性问题，修复 local demo `runtime-json` 示例包含可复用 capability token 占位的问题，修复 Phase 5 阶段完成检查仍未勾选状态枚举一致性的问题；确认 developer docs 没有把完整微信 Runtime、headless/mock provider、local unencrypted backend 或 demo-only fixture 写成 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 98]`；已读取主 Plan、Step 05-07 文档、Phase 5/6 阶段文档、05-08/06-01 至 06-07 Step 文档、现有 `docs/developer/import-wechat-mcp-skill.md`、README、local demo runbook、API/组件兼容矩阵、release gates 和 Host adapter / Runtime contract 相关文档；确认 05-06 implementation commit `f3d97cc` 与 closure commit `a8df50f`；`git diff --check -- docs/developer docs/runbook docs/plan README.md` 无输出；相对链接检查通过；CLI help 与文档命令一致；无 token `runtime-json` 示例实际运行通过；状态枚举和安全红线抽样符合预期；implementation commit `72f00df docs: add developer migration guides` 后 `git status --short --branch` = `## main...origin/main [ahead 99]`。 | 进入 05-08 |
| 05-08 | done | `main` | 2026-06-14 03:30:37 +0800 | 2026-06-14 03:35:46 +0800 | `18ca5b2`；closure `ebec9b7` | 2026-06-14 03:32:16 +0800 Phase 5 final Review 已记录：确认 05-01 至 05-07 台账和 Step 文档均为 `done`，commit hash、Review 证据和验证证据齐全；CLI schema 覆盖 validate/inspect/test-skill/import/doctor；coffee validate 仍为 `demo-only` warning，doctor 本地默认仍为 warning/skip，未被误标 production-ready；developer docs 与 API/组件矩阵使用同一状态枚举；未发现需要修改 Phase 5 代码的阻塞问题。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 100]`；已读取主 Plan、Step 05-08 文档、Phase 5 文档、release gates、API/组件兼容矩阵、developer docs、05-01 至 05-07 Step 文档和执行台账；05-01 至 05-07 implementation / closure commits 均能在 git history 解析；`cargo metadata --format-version 1 --no-deps`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p dock-cli --test coffee_order_flow`、`git diff --check -- docs/plan docs/architecture docs/runbook docs/developer docs/security README.md AGENTS.md` 均通过；Phase 5 CLI JSON 抽样 validate/inspect/test-skill/import/doctor 均可解析且 schema/status 符合预期；敏感串抽样仅命中文档红线、fixture mock 说明和安全说明，未发现真实敏感输出；final review commit 为 `18ca5b2 docs: record phase5 final review`；closure commit 为 `ebec9b7 docs: close phase5 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 102]`，工作区无未提交变更。 | 进入 06-01 |
| 06-01 | done | `main` | 2026-06-14 03:41:41 +0800 | 2026-06-14 04:03:32 +0800 | `3fb65f0` | 2026-06-14 03:59:38 +0800 commit 前 Review 已记录：确认 `dock-core::observability` 事件 schema、redaction policy、hashed user DID、RuntimeService emit hooks 和 tests 已覆盖当前公共 Runtime 入口；修复前期 UTF-8 截断和构造函数参数过多风险；确认事件 fields 不记录 raw arguments、token、Authorization、Signature、private key path、手机号、地址、文件内容或精确位置。剩余风险：`wx_api_call_*` / `request_*` 当前为稳定 schema 类型，尚未从 QuickJS / RequestBroker 细粒度路径 emit；06-02 必须接入 metrics/tracing bridge 时补真实链路事件和 trace propagation。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 103]`，工作区无未提交变更；已读取主 Plan、Step 06-01 文档、Phase 6 计划、Release Gates、RuntimeService/Orchestrator/Host/Audit/Component Runtime 相关源码和 runtime tests；已确认 Phase 5 final review commit `18ca5b2`、closure commit `ebec9b7`、closure evidence commit `8feb301`；`cargo fmt --check` 通过；`cargo test -p dock-core observability` 通过，实际命中 1 个 observability unit test 和 2 个 runtime observability tests；`cargo test -p dock-core runtime_observability` 通过，实际命中 2 个 runtime observability tests；`cargo test -p dock-core` 46 passed；`cargo test -p dock-cli --test coffee_order_flow` 12 passed；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`git diff --check -- crates/dock-core docs/runbook docs/plan Cargo.toml Cargo.lock` 无输出；敏感串扫描仅命中测试假值、redaction 断言和安全/计划文档红线，未发现真实凭据或隐私 payload 泄露；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 104]`。 | 进入 06-02 |
| 06-02 | done | `main` | 2026-06-14 04:05:17 +0800 | 2026-06-14 04:43:25 +0800 | `7fa8aee` | 2026-06-14 04:35:38 +0800 commit 前 Review 已记录：修复初版缺失 QuickJS VM / `wx.request` / token path metrics hook、缺失 audit span、unsupported API 专项 count、`runtime.negotiateVersion` IPC 分支类型错误和 metrics label 临时借用测试问题；确认 labels 只记录 API/component/risk/outcome/status bucket 等低基数字段，不记录 URL query、headers、body、raw arguments、token、Authorization、Signature、DID 原文或本机绝对路径。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 105]`，工作区无未提交变更；已读取主 Plan、Step 06-02 文档、Phase 6 计划、06-01 closure 证据和 Release Gates；确认 06-01 implementation commit `3fb65f0` 与 closure commit `2e899b0`；`cargo check -p dock-core`、`cargo check -p js-runtime-quickjs`、`cargo fmt --check`、`cargo test -p dock-core metrics`、`cargo test -p dock-core trace`、`cargo test -p js-runtime-quickjs quickjs_executor_records_vm_request_and_token_metrics_with_trace`、`cargo test -p dock-core`、`cargo test -p js-runtime-quickjs`、`cargo test -p dock-cli --test coffee_order_flow`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 均通过；`git diff --check -- crates/dock-core crates/js-runtime-quickjs crates/anp-adapter crates/component-runtime crates/consent-audit docs/runbook docs/plan` 无输出；敏感串扫描仅命中测试假值、redaction marker、文档红线和既有 mock 说明，未发现真实凭据、raw token、Authorization、Signature、DID 原文、本机绝对路径或隐私 payload 泄露；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 106]`。 | 进入 06-03 |
| 06-03 | done | `main` | 2026-06-14 04:45:24 +0800 | 2026-06-14 05:09:10 +0800 | `67a869e` | 2026-06-14 05:06:44 +0800 commit 前 Review 已记录：修复 clippy `manual_clamp`；确认 `dock-cli perf` 覆盖 Skill load、API VM call、component render、Render IR size、token lookup、storage read/write、RSS sample、并发 session、多 Skill、多组件、dynamic timer/request 和 result-size fail-closed；确认 report 是本地硬件相关 baseline，不是 production SLO，且不泄漏敏感字段或本机绝对路径。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 107]`，工作区无未提交变更；已读取主 Plan、Step 06-03 文档、Phase 6 计划、执行台账和 06-02 closure evidence；`cargo fmt --check`、`cargo clippy -p dock-cli --all-targets -- -D warnings`、`cargo test -p dock-cli perf`、`cargo test -p dock-cli --test coffee_order_flow perf_smoke_reports_baselines_and_stress`、`cargo run -p dock-cli -- perf examples/coffee-skill --iterations 1 > testdata/perf/coffee-smoke-baseline.json`、`python3 -m json.tool testdata/perf/coffee-smoke-baseline.json >/tmp/coffee-smoke-baseline.json`、`git diff --check -- crates testdata docs/runbook docs/plan README.md`、artifact 敏感串扫描、`cargo test -p dock-cli`、`cargo test -p dock-cli --test coffee_order_flow`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo test --workspace perf` 均通过；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 108]`，工作区无未提交变更。 | 进入 06-04 |
| 06-04 | done | `main` | 2026-06-14 05:13:33 +0800 | 2026-06-14 05:24:25 +0800 | `afaa5ab` | 2026-06-14 05:22:29 +0800 commit 前 Review 已记录：修复 `--report` 路径、artifact 输出目录 export 和陈旧 artifacts 扫描问题；确认 full mode 实际运行 required gates，skip 不计 pass，redaction / consent / sandbox / token leakage 均为 hard blocker。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 109]`，工作区无未提交变更；已读取主 Plan、Step 06-04 文档、Phase 6 计划、Release Gates runbook、README、API/组件兼容矩阵和 06-03 closure evidence；`bash -n scripts/release-gates.sh` 通过；`./scripts/release-gates.sh --quick --report target/release-gates/quick-report.json` 通过，6 pass / 0 fail / 4 skip，`releaseDecision = needs-review`；`./scripts/release-gates.sh --report target/release-gates/full-report.json` 通过，21 pass / 0 fail / 1 skip，唯一 skip 是未提供 release notes path，`requiredFailed = 0`，`hardBlockerFailed = 0`；两个 report 均通过 `python3 -m json.tool`；Markdown link checker 检查 751 个本地链接；compatibility matrix checker 检查 130 个 status cells；artifact redaction scan 覆盖 `target/release-gates/artifacts`、`testdata/render-ir`、`testdata/perf` 且无敏感命中；`git diff --check -- scripts docs/runbook docs/plan README.md` 无输出；implementation commit 后 `git status --short --branch` = `## main...origin/main [ahead 110]`，工作区无未提交变更。 | 进入 06-05 |
| 06-05 | done | `main` | 2026-06-14 05:26:17 +0800 | 2026-06-14 05:33:50 +0800 | `3b26ba2` | 2026-06-14 05:30:35 +0800 commit 前 Review 已记录：确认 release-process 覆盖版本化对象、release notes 模板、canary stage、rollback conditions/actions、cache purge dry-run 和 audit evidence retention；确认 local canary release notes 只证明 Stage 0，本地/headless/mock/provider/local backend/perf 数字未被误写成 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 111]`，工作区无未提交变更；已读取主 Plan、Step 06-05 文档、Phase 6 发布策略、Release Gates runbook、developer Host adapter 文档、相关 registry/cache/rollback 计划和 06-04 closure evidence；`git diff --check -- docs/runbook docs/plan README.md scripts` 无输出；`./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md --report target/release-gates/release-notes-report.json` 通过，report `dock.release-gates-report.v1` 为 `status = ok`、`releaseDecision = pass`、22 pass / 0 fail / 0 skip、`requiredFailed = 0`、`hardBlockerFailed = 0`；`python3 -m json.tool target/release-gates/release-notes-report.json` 通过；artifact redaction scan 无敏感命中；安全抽样只命中安全规则、流程说明、mock/demo 指引和计划证据，未发现真实 secret、raw token、private key material、本机私有路径或隐私原文；implementation commit `3b26ba2` 后 `git status --short --branch` = `## main...origin/main [ahead 112]`，工作区无未提交变更。 | 进入 06-06 |
| 06-06 | done | `main` | 2026-06-14 05:35:55 +0800 | 2026-06-14 05:46:08 +0800 | `e6c05bd` | 2026-06-14 05:43:22 +0800 commit 前 Review 已记录：修复 local canary release notes 对 06-06 的状态漂移，修复 `operations.md` 不可直接执行的 `runtime-json` 占位命令；确认 runbook 覆盖 Step 要求故障域、privacy deletion scope、audit evidence retention、rollback/cache purge 和 Host-specific gap，且未把本地/headless/mock/dev-only backend 写成 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 113]`，工作区无未提交变更；已读取主 Plan、Step 06-06 文档、Phase 6 文档、Release Gates、Release Process、local demo/security runbook、Phase 4 持久化与 cache cleanup 证据、doctor CLI surface、storage/audit/cache 相关源码和 06-05 closure evidence；`git diff --check -- docs/runbook docs/plan README.md` 无输出；full release gate `./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md --report target/release-gates/06-06-release-notes-report.json` 通过，22 pass / 0 fail / 0 skip，`releaseDecision = pass`；release gate JSON parse 通过；`runtime-json` negotiateVersion dry-run 输出 `ok` 且 JSON 可解析；quick release gate 文档/链接/矩阵/artifact redaction checks 通过，结果按 quick 模式为 `needs-review`；安全抽样只命中文档红线、mock/demo 命令、test fixture 说明和 redaction policy，没有真实 secret、raw token、private key material、本机私有路径或隐私原文；implementation commit `e6c05bd` 后 `git status --short --branch` = `## main...origin/main [ahead 114]`，工作区无未提交变更。 | 进入 06-07 |
| 06-07 | done | `main` | 2026-06-14 05:48:24 +0800 | 2026-06-14 05:56:25 +0800 | `8257147` | 2026-06-14 05:53:04 +0800 Phase 6 final Review 已记录：确认 06-01 至 06-06 台账和 Step 文档均为 `done`，commit hash、Review 证据和验证证据齐全；修复 release gate report command 本机绝对路径泄漏和 release-gates runbook 状态漂移；确认 structured events、metrics/tracing、perf baseline、release gates、canary/rollback、operations/privacy deletion 文档没有把 local/headless/mock/backend、Stage 0 local canary 或本地 perf 数字误写成 production-ready。 | 启动前 `git status --short --branch` = `## main...origin/main [ahead 115]`，工作区无未提交变更；已读取主 Plan、Step 06-07 文档、Phase 6 文档和执行台账；Step 06-01 至 06-06 在主台账与 Step 文档中均为 `done`；implementation / closure commit `3fb65f0`、`2e899b0`、`7fa8aee`、`b26d04c`、`67a869e`、`b8ccfed`、`afaa5ab`、`96f5572`、`3b26ba2`、`8866855`、`e6c05bd`、`4ea90a3` 均可解析；`cargo metadata --format-version 1 --no-deps`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p dock-cli --test coffee_order_flow`、`bash -n scripts/release-gates.sh`、full release gate with release notes、release gate JSON parse、docs diff checks、Phase 6 unchecked-item scan 和 artifact/report redaction scan 均通过；release gate report 为 22 pass / 0 fail / 0 skip、`releaseDecision = pass`；final review commit `8257147` 后 `git status --short --branch` = `## main...origin/main [ahead 116]`，工作区无未提交变更。 | 当前 Phase 5/6 Goal 已关闭；后续 production Host/deploy/provider 接入需通过新 Plan 或新增 Step 执行 |

#### 2.3.4 Codex Goal 执行协议

1. 启动前先执行并记录 `git status --short --branch`，确认是否存在用户未提交改动；若有，必须保护这些改动，不得回滚。
2. 读取顺序：本文、当前 Step 文档、相关 Phase 文档、相关深入子文档、`AGENTS.md`、必要的源文件和测试。
3. 一个 Step 的标准状态流为：`pending` -> `in_progress` -> `review` -> `committed` -> `done`。若被阻塞，记录为 `blocked`，解决后回到 `in_progress` 或 `review`。
4. 每个 Step 必须按对应小 Plan 执行，不得把多个 Step 的完成工作混在一个 commit 中。
5. 每个 Step 都必须同步更新实现、测试、兼容矩阵、runbook 或开发者文档中与该 Step 直接相关的部分；如果某类文档不适用，必须在 Step 记录原因。
6. 每个 Step 都必须运行小 Plan 中列出的验证命令；不能运行时，记录原因、影响和替代证据。
7. 每个 Step 都必须在 commit 前进行 Review，记录发现、修复、剩余风险、新增或缺失测试、已更新或缺失文档。
8. 只有 Review 必要问题已修复或明确记录、验证证据已记录、commit 已创建并回填台账后，才能把 Step 标为 `done`。
9. 任何改变范围、顺序、验收标准、公开契约、数据模型、配置、验证策略或安全边界的决定，必须先更新本文的 Plan 变更记录和受影响 Step 文档。
10. 完成全部 Step 后，执行最终全局 Review 与整体验证，不得只依赖各 Step 的局部验证。

#### 2.3.5 Review 与提交门禁

每个 Step 的 Review 是 commit 前门禁，不是可选收尾。Review 至少覆盖：

- 正确性：实现是否满足 Step 验收标准和阶段目标；
- 回归风险：是否破坏 coffee demo、现有 VM、组件、DID、consent/audit 路径；
- 公开契约：`SKILL.md`、`mcp.json`、`structuredContent`、`_meta.ui.componentPath`、Render IR、CLI JSON 是否漂移；
- 安全与隐私：token、`Authorization`、HTTP signature、private key path、手机号、地址、文件内容是否脱敏；
- 测试覆盖：是否有 unit、integration、fixture、snapshot 或文档检查证据；
- 文档同步：兼容矩阵、runbook、开发者文档、阶段文档是否与实现一致。

提交要求：

- 每个完成的 Step 创建一个 focused commit，commit message 建议格式为 `phase<N>: <step outcome>` 或 `docs: <step outcome>`。
- commit 前记录 `git status --short`、纳入文件和不纳入文件。
- commit 后记录 commit hash 与 `git status --short --branch`。
- 上一步完成工作未提交前，不得开始依赖它的下一步，除非 Plan 变更记录明确说明依赖原因和风险控制。
- 最终全局 Review 如果修改文件，需要单独 Review、验证并创建最终集成 commit。

#### 2.3.6 Blocked 处理

| Blocker | Step | 证据 | 已尝试方案 | 影响范围 | 下一步决策 |
|---|---|---|---|---|---|
| 待记录 | 待记录 | 待记录 | 待记录 | 当前步骤 / 整体计划 | 待记录 |

处理规则：

1. Blocker 必须写清楚触发命令、文件、错误输出或缺失决策，不得只写“实现困难”。
2. 若 blocker 只影响当前 Step，且后续某个 Step 在依赖表中不依赖它，执行者可以在记录风险后转入该独立 Step；当前 Step 批次默认不并行，除非更新本文。
3. 若 blocker 影响公开契约、安全边界、数据模型或阶段顺序，必须先更新 Plan 变更记录，再决定是否继续。
4. 只有没有安全假设、替代方案或独立下一步时，才向用户提问。

#### 2.3.7 Plan 变更控制

| 日期 | 变更 | 原因 | 影响步骤 | 是否需要 Review |
|---|---|---|---|---|
| 2026-06-12 | 新增 Codex Goal 执行控制、执行台账和 Phase 0/Phase 1 首批 Step 文档 | 将 roadmap 补强为可恢复执行的 AWiki 长跑计划 | 00-01 至 01-04 | 是 |
| 2026-06-12 | 新增 Phase 1 剩余 `planned-p1` 与 Phase 2 P1 下一批 Step 文档 | 将首批完成后的残余 API 缺口和组件运行时起步能力拆成可由 Codex Goal 继续执行的小 Plan | 01-05 至 02-06 | 是 |
| 2026-06-12 | 新增 Phase 3 至 Phase 6 全部后续里程碑 Step 文档 | 将安全增强、生产运行时/Host、开发者体验、观测发布运营阶段拆成可由 Codex Goal 顺序执行的小 Plan | 03-01 至 06-06 | 是 |
| 2026-06-12 | 修复计划 Review 发现：新增 02-07 final Review gate、前置 dynamic sandbox gate、拆分 Phase 4 持久化、标注 01-07 blocked 跳转 | 让 Codex Goal 恢复执行时不会跳过批次最终 Review，避免动态组件先于安全 gate 扩权，保持每个 Step 一个 focused commit | 01-07、02-05、02-06、02-07、03-01、04-04 至 04-10、05-05、06-01、06-03、06-05、06-06 | 是 |
| 2026-06-13 | 同步 Step 拆分表状态、恢复指针和 02-07 closure evidence | 修复 Step 拆分表与执行台账不一致、final Review 关闭提交表述滞后、以及后续 Goal 仍显示从 01-05 开始的问题 | 01-05 至 02-07、03-01 | 是 |
| 2026-06-13 | 新增 Phase 3 final Review gate | 按当前 Codex Goal 要求，Step 03-06 完成后必须执行可追踪的 Phase 3 最终 Review 与整体验证，不能直接进入 04-01 | 03-06、03-07、04-01 | 是 |
| 2026-06-13 | 新增 Phase 4 final Review gate | 按当前 Codex Goal 要求，Step 04-10 完成后必须执行可追踪的 Phase 4 最终 Review 与整体验证，不能直接进入 05-01 | 04-10、04-11、05-01 | 是 |
| 2026-06-13 | 新增 Phase 5 和 Phase 6 final Review gate | 按当前 Codex Goal 要求，Phase 5 完成后必须先执行可追踪的阶段 Review 才能进入 Phase 6；Phase 6 完成后必须执行可追踪的最终 Review 与整体验证才能结束当前 Goal | 05-07、05-08、06-01、06-06、06-07 | 是 |

变更规则：

- 先改 Plan，再改实现；不得先扩大实现范围再补文档。
- 变更必须说明是否影响依赖、验收标准、验证命令、Review 范围和 commit 策略。
- 受影响 Step 已经开始时，必须同步更新该 Step 的执行状态和变更记录。

#### 2.3.8 最终全局 Review 与整体验证

触发条件：当前批次 Step 或后续扩展 Step 全部 `done`，且每个 Step 都已有 Review 证据、验证证据和 commit hash。历史执行记录只覆盖当时已完成的 Step；新增 Step 完成后必须追加新的最终全局 Review 记录。

最终 Review 范围：

- 全部变更文件、公开契约、测试、fixtures、兼容矩阵、runbook、开发者文档；
- `wx.*` / `wx.modelContext` 行为与小程序 MCP 兼容策略；
- ANP DID、capability token、allowlist、consent、audit、redaction 和 sandbox 安全边界；
- 执行台账是否与 git history、验证命令和 Step 文档一致；
- 是否存在未提交变更、未解决 Review 发现、跳过验证或文档漂移。

整体验证基线：

```bash
cd anp/anp-miniapp-dock
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p dock-cli --test coffee_order_flow
```

文档和计划验证：

```bash
cd anp/anp-miniapp-dock
git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md
```

若命令因环境、依赖或时间无法运行，必须记录原因、影响、替代检查和剩余风险。最终 Review 若修改文件，必须单独记录 Review、验证和最终集成 commit。

最终 Review 执行记录：

> 以下记录按批次追加；每条记录只覆盖该条 `范围` 中列出的 Step 和文档。

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-12 12:31:47 +0800 |
| 范围 | Roadmap 执行台账、Step 00-01 至 01-04 文档、Phase 1 子文档、兼容矩阵、release gates、local demo runbook、threat model、相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 00-01 至 01-04 均为 `done`；未发现 pending / in_progress / review / blocked / committed Step；台账记录的主产物 commit `de4c3e2`、`22e7f25`、`fea9d35`、`04448a1`、`10db676`、`ec46e1f`、`1504eff`、`e9cbdbe` 均能在 git history 解析。 |
| Review 发现与修复 | 修复文档漂移：`current-capability-baseline.md` 明确为 Step 00-01 时点基线，避免与 01-03/01-04 后实时能力混淆；`phase-1-wx-capability-broker.md` 和 `phase-1-wx-api-bridge-contract.md` 标出已由 01-03/01-04 证明的验收项，并保留 storage、L3/L4 provider、全量 unsupported stub、production Host RequestBroker 等后续工作。 |
| 安全/敏感信息 Review | 运行敏感词扫描命中预期的测试夹具、redaction 代码、安全文档和 demo-only 文档；未发现真实 secret、真实 DID private key、真实 bearer token 或生产凭据。测试继续覆盖 CLI/demo redaction、JS auth header fail closed、response auth/token header redaction、challenge proof redaction 和 audit redaction。 |
| 残余风险 | 首批 Step 已完成；production Host RequestBroker transport、registry allowlist、persistent request/audit store、logout/revocation list、全量 unsupported API stub、storage JS bridge、L3/L4 Host provider conformance 仍按 Phase 2/3/4/5 计划推进，不作为本次首批 Step 完成条件。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 3 passed；`cargo run -p dock-cli -- validate examples/coffee-skill` 通过并保持 `compatibilityLevel: demo-only`；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 通过。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 17]`；最终 Review 文档提交为 `636f1e9 docs: record final production readiness review`；最终台账关闭提交后 `git status --short --branch` = `## main...origin/main [ahead 19]`，且工作区无未提交变更。 |

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-12 23:04:43 +0800 |
| 范围 | Roadmap 执行台账、Step 01-05 至 02-06 文档、Phase 1/2 子文档、wx API / component 兼容矩阵、release gates、threat model、README、fixtures、Render IR snapshots、相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 01-05 至 02-07 均为 `done`，且有 commit hash、Review 证据和验证证据；`02-07` 是 03-01 前可追踪 final Review gate。台账记录的主产物 commit `8e475dd`、`1599294`、`50cc245`、`33591f0`、`0cfea24`、`79417d5`、`c8bb813`、`cc7b3b8`、`7baca29`、`f778a14`、`2f0d122` 均能在 git history 解析；final ledger closure commit 为 `8cd9b80`。 |
| Review 发现与修复 | 修复文档漂移：`phase-2-component-runtime-alignment.md` 不再把所有 P1 Component JS/WXML/WXSS 能力整体标为完成，改为明确当前批次只覆盖 WXML/WXSS P1 子集、表单/静态媒体、dynamic 和 fixture 能力；`this.triggerEvent()` 与 `preloadDetailPage()` 仍保留为 `planned-p1`，后续需单独拆 Step 或通过 Plan 变更处理。 |
| 安全/敏感信息 Review | dynamic request/timer 已在 02-05 前置通过 sandbox escape/resource-limit gate，并在 02-06 dynamic-status snapshot 证明 broker boundary 与响应头脱敏；严格 fixture/snapshot 禁用串扫描无命中。广义敏感词扫描仅命中 redaction 规则、安全说明、mock/dev-only 示例、计划台账和测试假值，未发现真实 secret、真实 token、真实地址、手机号、精确经纬度或本机路径写入 fixture/snapshot。 |
| 残余风险 | 真实 Host renderer/provider/conformance、production network transport/background scheduler、persistent audit/request store、权限策略引擎、token revoke/replay、Skill 包签名、`triggerEvent()`、`preloadDetailPage()` 仍待 Phase 3/4/5 或后续拆分 Step；本批次不能被解释为 production release 完成。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo test -p component-runtime snapshot` 通过；`cargo test -p dock-cli fixture` 1 passed；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 42]`，无未提交完成工作；final review commit 为 `2f0d122 docs: record phase1 phase2 final review`；final ledger closure commit 为 `8cd9b80 docs: close phase1 phase2 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 44]`，工作区无未提交变更。 |

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-13 16:10:22 +0800 |
| 范围 | Roadmap 执行台账、Step 03-01 至 03-07 文档、Phase 3 子文档、Threat Model、Release Gates、wx API / component 兼容矩阵、相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 03-01 至 03-06 均为 `done`，且有 commit hash、Review 证据和验证证据；03-07 是进入 04-01 前可追踪 final Review gate。台账记录的 commit `a61a7e7`、`1c4e784`、`32ada09`、`24a8f10`、`e7c9f49`、`b9c767b` 和 03-06 closure `b70fd1b` 均能在 git history 解析。 |
| Review 发现与修复 | 修复文档漂移：恢复指针和通用 Codex Goal 提示词仍指向 03-01，已改为 Phase 3 完成后的下一个未完成 Step 04-01；`phase-3-security-hardening.md` 仍把 03-07 标为未完成，已改为 final Review gate 完成。未发现需要修改 Phase 3 代码的阻塞问题。 |
| 安全/敏感信息 Review | Threat Model、Release Gates 和 Phase 3 文档均保留 CI 自动化、生产 Host 配置、真实 registry/cache、生产签名 verifier、secret store、持久化 token cache/revocation restore 等残余风险，没有误写成已完成；sandbox、permission、DID/token、Consent/Audit、package integrity 的本地 required gates 均保持 fail closed。敏感词抽样仅命中测试假值、redaction 断言、安全文档、runbook 和 demo-only placeholder，未发现真实 secret、token、proof、private key path、package signature value、手机号、地址、文件内容或隐私原文输出。 |
| 残余风险 | Phase 3 已完成本地安全 gate 和最终 Review；CI gate 自动化、生产 Host/registry/cache、生产签名 verifier、部署级 secret store、持久化 token cache/revocation restore、生产 Host UI/conformance、真实 registry zip extraction 和运维发布自动化仍待 Phase 4/6。Coffee demo 仍为 `demo-only`，不能解释为 production release 完成。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 4 passed；`cargo run -q -p dock-cli -- validate examples/coffee-skill` 输出 `compatibilityLevel: demo-only`、`supplyChain.status = demo-unsigned`、releaseBlockers 含 `supply_chain`；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；Phase 3 commit hash 审计通过。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 58]`，未提交变更仅为 03-07 final Review 文档回填；final review commit 为 `e888b24 docs: record phase3 final review`；ledger closure commit 为 `9f884ac docs: close phase3 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 60]`，工作区无未提交变更。 |

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-13 22:38:26 +0800 |
| 范围 | Roadmap 执行台账、Step 04-01 至 04-11 文档、Phase 4 子文档、Runtime/Host contract 文档、Threat Model、Release Gates、component 兼容矩阵、相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 04-01 至 04-10 均为 `done`，且有 commit hash、Review 证据和验证证据；04-11 是进入 05-01 前可追踪 final Review gate。台账记录的 implementation / closure commit `1b470d5`、`2f299db`、`53e71be`、`fb2d36d`、`81c32c9`、`f004bb4`、`189ad87`、`3713899`、`f742304`、`db9f457`、`67237ba`、`6b09301`、`c8f4a96`、`92ce9c8`、`01a0cec`、`1620fb4`、`b6120cc`、`d4cf617`、`932e2e5`、`259ccee` 均能在 git history 解析。 |
| Review 发现与修复 | 修复文档漂移：roadmap 顶层 Phase 4 完成标志仍暗示“至少一个真实 Host 已通过稳定协议接入”，已改为 Runtime/Host contract、headless conformance、持久化边界和 release blockers 可审计；Phase 4 子文档阶段完成检查仍未勾选，已改为带 release blocker 限定的已完成项；通用 Codex Goal 提示词仍硬编码 04-01 起点，已改为从主台账第一个非 `done` Step 恢复。未发现需要修改 Phase 4 代码的阻塞问题。 |
| 安全/敏感信息 Review | Phase 4 文档继续保留真实 production Host UI/provider、HTTP/gRPC sidecar、远端 registry download、生产签名 verifier、Host/encrypted token/storage/audit backend、分布式 lock、durable idempotency、provider cancellation、CI/ops 自动化等 release blockers；未把 demo/headless/mock/local 未加密能力写成 production-ready。敏感词扫描仅命中源码 redaction 逻辑、测试假值、安全/计划文档和 demo-only 示例，未发现真实 token、Authorization、signature、private key material、本机私有路径或生产凭据泄露。 |
| 残余风险 | Phase 4 已完成本地 Runtime API、headless IPC、registry/cache、config/secret、token/storage/audit/cache contract、Host adapter/action protocol 和本地 concurrency/idempotency gate；真实 production Host UI/provider/conformance、HTTP/gRPC sidecar、真实远端 registry download、生产签名 verifier/publisher policy、生产加密持久化 backend、部署级 audit/export/privacy deletion、跨进程 lock、merchant/provider durable idempotency、metrics/CI/ops 自动化仍待 Phase 5/6 或后续生产接入 Step。Coffee demo 与 headless/local backend 仍不能解释为 production release 完成。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 8 passed；`git diff --check -- docs/plan docs/architecture docs/runbook docs/security README.md AGENTS.md` 无输出；Phase 4 commit hash 审计通过。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 82]`；final review commit 为 `c3be4c5 docs: record phase4 final review`；ledger closure commit 为 `e149d1c docs: close phase4 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 84]`，工作区无未提交变更。 |

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-14 03:32:16 +0800 |
| 范围 | Roadmap 执行台账、Step 05-01 至 05-08 文档、Phase 5 子文档、developer docs、Release Gates、README、local demo runbook、examples fixtures、test-skill expected JSON、Render IR snapshots、相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 05-01 至 05-07 均为 `done`，且有 commit hash、Review 证据和验证证据；05-08 是进入 06-01 前可追踪 final Review gate。台账记录的 implementation / closure commit `153027c`、`d8ae27f`、`ed5599f`、`31ac65c`、`aab9653`、`7d78aea`、`ac47ba2`、`4079220`、`9d19744`、`56daf6f`、`f3d97cc`、`a8df50f`、`72f00df`、`122adfb` 均能在 git history 解析。 |
| Review 发现与修复 | 未发现需要修改 Phase 5 代码的阻塞问题。确认 `validate`、`inspect`、`test-skill`、`import-wechat-mcp`、`doctor` 的 schema/status/commandStatus 语义稳定；developer docs 与 API/组件矩阵使用同一状态枚举；Phase 5 子文档阶段完成检查已全勾选；05-07 已修复导入指南 `/tmp` 示例、`runtime-json` token 示例和文档枚举勾选漂移。 |
| 安全/敏感信息 Review | coffee validate 仍保持 `compatibilityLevel = demo-only` 且含 release blockers；doctor 本地默认仍为 `warning`，`skipCountsAsPass = false`，未把缺 Host provider、未签名包、local/headless/mock/backend 写成 production-ready。敏感串抽样仅命中文档红线、fixture mock 说明和安全说明，未发现真实 token、Authorization、signature、private key material、手机号、真实地址、文件内容、本机绝对路径或真实隐私原文输出。 |
| 残余风险 | Phase 5 已完成开发者 CLI、fixtures、导入/诊断工具和文档闭环；生产 Host provider/renderer、HTTP/gRPC sidecar、加密 token/storage/audit/cache backend、CI release automation、observability/metrics/tracing、privacy deletion 和真实 registry/signature verifier 仍待 Phase 6 或后续生产接入，不作为 Phase 5 完成条件。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 12 passed；`git diff --check -- docs/plan docs/architecture docs/runbook docs/developer docs/security README.md AGENTS.md` 无输出；Phase 5 CLI JSON 抽样通过：validate `dock.validate-report.v1` / warning / demo-only，inspect `dock.inspect-report.v1` / warning，test-skill `dock.test-skill-report.v1` / ok / 3 passed / 0 failed，import `dock.import-wechat-mcp-report.v1` / dry-run，doctor `dock.doctor-report.v1` / warning / 5 pass / 7 warn / 1 skip / 0 fail。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 100]`，未提交变更仅为 05-08 final Review 文档回填；final review commit 为 `18ca5b2 docs: record phase5 final review`；ledger closure commit 为 `ebec9b7 docs: close phase5 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 102]`，工作区无未提交变更。 |

| 项目 | 记录 |
|---|---|
| 执行时间 | 2026-06-14 05:53:04 +0800 |
| 范围 | Roadmap 执行台账、Step 06-01 至 06-07 文档、Phase 6 子文档、Release Gates、release/canary/rollback runbook、operations/troubleshooting/privacy deletion runbooks、release gate runner、README、Phase 6 相关源码/测试和 git history。 |
| Step/ledger 审计 | 执行台账中 06-01 至 06-06 均为 `done`，且有 commit hash、Review 证据和验证证据；06-07 是当前 Phase 5/6 Goal closure 前可追踪 final Review gate。台账记录的 implementation / closure commit `3fb65f0`、`2e899b0`、`7fa8aee`、`b26d04c`、`67a869e`、`b8ccfed`、`afaa5ab`、`96f5572`、`3b26ba2`、`8866855`、`e6c05bd`、`4ea90a3` 均能在 git history 解析。 |
| Review 发现与修复 | 修复 release gate report redaction 问题：artifact redaction scan 的 report `command` 字段曾写入 `$ARTIFACT_DIR` 本机绝对路径，已改为记录仓库相对 target；修复 `docs/runbook/release-gates.md` 顶部状态仍写 Phase 6 observability gates 进行中的文档漂移，改为 Phase 6 本地 gates 已完成并保留生产 Host blocker。未发现需要修改 Phase 6 runtime 行为的阻塞问题。 |
| 安全/敏感信息 Review | structured events 默认 hash/redact user DID 与敏感字段；metrics/tracing labels 保持低基数，不记录 URL query、headers、body、raw arguments、token、Authorization、Signature、DID 原文或本机绝对路径；perf artifact 是本地硬件相关 baseline，不是 production SLO；local canary 只证明 Stage 0 headless/local dry-run。严格 artifact/report scan 仅命中 release report hard blocker 名称 `Authorization or Signature leakage`，未发现真实 secret、raw token、private key material、手机号、真实地址、文件内容、本机绝对路径或精确位置进入 evidence artifacts。 |
| 残余风险 | Phase 6 已完成本地观测事件、metrics/tracing、performance smoke、release gate runner、release notes canary dry-run、rollback/runbook 和 privacy deletion runbook；真实 production Host secure store、encrypted storage/audit backend、deploy platform、traffic router、provider conformance、production privacy deletion job/approval workflow、vendor exporter 和生产 SLO 仍是后续生产接入 blocker。Coffee demo、headless provider、mock/local backend 和 Stage 0 local canary 仍不能解释为 production release 完成。 |
| 整体验证 | `cargo metadata --format-version 1 --no-deps` 通过；`cargo fmt --check` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace` 通过；`cargo test -p dock-cli --test coffee_order_flow` 13 passed；`bash -n scripts/release-gates.sh` 通过；`./scripts/release-gates.sh --release-notes docs/runbook/releases/2026-06-14-local-canary.md --report target/release-gates/06-07-release-notes-report.json` 通过，22 pass / 0 fail / 0 skip，`releaseDecision = pass`，`requiredFailed = 0`，`hardBlockerFailed = 0`；release gate report JSON parse 通过；`git diff --check -- docs/plan docs/architecture docs/runbook docs/developer docs/security README.md AGENTS.md` 无输出；`git diff --check -- scripts docs/runbook docs/plan README.md` 无输出；Phase 6 子 Step 未完成验收项扫描无命中；artifact/report redaction scan 通过。 |
| 最终工作区状态 | 记录 Review 前 `git status --short --branch` = `## main...origin/main [ahead 115]`，未提交变更仅为 06-07 final Review 记录、release gates runbook 状态修复和 release gate report path redaction 修复；final review commit 为 `8257147 docs: record phase6 final review`；final review commit 后 `git status --short --branch` = `## main...origin/main [ahead 116]`，工作区无未提交变更；closure commit 为 `ce0ac35 docs: close phase6 final review gate`；closure 后 `git status --short --branch` = `## main...origin/main [ahead 117]`，工作区无未提交变更。 |

#### 2.3.9 Codex Goal 提示词

下面提示词用于启动后续实现型 Codex Goal。执行时仍以本文和 Step 文档为准；不要硬编码历史起点，必须从主台账第一个非 `done` Step 恢复。若当前 Goal 明确限定只执行某个 Phase，则该 Phase final Review Step 完成后停止，不自动进入下一 Phase。

```text
请以 anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md 为唯一规划入口，按文档执行生产化计划。当前从主台账第一个非 done Step 开始，不依赖聊天历史或旧提示词中的固定 Step 编号。

开始前先读取：
- anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md
- 当前第一个未 done 的 Step 文档
- 主 Plan 的执行台账、Codex Goal 执行协议、Review 与提交门禁、Blocked 处理、Plan 变更记录
- 当前 git status --short --branch

请从第一个状态不是 done 的 Step 开始，一次只执行一个 Step。每步都要按对应小 Plan 实现、验证、Review、修复或记录 Review 发现，然后创建一个 focused commit，并回填主 Plan 执行台账和 Step 执行状态。

需要改变范围、顺序、验收标准、公开契约、数据模型、安全边界或验证策略时，先更新 Plan 变更记录和受影响 Step 文档。不得绕过 ANP DID、capability token、allowlist、ConsentGate、audit、redaction 和 sandbox 边界。

所有目标范围内的步骤完成后，执行对应 final Review 和整体验证，记录实际命令、通过/失败/跳过数量、失败或跳过原因、剩余风险和最终工作区状态；如果 Goal 只限定某个 Phase，完成该 Phase final Review 后停止。
```

若需要在尚未执行该批次的工作区补跑 Phase 1/2 执行型 Codex Goal，可使用更窄的提示词。它只执行 Phase 1 剩余 Step 和 Phase 2 全部 Step，完成 Step 02-07 后停止，不进入 Phase 3：

```text
请以 anp/anp-miniapp-dock/docs/plan/production-readiness-roadmap.md 为唯一规划入口，只执行 Phase 1 剩余 Step 和 Phase 2 全部 Step：从 01-05 开始，做到 02-07 完成后停止，不进入 03-01。

开始前先读取主 Plan、当前 Step 文档、执行台账、Codex Goal 执行协议、Review 与提交门禁、Blocked 处理、Plan 变更记录，以及当前 git status --short --branch。

请按执行台账从第一个非 done Step 继续，一次只执行一个 Step。每个 Step 都必须按对应小 Plan 实现、验证、Review、修复或记录 Review 发现，创建一个 focused commit，并回填主 Plan 执行台账和 Step 执行状态。

Step 01-07 独立于 01-06；只有当 01-06 被记录为 blocked 且无未提交完成工作时，才可按 Blocked 规则串行跳转到 01-07。Step 02-05 开放 dynamic request/timer 前必须先通过 component sandbox escape/resource-limit gate。Step 02-06 完成后必须执行 02-07 批次最终 Review 与整体验证。

不要启动 Phase 3。02-07 完成、提交并回填台账后结束本 Goal，报告 commit hash、验证证据、剩余风险和最终 git status。
```

## 3. Phase 0：基线冻结与产品化门槛

### 3.1 阶段整体计划

Phase 0 的目标不是新增功能，而是把当前 Demo 能力、缺口和上线门槛固化下来，避免后续开发变成零散 patch。

主要工作包：

- 冻结当前 P0.5 能力清单与验证命令；
- 建立 `wx.*` API 和组件兼容矩阵；
- 定义线上产品的 release gate、测试 gate、安全 gate；
- 将后续 Phase 拆成可创建 issue / milestone 的 backlog。

阶段产出物：

- `docs/architecture/wx-api-compatibility-matrix.md`：接口/API 支持等级、映射策略、错误语义、优先级。
- `docs/architecture/component-compatibility-matrix.md`：组件、事件、WXML、WXSS、动态组件支持等级。
- `docs/security/threat-model.md`：容器威胁模型与安全基线。
- `docs/runbook/release-gates.md`：每次 release 前必须通过的命令、fixture、审计项。

### 3.2 细分小阶段与实施方案

#### 0.1 当前能力盘点与基线固化

实施方案：

1. 将当前 workspace crate、CLI 命令、demo-server endpoint、coffee Skill 能力整理成表格。
2. 为每项能力标注证据：对应 crate、测试文件、runbook 命令或 demo 输出。
3. 将 `cargo metadata --format-version 1 --no-deps`、`cargo test --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings` 设为基础 gate。
4. 在文档中明确当前 demo-only 能力，例如 localhost `wx.request` bridge、mock payment、mock consent provider、非生产 FastAPI 示例。

验收：

- 文档能回答“当前已经做到什么、由哪个 crate 负责、哪些测试证明”。
- 任何新功能都能挂到已有能力图或明确新增模块。

#### 0.2 建立接口兼容矩阵

实施方案：

1. 从 `docs/weichat-miniapp-mcp-protocol/weichat-miniapp-mcp.txt` 抽取 API 列表，按原子接口环境、原子组件环境、半屏页面环境分类。
2. 为每个 API 标注：`supported`、`host-boundary`、`planned-p1`、`planned-p2`、`unsupported-by-design`、`demo-only`。
3. 对每个 planned API 定义映射策略：
   - 原样 JS 语义；
   - Host capability；
   - ANP DID 替代；
   - mock / fallback；
   - deterministic unsupported error。
4. 定义 callback、Promise、`errMsg`、错误码、敏感字段返回规则。

验收：

- 每个小程序 MCP 列表中的 API 都有明确状态，不允许出现“未知”。
- 不支持的 API 有原因，不因兼容压力进入完整微信 Runtime 复刻。

#### 0.3 建立组件兼容矩阵

实施方案：

1. 将组件支持列表拆成内置组件、Component JS、WXML、WXSS、事件、`wx.modelContext`、动态组件。
2. 将当前 P0 支持能力和目标 P1/P2 能力分离。
3. 明确哪些能力只输出 Render IR，不要求当前提供完整 UI 展现。
4. 为每个能力绑定 fixture 或未来 golden snapshot。

验收：

- 交易型卡片必须能力和可延期能力区分清晰。
- Host renderer 不再阻塞 component runtime 的契约演进。

#### 0.4 定义 release gates 与 milestone backlog

实施方案：

1. 为每个 Phase 建立 Definition of Done。
2. 将每个小阶段拆成 issue 粒度：输入、输出、影响 crate、测试、文档。
3. 规定每次开发必须同步更新兼容矩阵和 runbook。
4. 建立失败回滚规则：任何新 API 默认 fail closed，不能静默 mock 成成功。

验收：

- 后续代码开发可以直接按 issue/milestone 执行。
- 文档、测试、runbook 与实现同步成为 release 条件。

## 4. Phase 1：接口对齐与 wx Capability Broker

### 4.1 阶段整体计划

Phase 1 的目标是补齐原子接口环境的核心能力，使容器底层对 Skill 暴露的接口尽量对齐小程序 MCP 和微信标准 API 的调用方式。重点不是一次性支持所有微信 API，而是建立统一的 `wx Capability Broker`，让已支持、后续支持和明确不支持的 API 都有一致语义。

主要工作包：

- 完整化 Skill package / manifest 校验；
- 扩展原子接口 JS bridge 的 `wx.modelContext`；
- 把 `wx.login` / `wx.checkSession` / `wx.request` / storage / payment / privacy APIs 纳入统一 capability broker；
- 对齐 callback / Promise / `errMsg` 语义；
- 引入接口级权限声明、allowlist 和风险分级。

阶段完成标志：

- coffee Skill 不依赖 demo-only 特殊路径也能通过统一 broker 运行；
- 一个新的交易型 Skill 可以使用登录、请求、存储、下单、支付确认、地址/手机号授权等核心能力；
- 不支持 API 返回稳定、可测试、可脱敏的错误，不会访问宿主资源。

### 4.2 细分小阶段与实施方案

#### 1.1 Skill package 与 manifest 对齐

实施方案：

1. 扩展 validator 支持小程序 MCP 关键约束：
   - `SKILL.md` 单文件和长度限制；
   - `mcp.json` 长度统计规则；
   - `apis[].name` 与注册名一致；
   - `inputSchema` 必须为对象；
   - `outputSchema` mismatch 作为 warning；
   - `format: "image" | "file"` 输入字段识别；
   - `_meta.ui.componentPath` 和 `components[].path` 关系；
   - `components[].relatedPage`、`permissions.scope.dynamic`、`expirable`、`expiredText`。
2. 增加可选 `app.json` / `AGENTS.md` 读取规划：
   - 不要求 P1 完整支持小程序分包；
   - 可识别 `agent.skills[]` 作为多 Skill registry 输入；
   - 保留单 Skill 目录作为当前默认路径。
3. 校验策略分层：
   - spec error：不能加载；
   - compatibility warning：可加载但报告降级；
   - production warning：demo 可用但上线不允许。

涉及模块：`mcp-schema`、`skill-loader`、`dock-cli validate`。

验收：

- `dock-cli validate` 输出机器可读兼容性报告。
- 每个 manifest warning 都有修复建议或明确降级行为。

#### 1.2 `wx.modelContext` 原子接口 API 对齐

实施方案：

1. 在 Atomic API VM 注入：
   - `wx.modelContext.getSessionId()`；
   - `wx.modelContext.expireAllCards({ componentPaths, match })`；
   - `wx.modelContext.NotificationType` 常量；
   - 未来多 Skill 场景下的 `createSkill(skillPath)` 路径校验。
2. 将卡片过期从组件 VM 行为扩展为 runtime-level card event，不直接耦合 CLI demo。
3. 规定 `expireAllCards` 的策略：
   - 只影响声明 `expirable: true` 的组件；
   - `componentPaths` 必须 canonicalize；
   - `match: latest` 与 all 行为明确；
   - 操作进入 audit。

涉及模块：`js-runtime-quickjs`、`wx-compat`、`dock-core`、`component-runtime`。

验收：

- 原子接口 JS 可直接调用上述 API。
- CLI 和测试能观察到 card expiration event。

#### 1.3 `wx.login` / `wx.checkSession` 生产化

实施方案：

1. 把当前 `HostDidAuthConfig` 提炼为正式 `DidAuthSessionManager`：
   - key：`merchantDid + userDid + agentDid + skillId + sessionId + serverBaseUrl`；
   - token 只由 host 持有，默认不暴露给 Skill；
   - 支持 token refresh、过期清理、强制登出、会话隔离。
2. 对齐微信语义：
   - `wx.login()` 返回 code-like receipt 与 `errMsg`；
   - `wx.checkSession()` 校验 session/token 状态；
   - callback 和 Promise 同时支持；
   - DID proof、token、Authorization 不进入模型可见输出。
3. 服务端契约固定：
   - challenge 字段包含 `challengeId`、`nonce`、`merchantDid`、`issuedAtMs`、`expiresAtMs`、`audience`；
   - login 请求携带 signed challenge；
   - 返回 scoped capability token 和 expiry；
   - replay、audience mismatch、scope mismatch 必须失败。

涉及模块：`anp-adapter`、`js-runtime-quickjs`、`demo-server`、`examples/coffee-fastapi-server`。

验收：

- 同一 session 重复 `wx.login` 使用缓存或刷新策略；
- token 不出现在 JS result、CLI output、日志和 audit 中；
- Rust demo-server 与 FastAPI 示例使用同一契约。

#### 1.4 `wx.request` 与网络能力对齐

实施方案：

1. 将当前 demo-only localhost bridge 替换/下沉到统一 `RequestBroker`：
   - 支持 method、headers、data、timeout、responseType、statusCode、header、data、errMsg；
   - 禁止 JS 传入 `Authorization` 覆盖 host token；
   - 默认 allowlist fail closed；
   - 自动附加 DID signature 或 cached bearer。
2. 区分请求类型：
   - business API：走 capability token；
   - login/challenge：走 DID signature；
   - public GET：可按策略匿名或签名；
   - upload/download：走文件 broker，不直接开放任意文件路径。
3. 对齐错误与重试：
   - 401 清 token 后可一次 challenge retry；
   - 网络错误返回 `request:fail ...`；
   - 非 2xx 仍走 success callback 还是 fail callback需按微信语义固定；
   - 所有错误输出脱敏。

涉及模块：`wx-compat`、`anp-adapter`、`js-runtime-quickjs`、`dock-core`。

验收：

- 非 allowlist 域名无网络出站；
- Skill 不能读取或覆盖 Authorization；
- 请求行为通过 fake transport 和 integration tests 覆盖。

#### 1.5 storage、文件、媒体与设备核心 API

实施方案：

1. 注入 storage API：
   - `wx.getStorage` / `setStorage` / `removeStorage` / `clearStorage`；
   - 同步版本 `getStorageSync` / `setStorageSync`；
   - batch API 可 P1.5；
   - scope 固定为 DID + merchant + Skill。
2. 文件/媒体 API 优先实现 host-boundary 形态：
   - `format:image/file` 入参只接收 host 提供的 opaque handle；
   - `wx.chooseMedia`、`wx.chooseMessageFile` 返回 host file handle，不暴露任意本地路径；
   - `wx.previewMedia` 在 Host 能力不足时返回 fallback。
3. 设备和系统 API：
   - `wx.getDeviceInfo` / `wx.getAppBaseInfo` 保持最小真实信息；
   - `wx.getNetworkType`、`wx.onNetworkStatusChange` 可先以 snapshot/no-op listener 支持；
   - 复杂传感器、WiFi、蓝牙、TCP/UDP 默认 unsupported-by-design。

验收：

- storage 隔离测试覆盖不同 DID、merchant、Skill。
- 文件/media API 不泄漏真实路径和隐私内容。

#### 1.6 隐私、地址、手机号、支付与高风险 API

实施方案：

1. 将高风险 API 统一接入 `ConsentGate`：
   - `wx.getPhoneNumber`；
   - `wx.chooseAddress`；
   - `wx.requestPayment` / `wx.requestVirtualPayment` / `wx.requestJointPayment` 的 ANP Payment Intent 映射；
   - `wx.openLocation` / `wx.chooseLocation`；
   - `wx.makePhoneCall` / `wx.scanCode`。
2. 默认实现分层：
   - production host：调用真实 host UI/系统能力；
   - headless/CLI：显式 mock provider，输出 mock 标识；
   - 未配置 provider：fail closed。
3. payment 不复刻微信支付收银台：
   - 以 Payment Intent + user consent + merchant API 为主线；
   - demo 可 mock pay，但必须保留风险等级、proof 和 audit。

验收：

- 未配置 consent/provider 时无法执行 L3/L4 API。
- 审计记录只包含脱敏摘要、proof id 和 digest。

#### 1.7 明确 unsupported API 策略

实施方案：

1. 对 `wx.cloud.*`、微信社交、广告、公众号/视频号/客服、WiFi、蓝牙、TCP、UDP、mDNS、传感器、人脸核身、完整地图交互等 API 建立 deterministic unsupported stub。
2. stub 返回：
   - `errMsg: "<api>:fail unsupported"`；
   - `reason`；
   - `suggestion`；
   - 不访问任何宿主资源。
3. 兼容矩阵中标注 unsupported-by-design 或 P2+。

验收：

- 任何未实现 API 都不会变成 `undefined is not a function` 这类不稳定错误。
- 业务开发者能从错误中知道如何 fallback。

## 5. Phase 2：组件运行时对齐

### 5.1 阶段整体计划

Phase 2 的目标是让组件运行时从 coffee P0 卡片子集扩展到小程序 MCP 原子组件的稳定核心子集。仍然不做完整 UI 展现，重点是 **Component VM + WXML/WXSS 子集 + Render IR contract** 足够稳定，Host 可以用 Flutter、SwiftUI、Web 或 native card adapter 渲染。

主要工作包：

- 组件 manifest 元数据对齐；
- Component JS 语义增强；
- WXML/WXSS 与内置组件支持扩展；
- 动态组件能力受控开放；
- Render IR 版本化和 snapshot tests；
- 多 fixture 兼容性套件。

阶段完成标志：

- `drink-list`、`order-confirm`、`payment-result` 之外，至少再加入 3 类真实交易/表单 Skill fixture；
- Render IR 有稳定 schema version；
- 动态组件 request/timer 只在声明权限后可用；
- Host renderer 不支持时可稳定 fallback 到 CardSpec。

### 5.2 细分小阶段与实施方案

#### 2.1 组件声明与生命周期元数据

实施方案：

1. 完整读取 `components[]` 字段：
   - `path`；
   - `relatedPage`；
   - `permissions.scope.dynamic`；
   - `expirable`；
   - `expiredText`；
   - `_meta` 扩展。
2. 读取 `index.json` 基础配置，至少保留 unknown fields。
3. 组件路径 canonicalize，保证 `api._meta.ui.componentPath` 与 `components[].path` 一致。
4. 组件实例增加元数据：component id、render id、created at、expiry state、related page state。

验收：

- 组件 manifest 信息能进入 RenderOutcome 或 card event。
- `expirable: false` 的组件不会被误过期。

#### 2.2 Component JS 语义增强

实施方案：

1. 增强 `properties` 类型处理：String、Number、Boolean、Object、Array、optional/default。
2. 增加 `this.triggerEvent()` P1 支持，将事件转换为 Render IR / Host event，不直接执行宿主动作。
3. 增加 `observers` / simple watchers P2 规划，P1 可先 warning。
4. 补齐 `NotificationType.Overflow` 和 view dimension 事件。
5. 统一 lifecycle trace：created、attached、result/input notification、event、setData、expire、detached。

验收：

- Component state update 和 Render IR refresh 有 snapshot。
- 不支持的 Component 选项有 warning，不静默忽略高风险行为。

#### 2.3 WXML 子集增强

实施方案：

1. 保持 P0 表达式简单，P1 增加常见表达式：
   - `wx:elif` / `wx:else`；
   - boolean not / equality；
   - string/number literal；
   - 简单三元表达式可 P2。
2. 增加事件和 dataset 语义：
   - `catchtap`；
   - `data-*` camelCase / original key 双表示；
   - disabled button 不触发 tap。
3. 增加内置组件：
   - 表单：`input`、`textarea`、`radio`、`checkbox`、`picker`；
   - 展示：`map` preview、`canvas` static；
   - 仍不支持 `video`、`web-view`、`navigator`、广告和社交 open-type。

验收：

- 每个新增 WXML 语法都有 parser test、compiler test 和 fixture snapshot。
- 表单组件只产生 host action / component state，不绕过 consent。

#### 2.4 WXSS 子集增强

实施方案：

1. P1 增加选择器：id、标签、简单后代选择器。
2. P1 增加属性：min/max width/height、box-shadow、gap、justify-content、align-items、overflow-x。
3. 维持禁止或降级：animation、transition、复杂 transform、filter、mask、自定义字体。
4. rpx 与 host logical pixels 规则文档化，避免不同 host 渲染不一致。

验收：

- unsupported WXSS 输出 warning，不影响安全渲染。
- Render IR style 字段保持跨 Host 中立。

#### 2.5 动态组件能力

实施方案：

1. 只有声明 `components[].permissions.scope.dynamic` 的组件可使用：
   - 受限 `wx.request`；
   - `setTimeout` / `setInterval` / clear；
   - 可选 polling helper。
2. 动态组件必须满足：
   - request allowlist；
   - timer 最大数量与频率限制；
   - expire/detach 后清理；
   - host background 后暂停；
   - 审计动态请求摘要。
3. 默认组件仍禁用网络、timer、WebSocket。

验收：

- 未声明 dynamic 的组件调用 request/timer 必须失败。
- expire 后 timer 不再触发。

#### 2.6 Render IR 版本化与 Host adapter contract

实施方案：

1. 为 Render IR 增加 schema version、node kind registry、action registry。
2. 规定 Host adapter 必须处理：
   - unknown node kind fallback；
   - unknown style warning；
   - action confirmation boundary；
   - accessibility fields。
3. 增加 golden snapshot tests：相同 input 生成稳定 Render IR。
4. 增加 CardSpec fallback contract：组件加载失败、WXML 解析失败、host 不支持 node、API error。

验收：

- Render IR 变更必须更新 schema version 或 migration notes。
- Host adapter 可以独立于 Component VM 开发和测试。

## 6. Phase 3：安全增强与可信执行

### 6.1 阶段整体计划

Phase 3 的目标是把安全能力从“Demo 中证明可行”升级为“线上默认安全”。后续新增任何 API 或组件能力都必须先通过本阶段定义的安全边界。

主要工作包：

- threat model 与安全基线；
- QuickJS 沙箱和资源限制加固；
- 权限策略引擎；
- DID / token 生命周期生产化；
- consent / audit 真实化；
- Skill 包供应链安全。

阶段完成标志：

- 高风险动作无法绕过 consent；
- 非 allowlist 网络、跨包 require、远程代码、私钥/令牌泄露都有自动化测试；
- audit 可落盘、可检索、可脱敏导出；
- Skill 包加载前可校验来源和完整性。

### 6.2 细分小阶段与实施方案

#### 3.1 Threat Model 与安全分级

实施方案：

1. 建立攻击面清单：Skill package、JS runtime、component runtime、request broker、storage、DID key、token cache、Host adapter、demo/server、logs。
2. 定义攻击者模型：恶意 Skill、被篡改 Skill 包、恶意商家 Agent、恶意 Host plugin、网络中间人、日志读取者。
3. 为每条威胁定义控制措施、测试和残余风险。
4. 将风险等级 L0-L4 与 API、manifest、consent、audit 绑定。

验收：

- 每个高风险 capability 都能在 threat model 中找到对应控制措施。

#### 3.2 QuickJS 沙箱加固

实施方案：

1. 审计 `eval` / `Function` / prototype constructor / async function constructor / generator constructor escape。
2. 增加资源限制：
   - memory hard limit；
   - stack limit；
   - CPU/interrupt timeout；
   - Promise job drain 上限；
   - console/log size 上限；
   - result size 上限。
3. 将 API VM 和 Component VM 的 sandbox policy 文档化并测试。
4. 禁止远程代码、任意文件系统、socket、WebSocket，除非 capability broker 明确开放。

验收：

- sandbox escape tests 作为 CI gate。
- 超限行为返回稳定错误并记录 audit。

#### 3.3 权限策略与 allowlist

实施方案：

1. 从 `mcp.json`、`_meta.anp`、`x_anp`、`components[].permissions` 推导权限。
2. 支持宿主级 policy override：allow、deny、mock、prompt。
3. 网络 allowlist 支持：scheme、host、port、path prefix、method、scope。
4. storage、file、media、location、phone、address、payment 全部通过 capability broker。
5. 未声明权限但调用敏感能力时 fail closed。

验收：

- 任何 capability 都有 permission decision 记录。
- CLI/headless mock 必须显式开启，不能默默通过。

#### 3.4 DID、token 与会话安全

实施方案：

1. token claims 固定：issuer、audience、merchantDid、userDid、agentDid、skillId、sessionId、scopes、iat/nbf/exp、jti、version。
2. 支持 token refresh、revoke、logout、cache eviction。
3. challenge 防 replay：nonce 一次性、TTL、audience、method/url、DID document binding。
4. DID document resolver 生产化：cache、TTL、trust anchors、network failure policy。
5. 私钥只在 host/credential provider 边界使用，永不进入 JS 或日志。

验收：

- replay、wrong audience、wrong scope、expired token、wrong DID document 全部有测试。
- token 轮换不破坏 running Skill session。

#### 3.5 Consent 与审计生产化

实施方案：

1. 把 mock consent provider 抽象为 host consent adapter：CLI、Mac、Flutter、server-side headless policy 可分别实现。
2. ConsentProof 增加不可抵赖所需字段：policy version、UI prompt digest、decision actor、timestamp、parameter digest。
3. Audit sink 支持持久化：SQLite / file append / remote audit service，至少一个生产候选实现。
4. 审计查询必须默认脱敏，原始敏感字段不落盘或加密保存。
5. 建立 redaction regression tests：Authorization、Signature、token、secret、private key、phone、address、file content。

验收：

- 支付/下单/地址/手机号 API 没有 consent proof 不会执行。
- `GET /audit` 或导出接口永不泄露 token/signature。

#### 3.6 Skill 包完整性与供应链

实施方案：

1. 支持 Skill package digest、签名、版本、publisher DID。
2. 下载/缓存 Skill 时校验 digest 和 path boundary。
3. 禁止 symlink escape、absolute path、remote require。
4. 记录 package source、version、digest 到 audit。
5. 为第三方 Skill 建立 quarantine / review / allowlist 流程。

验收：

- 篡改包、路径穿越、签名不匹配、未知 publisher 默认无法加载。

## 7. Phase 4：生产运行时与 Host 接入

### 7.1 阶段整体计划

Phase 4 的目标是把 CLI/demo-server 形态升级为可被真实宿主集成和线上部署的容器。这里仍不要求做完整 UI，只要求稳定的 runtime API、进程边界、持久化和 Host adapter contract。

主要工作包：

- Runtime 公共 API / SDK；
- Skill 发现、下载、缓存、版本管理；
- session/token/storage/audit 持久化；
- 本地进程或嵌入式 SDK 形态；
- Host renderer / action protocol；
- 并发、取消、重试和幂等。

阶段完成标志：

- Runtime public API、headless IPC/SDK envelope 和 Host adapter/action contract 均已版本化并可通过测试查询；
- CLI/demo 已收敛到同一 Runtime facade，headless adapter 可证明 Render IR、action、fallback、redaction 和 fail-closed contract；
- token/storage/audit/cache 的持久化、恢复、清理和 release blocker 边界可审计，不把 dev-only/local 未加密 backend 写成 production-ready；
- 多用户、多商家、多 Skill session 隔离、高风险串行、取消、timeout、幂等和 no-retry 策略通过测试；
- 真实 production Host UI/provider、HTTP/gRPC sidecar、远端 registry download、生产签名 verifier、加密生产后端、分布式 lock 和 CI/ops 自动化仍作为后续 release blocker 记录。
- Step 04-11 完成 Phase 4 最终 Review 与整体验证后，才能作为 Phase 5 启动 gate。

### 7.2 细分小阶段与实施方案

#### 4.1 Runtime API 稳定化

实施方案：

1. 定义 public Rust API：load skill、validate、call api、render component、dispatch action、expire cards、query audit。
2. 定义可选 IPC API：HTTP/gRPC/JSON-RPC，以便非 Rust Host 接入。
3. 保持 API 输入输出模型稳定并版本化。
4. 将 CLI 改为调用同一 Runtime API，避免 CLI 逻辑成为第二套 runtime。

验收：

- CLI、Mac host、未来 Flutter host 共用一套 runtime contract。

#### 4.2 Skill 发现、获取与版本管理

实施方案：

1. 对接 ANP Agent registry / merchant manifest：发现 merchant DID、Skill manifest URL、package digest、auth endpoints。
2. 支持本地 cache：按 publisher DID + skill id + version + digest 存储。
3. 支持版本选择策略：latest、pinned、allow prerelease、rollback。
4. package.zip 从 no-op 变成真实服务路径，但加载仍先解包到安全隔离目录。

验收：

- 相同 digest 可复用缓存；digest mismatch 会拒绝加载。
- 可回滚到上一个已验证 Skill version。

#### 4.3 持久化与配置

实施方案：

本节不对应单个大 Step，必须按 Step 04-04 至 04-08 拆开执行，避免一个 commit 同时跨配置、secret、token、storage、audit 和 cache：

1. Step 04-04 只冻结 runtime config schema、profile、secret reference、provider handle 和 redaction，不实现具体持久化 backend。
2. Step 04-05 单独实现 token cache 持久化与恢复，生产路径必须使用 secure store 或加密 backend。
3. Step 04-06 单独实现 scoped storage 持久化与 quota，按 DID/merchant/Skill scope 隔离。
4. Step 04-07 单独实现 persistent audit sink、retention 和 redacted export。
5. Step 04-08 单独实现 Skill cache cleanup、quarantine、eviction 和 rollback pin 保护。

验收：

- 每个子项都有独立 Step 文档、Review 记录、验证证据和 focused commit。
- 重启后 token/storage/audit 的恢复行为只在对应 Step 完成后声明为支持。
- 删除用户/Skill 数据能按 scope 清理，且不会泄露 secret、token、隐私原文或本机绝对路径。

#### 4.4 Host renderer 与 action protocol

实施方案：

1. 定义 Host 需要实现的最小协议：
   - render Render IR；
   - render CardSpec fallback；
   - request consent；
   - handle phone/address/media/file/payment/location providers；
   - dispatch user events back to container。
2. action 必须回到 `dock-core`，不允许组件直接调用高风险 host 操作。
3. 定义 headless 模式：只输出 JSON，不做 UI；用于 CI 和 server-side agent。
4. Mac/Flutter/Web adapter 可作为参考实现，但不是容器核心。

验收：

- Render IR snapshot 可被至少一个 adapter 渲染或安全 fallback。
- 用户点击后 action flow 保持 audit/consent 边界。

#### 4.5 并发、取消、重试与幂等

实施方案：

1. 每个 session 支持多个并发 API 调用，但同一高风险交易可按 policy 串行化。
2. API call 支持 cancellation token 和 timeout。
3. request broker 支持 retry policy，但支付/下单等非幂等 API 默认不自动重试。
4. order/payment API 建议引入 idempotency key。
5. 组件过期和 session 结束会取消动态请求/timer。

验收：

- 并发场景不会串 session/token/storage。
- 取消后不会继续执行高风险 action。

## 8. Phase 5：开发者体验与生态兼容

### 8.1 阶段整体计划

Phase 5 的目标是让外部 Skill 开发者能理解容器能力、导入 Skill、自助调试并获得兼容性报告。

主要工作包：

- CLI / SDK 工具链；
- 兼容性报告；
- 示例 Skill 和迁移指南；
- 本地调试器；
- 文档与模板。

阶段完成标志：

- 开发者可用一个命令验证 Skill 是否可在容器内上线；
- 兼容性报告能指出哪些 API/组件会降级或失败；
- 至少有 coffee 之外的多个示例覆盖不同能力。

### 8.2 细分小阶段与实施方案

#### 5.1 CLI 命令扩展

实施方案：

1. `dock-cli validate` 输出 compatibility level、warnings、unsupported API、component fallback risk。
2. `dock-cli inspect` 展示 Skill package、权限、风险 API、组件树、依赖。
3. `dock-cli test-skill` 执行 fixture inputs、snapshot Render IR、审计输出。
4. `dock-cli import-wechat-mcp` 将小程序 MCP Skill 目录转换/复制为容器可验证结构，但不强制修改原字段。
5. `dock-cli doctor` 检查 DID identity、host providers、allowlist、storage、sandbox。

验收：

- CLI 输出全 JSON，可用于 CI。
- 所有敏感值默认 redacted。

#### 5.2 示例 Skill 与兼容测试集

实施方案：

1. 保留 coffee Skill 作为交易流程基线。
2. 新增至少三类 fixture：
   - 表单/地址/手机号授权；
   - 图片/文件输入处理；
   - 动态组件/状态刷新；
   - 可选地图/位置预览。
3. 每个 fixture 包含 `SKILL.md`、`mcp.json`、API JS、组件、expected result、Render IR snapshot。

验收：

- 每类核心能力都有可复制的示例。
- 新增兼容能力必须先新增或更新 fixture。

#### 5.3 文档与迁移指南

实施方案：

1. 编写“从小程序 MCP Skill 迁移到 ANP MiniApp Dock”的指南。
2. 编写 API 对齐表和组件对齐表，标明 ANP 替代实现。
3. 编写安全开发指南：不要存 token、不要在 content 暴露隐私、如何声明权限、如何处理 fallback。
4. 编写 Host adapter 开发指南。

验收：

- 开发者无需阅读 Rust 源码也能完成 Skill 验证。
- 文档和 CLI 报告术语一致。

## 9. Phase 6：观测、性能与发布运营

### 9.1 阶段整体计划

Phase 6 的目标是让容器具备线上运行所需的可观测性、性能边界、发布门禁和故障处理流程。

主要工作包：

- 结构化日志、metrics、traces；
- 性能和资源基准；
- CI/CD gates；
- release/canary/rollback；
- 线上 runbook。

阶段完成标志：

- 线上问题能通过 session id、skill id、merchant DID、api name 定位；
- 任意 release 都可回滚；
- 性能退化和敏感信息泄露能在 CI 阶段发现。

### 9.2 细分小阶段与实施方案

#### 6.1 可观测性

实施方案：

1. 统一结构化事件：skill_load、api_call_start/end、request_start/end、consent_prompt/decision、render_start/end、component_event、audit_record。
2. metrics：API latency、VM time、render time、request status、fallback rate、consent required/approved/denied、sandbox timeout、memory limit hit。
3. traces：同一用户请求贯穿 model decision、API call、request、render、action。
4. 所有 logs 默认 redacted。

验收：

- 可以在不看敏感 payload 的情况下定位失败阶段。

#### 6.2 性能与容量

实施方案：

1. 建立基准：Skill load time、API call latency、component render latency、memory per VM、token cache lookup。
2. 加入 stress tests：并发 session、多 Skill、多组件渲染、动态组件 timer。
3. 定义资源限制默认值和可配置范围。
4. 对生产 host 建议 warm cache，但保持每次调用安全上下文隔离。

验收：

- 性能基准写入 release notes。
- 超过资源限制时 fail closed，不影响其他 session。

#### 6.3 CI/CD 与 release gates

实施方案：

1. CI 必跑：fmt、clippy、unit、integration、sandbox escape、compat fixture、redaction、snapshot。
2. release 前必跑：完整 demo、多个 fixture、security checklist、docs link check。
3. 版本策略：runtime API、Render IR、capability token、Skill package contract 分别版本化。
4. 建立 canary：先在 headless/CLI 和内部 Host 跑，再开放外部 Skill。

验收：

- 任一 gate 失败不得发布。
- breaking change 必须有 migration note。

#### 6.4 运维 runbook

实施方案：

1. 编写部署、配置、identity、secret、storage、audit、升级、回滚文档。
2. 编写常见故障处理：DID 验签失败、token scope mismatch、allowlist deny、component render failed、consent required、sandbox timeout。
3. 定义数据清理与用户隐私删除流程。

验收：

- 运维人员能独立判断是 Skill 问题、商家 Agent 问题、Host provider 问题还是容器问题。

## 10. 能力优先级建议

后续实际开发应按以下优先级推进。

### 10.1 必须优先补齐

1. `wx.login` / `wx.checkSession` / `wx.request` / storage 的正式 JS bridge 与统一 capability broker。
2. `wx.modelContext.getSessionId`、`expireAllCards`、card event 的 runtime 化。
3. `components[].relatedPage`、`expirable`、`expiredText`、`permissions.scope.dynamic` 的完整读取与行为。
4. 高风险 API 的 consent/audit 强制路径。
5. API/组件兼容矩阵与 CLI 兼容报告。
6. sandbox escape、allowlist、redaction、token replay/scope 的 CI gate。

### 10.2 第二优先级

1. `format:image/file`、`chooseMedia`、`previewMedia`、`uploadFile` / `downloadFile` / `openDocument` 的 host-boundary 形态。
2. `chooseAddress`、`getPhoneNumber`、`requestPayment` 的 production provider 接口。
3. 表单组件、`openDetailPage` fallback、`Overflow`、动态组件 request/timer。
4. Skill package digest/signature/cache。
5. persistent storage/token/audit。

### 10.3 可后置或明确不做

1. 完整微信页面路由、TabBar、多页面生命周期。
2. 完整半屏小程序页面能力。
3. 微信云开发、微信支付收银台、微信社交生态 API。
4. 蓝牙、WiFi、TCP、UDP、mDNS、复杂传感器、完整地图交互。
5. 完整 WXML/WXSS 和完整自定义组件系统。

## 11. 阶段验收总表

| 能力方向 | Demo 原型可接受 | 线上产品必须达到 |
|---|---|---|
| Skill 加载 | 单 coffee Skill，本地目录 | 多 Skill、版本、digest、签名、缓存、兼容报告 |
| 原子接口 | P0 createSkill/registerAPI/use | 核心 `wx.modelContext` 与 `wx.*` JS bridge 对齐，unsupported API 稳定失败 |
| 网络 | localhost demo bridge | allowlist + DID signature + scoped bearer + retry/refresh + redaction |
| 身份 | 示例 DID 文件 | host credential provider、DID resolver、token lifecycle、secret store |
| 组件 | coffee 三卡片 | 组件矩阵 P1、Render IR version、动态组件受控、snapshot fixtures |
| 安全 | 单元测试覆盖关键点 | threat model、sandbox gates、package signing、audit persistence、CI fail closed |
| Host | CLI/Mac demo | 稳定 Runtime API / IPC、Host adapter contract、headless mode |
| 运维 | 本地 runbook | metrics/logs/traces、release gates、rollback、privacy deletion |

## 12. 立即下一步建议

如果下一轮开始进入代码开发，建议按以下顺序开工：

1. 先完成 Phase 0 的三份基础文档：`wx-api-compatibility-matrix.md`、`component-compatibility-matrix.md`、`threat-model.md`。
2. 从 Phase 1.1 与 Phase 1.2 开始：增强 validator 与 `wx.modelContext` JS bridge，因为它们影响面清晰、风险较低、会为后续 API 对齐提供骨架。
3. 同步把当前 demo-only `wx.login` / `wx.request` 代码收敛为正式 `DidAuthSessionManager` 和 `RequestBroker` 注入路径。
4. 每补一个 API 或组件能力，必须同时补：兼容矩阵、unit test、fixture 或 snapshot、runbook/CLI 输出。
5. 在 Phase 1/2 期间就启动 Phase 3 threat model，避免后续因为安全边界不清晰返工。
