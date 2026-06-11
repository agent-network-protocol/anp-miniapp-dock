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

1. 先读总览 roadmap，确认当前要进入哪个 Phase。
2. 进入对应 Phase 文档，按“开发顺序”拆 issue。
3. 如果 Phase 文档引用深入子文档，先冻结子文档中的契约，再写代码。
4. 每个 issue 必须同时更新：实现、测试、兼容矩阵、runbook 或开发者文档。
5. 每个 Phase 完成前必须按对应文档的“阶段完成检查”做一次审计。

## 共同 Definition of Done

每个 Phase 的开发都必须满足：

- 不破坏“智能体原生小程序 MCP 容器”的边界：不做完整微信小程序 Runtime，不把 UI 复刻作为核心目标。
- 对 Skill 暴露的接口优先兼容小程序 MCP；底层身份、网络和授权由 ANP DID / Rust Runtime 替换。
- 新增能力默认 fail closed；demo mock 必须显式标记，不能静默进入生产路径。
- 敏感信息不得进入模型可见输出、日志、CLI JSON、audit export 或 Render IR。
- 新增 API / 组件 / 安全策略必须有自动化测试或可执行 fixture。
- 文档中的状态必须与代码状态同步：`supported`、`host-boundary`、`planned`、`unsupported-by-design`、`demo-only` 不得混用。
