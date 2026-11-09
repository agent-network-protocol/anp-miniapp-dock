# 产品化里程碑详细计划文档索引

本目录是 [`production-readiness-roadmap.md`](../production-readiness-roadmap.md) 的展开版。Roadmap 负责说明从 Demo 原型到线上容器的总体阶段，本目录中的文档负责指导每个阶段如何落地开发、如何拆任务、改哪些模块、如何验收。

## 文档地图

| 阶段 | 详细计划 | 适用范围 | 深入子文档 |
|---|---|---|---|
| Phase 0 | [基线冻结与产品化门槛](phase-0-baseline-and-gates.md) | 当前能力盘点、兼容矩阵、release gates、backlog | - |
| Phase 1 | [接口对齐与 wx Capability Broker](phase-1-wx-capability-broker.md) | 原子接口环境、`wx.modelContext`、`wx.*`、DID login、request、storage、支付/隐私 API | [wx API Bridge Contract](phase-1-wx-api-bridge-contract.md)、[DID Request Session Manager](phase-1-did-request-session-manager.md) |
| Phase 2 | [组件运行时对齐](phase-2-component-runtime-alignment.md) | Component VM、WXML/WXSS、动态组件、组件交互、Render IR | [Render IR 与 Fixture 体系](phase-2-render-ir-and-fixtures.md) |
| Phase 3 | [安全增强与可信执行](phase-3-security-hardening.md) | sandbox、权限、token、audit、Skill 包供应链 | [Threat Model 与安全控制](phase-3-threat-model-and-controls.md) |
| Phase 4 | [生产运行时与 Host 接入](phase-4-runtime-host-integration.md) | Runtime API、IPC、Skill registry/cache、持久化、Host adapter | - |
| Phase 5 | [开发者体验与生态兼容](phase-5-developer-experience.md) | CLI/SDK、兼容报告、示例 Skill、迁移指南 | - |
| Phase 6 | [观测、性能与发布运营](phase-6-observability-release.md) | metrics/logs/traces、性能基线、CI/CD、runbook | - |

## 使用方式

1. 先读总览 roadmap，确认当前要进入哪个 Phase，并查看其中的 `Resume From Here`、执行台账、Codex Goal 执行协议和 Review 门禁。
2. 进入对应 Phase 文档，按“开发顺序”拆 issue；已经拆出的 Step 必须优先以 [`steps/`](steps/) 下的小 Plan 为执行入口。
3. 如果 Phase 文档引用深入子文档，先冻结子文档中的契约，再写代码。
4. 每个 issue 或 Step 必须同时更新：实现、测试、兼容矩阵、runbook 或开发者文档。
5. 每个 Step 完成前必须按小 Plan 的“Review 环节”做 commit 前 Review；每个 Phase 完成前必须按对应文档的“阶段完成检查”做一次阶段审计。

## Codex Goal Step 文档

以下 Step 是当前已经拆出的可执行小 Plan。状态、依赖、commit 和证据以 [`../production-readiness-roadmap.md`](../production-readiness-roadmap.md) 的执行台账为准；当前恢复指针从 Step 01-05 开始。

