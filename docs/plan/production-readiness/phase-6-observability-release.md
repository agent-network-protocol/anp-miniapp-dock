# Phase 6：观测、性能与发布运营实施计划

## 1. 阶段目标

Phase 6 让容器具备线上运行的可观测性、性能边界、发布门禁、灰度和回滚能力。完成后，线上问题能定位到具体 session、Skill、API、Host provider 或 merchant Agent，而不需要查看敏感 payload。

## 2. 观测模型

### 2.1 结构化事件

必须记录但默认脱敏：

- `skill_load_start/end`
- `api_call_start/end`
- `wx_api_call_start/end`
- `request_start/end`
- `consent_prompt/decision`
- `component_render_start/end`
- `component_event`
- `fallback_used`
- `audit_record_written`
- `sandbox_limit_hit`

公共字段：

```text
traceId
sessionId
skillId
apiName?
componentPath?
merchantDid?
userDid? (可 hash)
runtimeVersion
renderIrVersion
outcome
latencyMs
```

### 2.2 Metrics

| 指标 | 目的 |
|---|---|
| API latency | 识别慢接口 |
| VM execution time | sandbox 性能 |
| render latency | 组件渲染性能 |
| request status | merchant/网络错误 |
| fallback rate | 兼容性质量 |
| consent approve/deny rate | 风控与 UX |
| unsupported API count | 迁移阻塞点 |
| sandbox timeout/memory hit | 恶意或低质量 Skill |
| token refresh/fail count | DID/auth 健康度 |

### 2.3 Tracing

一条用户请求应串起：

```text
Host message
  -> model/intent decision
  -> Skill/API call
  -> wx.login/checkSession/request
  -> merchant response
  -> component render
  -> user action
  -> follow-up api/call
  -> audit
```

## 3. 性能基线

建议基准：

| 基准 | 初始目标 |
|---|---|
| Skill load | 本地 P50/P95 |
| API VM cold call | P50/P95 |
| API VM warm-ish call | P50/P95 |
| Component render | P50/P95 |
| Render IR size | P50/P95 |
| token lookup | P50/P95 |
| storage read/write | P50/P95 |
| memory per VM | max |

具体数值应在实现基准测试后写入 release notes，不在计划文档中凭空承诺。

## 4. CI/CD Gates

基础 gates：

```bash
cargo metadata --format-version 1 --no-deps
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

产品 gates：

- compatibility matrix coverage；
- sandbox escape regression；
- redaction regression；
- DID/token replay/scope tests；
- Render IR snapshots；
- fixture E2E；
- markdown link check；
- release notes completeness。

## 5. 发布策略

### 5.1 版本化对象

- Runtime API version；
- Render IR schema version；
- capability token version；
- Skill package contract version；
- Host adapter contract version。

### 5.2 灰度流程

1. headless fixture 全量通过；
2. internal Host canary；
3. allowlisted merchant Skill；
4. expand by publisher DID / skill version；
5. monitor fallback/error/consent/token metrics；
6. rollback on gate breach。

### 5.3 回滚条件

- token leakage regression；
- consent bypass；
- sandbox escape；
- fallback rate 超阈值；
- auth failure rate 激增；
- Host crash / Render IR incompatible；
- audit write failure。

## 6. 运维 Runbook

需要覆盖：

- DID 验签失败；
- token scope mismatch；
- allowlist deny；
- component render failed；
- sandbox timeout；
- storage quota exceeded；
- audit sink unavailable；
- Host provider unavailable；
- merchant Agent unavailable；
- Skill package signature mismatch；
- rollback and cache purge。

## 7. 阶段完成检查

- [ ] 结构化日志和 metrics 默认脱敏。
- [ ] trace 能串起一次完整 coffee/order flow。
- [ ] 性能基准有自动化脚本。
- [ ] CI gates 覆盖安全、兼容、snapshot、文档。
- [ ] canary/rollback runbook 可执行。
- [ ] release notes 包含版本、兼容变化、风险和回滚方式。