| Step | 小 Plan | 范围 |
|---|---|---|
| 00-01 | [当前能力盘点与基线固化](steps/00-01-baseline-inventory.md) | Phase 0 当前能力、证据、demo-only 标注 |
| 00-02 | [wx API 兼容矩阵](steps/00-02-wx-api-compatibility-matrix.md) | Phase 0 API 状态、映射、风险、测试证据 |
| 00-03 | [组件兼容矩阵](steps/00-03-component-compatibility-matrix.md) | Phase 0 组件、WXML/WXSS、事件、Render IR 能力 |
| 00-04 | [Threat model 与 release gates 初版](steps/00-04-threat-model-release-gates.md) | Phase 0 安全基线、发布门槛、回滚规则 |
| 01-01 | [wx API Bridge Contract 冻结](steps/01-01-wx-api-bridge-contract-freeze.md) | Phase 1 bridge 契约、错误语义、callback/Promise 决策 |
| 01-02 | [Skill package 与 manifest 对齐](steps/01-02-skill-package-manifest-alignment.md) | Phase 1 manifest 校验、兼容报告 |
| 01-03 | [`wx.modelContext` 原子接口桥接](steps/01-03-model-context-bridge.md) | Phase 1 modelContext JS bridge 与 card event |
| 01-04 | [DID 会话与 RequestBroker 收敛](steps/01-04-did-session-request-broker.md) | Phase 1 `DidAuthSessionManager`、登录、会话、请求 |
| 01-05 | [Unsupported API Registry 与统一 fail shape](steps/01-05-unsupported-api-registry-fail-shape.md) | Phase 1 unsupported registry、统一 fail shape |
| 01-06 | [Storage JS Bridge](steps/01-06-storage-js-bridge.md) | Phase 1 storage async/sync JS bridge |
| 01-07 | [Device/App Info Atomic API](steps/01-07-device-app-info-atomic-api.md) | Phase 1 `wx.getDeviceInfo`、`wx.getAppBaseInfo` 原子接口 |
| 01-08 | [高风险 API Host Boundary 与 fail-closed](steps/01-08-high-risk-api-host-boundary.md) | Phase 1 phone/address/location/media/payment/scan/phone call Host boundary |
| 02-01 | [Render IR schemaVersion 与 fallback reason enum](steps/02-01-render-ir-schema-fallback-reasons.md) | Phase 2 Render IR contract 和 fallback reason |
| 02-02 | [Component manifest metadata runtime flow](steps/02-02-component-manifest-metadata-runtime-flow.md) | Phase 2 component metadata runtime 流向 |
| 02-03 | [WXML/WXSS P1 语法增强](steps/02-03-wxml-wxss-p1-syntax.md) | Phase 2 WXML/WXSS P1 parser/compiler/style |
| 02-04 | [表单与静态媒体节点](steps/02-04-form-static-media-nodes.md) | Phase 2 form、map-preview、canvas-static Render IR nodes |
| 02-05 | [Dynamic component controls](steps/02-05-dynamic-component-controls.md) | Phase 2 dynamic request、timer、resource cleanup |
| 02-06 | [Fixture 与 Render IR snapshots](steps/02-06-fixtures-render-ir-snapshots.md) | Phase 2 fixtures 和 Render IR golden snapshots |
| 02-07 | [01-05 至 02-06 批次最终 Review 与整体验证](steps/02-07-batch-final-review-verification.md) | Phase 1/2 批次 final Review gate |
| 03-01 | [Threat Model 与安全分级收敛](steps/03-01-threat-model-security-classification.md) | Phase 3 安全控制矩阵、L0-L4 风险分级 |
| 03-02 | [QuickJS 沙箱逃逸回归与资源限制](steps/03-02-quickjs-sandbox-resource-limits.md) | Phase 3 API VM / Component VM sandbox gates |
| 03-03 | [权限策略引擎与 allowlist decision](steps/03-03-permission-policy-engine-allowlist.md) | Phase 3 permission policy、Host override、network allowlist |
| 03-04 | [DID / Token 生命周期与 Resolver 信任锚](steps/03-04-did-token-lifecycle-resolver.md) | Phase 3 token lifecycle、replay 防护、resolver trust |
| 03-05 | [Consent Adapter 与持久化 Audit Sink](steps/03-05-consent-adapter-persistent-audit.md) | Phase 3 Host consent adapter、persistent audit |
| 03-06 | [Skill 包完整性与供应链 Gate](steps/03-06-skill-package-integrity-supply-chain.md) | Phase 3 digest、signature、publisher DID、quarantine |
| 04-01 | [Runtime API Facade 与版本化](steps/04-01-runtime-api-facade-versioning.md) | Phase 4 public Runtime API、version、stable error |
| 04-02 | [IPC / SDK 形态与 Host 进程边界](steps/04-02-ipc-sdk-host-process-boundary.md) | Phase 4 IPC/headless JSON/Rust SDK boundary |
| 04-03 | [Skill Registry / Cache 与版本回滚](steps/04-03-skill-registry-cache-versioning.md) | Phase 4 registry ref、digest cache、version rollback |
| 04-04 | [Runtime Config 与 Secret Store 边界](steps/04-04-runtime-config-secret-store.md) | Phase 4 config schema、secret reference、redaction |
| 04-05 | [Token Cache 持久化与恢复](steps/04-05-token-cache-persistence.md) | Phase 4 token cache secure backend、restore policy |
| 04-06 | [Scoped Storage 持久化与 quota](steps/04-06-scoped-storage-persistence.md) | Phase 4 storage backend、scope isolation、quota |
| 04-07 | [Persistent Audit Sink retention/export](steps/04-07-persistent-audit-retention-export.md) | Phase 4 audit persistence、retention、redacted export |
| 04-08 | [Skill Cache cleanup 与版本清理](steps/04-08-skill-cache-cleanup.md) | Phase 4 digest cache cleanup、quarantine、eviction |
| 04-09 | [Host Adapter Contract 与 Action Protocol](steps/04-09-host-adapter-contract-action-protocol.md) | Phase 4 Host renderer/provider/action conformance |
| 04-10 | [并发、取消、重试与幂等策略](steps/04-10-concurrency-cancellation-idempotency.md) | Phase 4 session manager、cancel、retry、idempotency |
| 05-01 | [CLI validate 兼容报告增强](steps/05-01-cli-validate-compatibility-report.md) | Phase 5 JSON compatibility report、release blockers |
| 05-02 | [CLI inspect Skill package](steps/05-02-cli-inspect-skill-package.md) | Phase 5 package/API/component/permission/risk inspect |
| 05-03 | [CLI test-skill 与 Fixture Runner](steps/05-03-cli-test-skill-fixture-runner.md) | Phase 5 fixture runner、snapshot compare、audit summary |
| 05-04 | [CLI import-wechat-mcp](steps/05-04-cli-import-wechat-mcp.md) | Phase 5 safe import、dry-run、ANP `_meta` patch 建议 |
| 05-05 | [CLI doctor 环境诊断](steps/05-05-cli-doctor-environment.md) | Phase 5 toolchain/DID/resolver/storage/provider diagnostics |
| 05-06 | [示例 Skill 与兼容测试集](steps/05-06-example-skills-compatibility-fixtures.md) | Phase 5 address/media/dynamic/location examples |
| 05-07 | [开发者文档与迁移指南](steps/05-07-developer-docs-migration-guides.md) | Phase 5 import/API/component/security/Host adapter docs |
| 06-01 | [结构化观测事件与脱敏日志](steps/06-01-structured-observability-events.md) | Phase 6 structured events、redacted logs |
| 06-02 | [Metrics / Tracing 与请求链路关联](steps/06-02-metrics-tracing-correlation.md) | Phase 6 metrics registry、trace propagation |
| 06-03 | [性能基线与 Stress Tests](steps/06-03-performance-baselines-stress.md) | Phase 6 benchmarks、stress tests、baseline artifact |
| 06-04 | [CI/CD Release Gates 自动化](steps/06-04-ci-cd-release-gates-automation.md) | Phase 6 gate runner、CI workflow、release report |
| 06-05 | [Canary 发布、版本化与回滚策略](steps/06-05-canary-release-rollback.md) | Phase 6 release notes、canary、rollback/cache purge |
| 06-06 | [运维 Runbook 与隐私删除流程](steps/06-06-operations-runbook-privacy-deletion.md) | Phase 6 troubleshooting、operations、privacy deletion |

## 共同 Definition of Done

每个 Phase 的开发都必须满足：

- 不破坏“智能体原生小程序 MCP 容器”的边界：不做完整微信小程序 Runtime，不把 UI 复刻作为核心目标。
- 对 Skill 暴露的接口优先兼容小程序 MCP；底层身份、网络和授权由 ANP DID / Rust Runtime 替换。
- 新增能力默认 fail closed；demo mock 必须显式标记，不能静默进入生产路径。
- 敏感信息不得进入模型可见输出、日志、CLI JSON、audit export 或 Render IR。
- 新增 API / 组件 / 安全策略必须有自动化测试或可执行 fixture。
- 文档中的状态必须与代码状态同步：`supported`、`host-boundary`、`planned`、`unsupported-by-design`、`demo-only` 不得混用。
