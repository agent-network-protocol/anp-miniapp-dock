# Phase 4：生产运行时与 Host 接入实施计划

## 1. 阶段目标

Phase 4 将 CLI/demo 形态升级为可被真实宿主集成和线上部署的容器。核心产物是稳定 Runtime API、可选 IPC 边界、Skill registry/cache、持久化和 Host adapter contract。

## 2. 涉及模块

| 模块 | 方向 |
|---|---|
| `dock-core` | public runtime service、orchestrator facade、session/action manager |
| `dock-cli` | 改为 Runtime API 的调用者，而不是第二套流程 |
| `skill-loader` | registry/cache/package zip/digest |
| `anp-adapter` | Agent discovery、DID resolver、trusted merchant policy |
| `component-runtime` | Render IR service endpoint |
| `consent-audit` | persistent audit sink |
| Host apps | Mac/Flutter/Web/headless adapters |

## 3. 开发顺序

### 3.1 Runtime API 稳定化

定义稳定 API：

```text
validate_skill(path_or_package)
load_skill(skill_ref)
call_api(session, skill_id, api_name, arguments)
render_component(session, component_path, input)
dispatch_component_action(session, render_id, action)
expire_cards(session, filters)
get_audit_records(filters)
close_session(session)
```

要求：

- 输入输出 JSON 可序列化；
- 错误码稳定；
- API version 可协商；
- CLI 和 Host 共用同一 API。

当前 Step 04-01 已冻结 Rust facade 的初版 contract：

| API | 当前 contract | Phase 4 后续边界 |
|---|---|---|
| `validate_skill(path_or_package)` | `validate_skill_path()` / `RuntimeService::validate_skill()` 返回 `RuntimeResponse<RuntimeValidateSkillResponse>` | 不执行生产 registry download；真实 registry 来源由 04-03 接入。 |
| `load_skill(skill_ref)` | `load_skill_path()` / `RuntimeService::load_skill_response()` 返回 `RuntimeResponse<RuntimeLoadSkillResponse>` | production policy、cache pin 和 rollback 由 04-03 继续收敛。 |
| `call_api(session, skill_id, api_name, arguments)` | `RuntimeService::call_api(RuntimeCallRequest)`；返回 `RuntimeCallResponse`，内部仍走 Orchestrator、Permission、Consent、Audit、RenderRouter | `capabilityToken` 不序列化、不进入 Debug；Host/IPC 形态由 04-02 复用。 |
| `render_component(session, component_path, input)` | `RuntimeService::render_component(RuntimeRenderComponentRequest)`；返回 `RenderOutcome` | 真实 Host renderer UI 由 Host adapter 实现；04-09 冻结 conformance contract。 |
| `host_contract()` | `RuntimeService::host_contract()`；返回 `dock.host-adapter.v1` capability declaration | Host 必须声明 required/optional/unsupported-by-design capability；headless mock 明确 `productionReady = false`。 |
| `dispatch_component_action(session, render_id, action)` | `RuntimeService::dispatch_component_action(RuntimeDispatchComponentActionRequest)`；`api/call` 回 Orchestrator，非 API action 返回 Host action outcome | unknown/high-risk action protocol 和 Host adapter conformance 由 04-09 补齐。 |
| `expire_cards(session, filters)` | 返回稳定 `RuntimeExpireCardsResponse`，边界标记为 `host-managed-card-store` | 持久 card/session store 不在 04-01 实现，后续由 04-09/04-10 承接。 |
| `get_audit_records(filters)` | 通过 `RuntimeAuditReader` 返回 `RuntimeAuditEvent`，参数和 proof summary 二次脱敏 | 持久化 retention/export 由 04-07 承接。 |
| `concurrency_policy()` | `RuntimeService::concurrency_policy()` 返回 `dock.runtime.concurrency.v1`，声明低风险并发、高风险串行、取消、timeout、retry 和幂等策略 | 04-10 冻结本地 Runtime contract；不声明分布式锁或耐久幂等存储已完成。 |
| `cancel_operation(session, cancellationToken)` | `RuntimeService::cancel_operation(RuntimeCancelOperationRequest)` 把 cancellation token 写入 Runtime 本地取消注册表 | 只阻断后续带相同 token 的本地 dispatch；不承诺中断已经进入 executor/provider 的同步调用。 |
| `close_session(session)` | 返回稳定 `RuntimeCloseSessionResponse`，边界标记为 `runtime-session-manager`，并关闭本地 session 后续 dispatch | token/cache/storage/audit 的持久化清理仍由 04-05 至 04-08 和后续 ops/privacy deletion 串联。 |

版本策略：

- 当前版本常量为 `dock.runtime.v1`，所有 `RuntimeResponse<T>` / `RuntimeErrorResponse` 都包含 `version` 和 `status`。
- `negotiate_runtime_version()` 对未知版本返回稳定 `unsupported_version` error code。
- `RuntimeErrorResponse` 使用 `ErrorCode::as_str()` 或 runtime 专用 code，并对 token、Authorization、signature、private key、secret、credential 等文本做 redaction。
- `RuntimeSkillSummary.packageRef` 使用 digest ref 或 `local-dev-package`，不输出本机 skill root 绝对路径。
- `dock-cli call-api` 已改为通过 `RuntimeService::call_api()`，但保留原 CLI JSON 字段；`run-demo` 继续复用同一 `RuntimeHarness` 调用 facade。CLI JSON 仍不是 IPC/Host 生产协议，04-02 会定义 headless/IPC envelope。

### 3.2 IPC / SDK 形态

候选：

1. Rust library embedding；
2. local HTTP / JSON-RPC sidecar；
3. gRPC sidecar；
4. headless CLI JSON mode。

建议顺序：先稳定 Rust facade，再做 local HTTP/JSON-RPC。这样 Mac/Flutter/Web host 都能接入，且不会把 CLI 输出当生产协议。

当前 Step 04-02 选择的首个可测试 Host 接入形态是 `dock-cli runtime-json`：

- transport mode：`headless-cli-json`；
- binding：`local-process-stdio`；
- 目标：给非 Rust Host 一个本地进程级、machine-readable 的 JSON envelope，同时继续复用 04-01 的 `RuntimeService` facade；
- 非目标：不在本 Step 启动 HTTP/gRPC sidecar，不把旧的 developer CLI JSON 当作生产 IPC 协议。

请求 envelope：

```json
{
  "apiVersion": "dock.runtime.v1",
  "requestId": "req-1",
  "method": "runtime.callApi",
  "params": {}
}
```

响应 envelope 必须包含：

- `apiVersion`、`requestId`、`method`、`status`；
- `result` 或 `error`；
- `redaction.marker = "[REDACTED]"`、`redaction.policy = "dock.runtime.redaction.v1"`；
- `transport.mode = "headless-cli-json"`、`transport.binding = "local-process-stdio"`。

当前支持的 IPC method：

| Method | Runtime facade |
|---|---|
| `runtime.negotiateVersion` | `negotiate_runtime_version()` |
| `runtime.validateSkill` | `RuntimeService::validate_skill()` |
| `runtime.loadSkill` | `RuntimeService::load_skill_response()` |
| `runtime.hostContract` | `RuntimeService::host_contract()` |
| `runtime.concurrencyPolicy` | `RuntimeService::concurrency_policy()` |
| `runtime.callApi` | `RuntimeService::call_api()` |
| `runtime.renderComponent` | `RuntimeService::render_component()` |
| `runtime.dispatchComponentAction` | `RuntimeService::dispatch_component_action()` |
| `runtime.expireCards` | `RuntimeService::expire_cards()` |
| `runtime.getAuditRecords` | `RuntimeService::get_audit_records()` |
| `runtime.cancelOperation` | `RuntimeService::cancel_operation()` |
| `runtime.closeSession` | `RuntimeService::close_session()` |

安全边界：

- IPC 只负责传输，不允许绕过 permission、ConsentGate、audit、redaction 或 package integrity gate；
- `capabilityToken` 可作为请求参数进入 Runtime facade，但 request DTO 不派生 `Debug` 且响应默认不序列化该字段；
- invalid method、invalid params、version mismatch 和 request parse/schema error 都返回 redacted envelope；
- 当前 binding 是本地进程 stdio，后续 HTTP/JSON-RPC sidecar 必须单独补 loopback/socket binding、安全配置和 conformance tests。

### 3.3 Skill Registry / Cache

开发项：

- merchant Agent manifest；
- skill package URL；
- package digest/signature；
- local cache；
- version pinning；
- rollback；
- cache eviction；
- package.zip 从 demo no-op 变为真实候选路径。

当前 Step 04-03 已在 `skill-loader` 内冻结本地 registry/cache/version/rollback contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| Skill reference | `SkillReference` 支持 local path、package URL 和 registry id；保留 `skillId`、`publisherDid`、`version`、`digest` | 真实远端 marketplace/discovery 仍未接入。 |
| Registry trait | `SkillRegistry::resolve_skill()` 和 `LocalSkillRegistry` 可用本地 manifest/fixture 模拟 merchant registry | 不执行生产 HTTP download；真实网络必须后续接入 allowlist/HTTPS/DID policy。 |
| Cache key | `SkillCacheKey = publisher DID + skill id + version + digest`，目录名经过 sanitize | cache 命中也会重新用 registry digest 和 package integrity policy 验证。 |
| Verified cache | `SkillCache::load_or_insert()` 先验证源包 digest、签名、publisher allowlist，再复制到 digest-keyed cache 并设置 readonly | readonly 是本地文件权限 gate，不等同部署级防篡改或加密存储。 |
| Version selection | `Latest`、`Pinned(version)`、`Rollback { beforeVersion }`，默认不选 prerelease，显式 `registry_id_with_prerelease()` 才允许 | 版本比较为 semver-like numeric order，仍不替代生产 release policy。 |
| Rollback / eviction | 支持 rollback pin，`evict_unpinned()` 不删除 retain set 或 rollback pin | cache cleanup、quarantine 生命周期和 privacy/delete hooks 后续由 04-08 承接。 |
| Audit summary | `CachedSkillMetadata::audit_summary()` 输出 source type、package ref、publisher、skill、version、digest、supply-chain status、cache flags，并脱敏 package URL secret/query | 不输出本机 cache root 或 private path。 |

当前 Step 04-08 已在 `skill-loader` 内冻结 Skill cache cleanup/quarantine contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| cleanup metadata | cache root 下写入外部 sidecar `*.dock-cache.json`，记录 `SkillCacheKey`、可选 `merchantDid`、package source summary、production-ready flag、quarantine flag 和 last-used timestamp | sidecar 不写入 Skill 包目录，避免改变包 digest；04-03 之前没有 sidecar 的 legacy cache 只能被全量 cleanup 匹配，不能按 publisher/Skill 反解。 |
| dry-run/report | `SkillCacheCleanupPolicy` + `SkillCacheCleanupReport` 支持 dry-run、delete scope、retain set、quarantine purge policy 和 redaction metadata | report 不输出 cache root、本机绝对路径或 package URL secret/query；当前是 Rust API contract，不新增 CLI 命令。 |
| scope cleanup | `SkillCacheCleanupScope` 可按 publisher DID、merchant DID、Skill id、version、digest 过滤；用于 Host/ops 后续串联 privacy/delete scope hooks | cache 目录名仍保持 04-03 的 publisher DID + Skill id + version + digest，不把 merchant DID 加入目录名。 |
| rollback protection | cleanup 会保留 active retain set 和 `pin_rollback()` 记录的 rollback key | 错误清理若仍删除外部备份或部署级 cache，不在本地 contract 覆盖范围内。 |
| quarantine lifecycle | `SkillCache::quarantine()` 写入 redacted reason；后续 `load_or_insert()` 看到 quarantined sidecar 会 fail closed；cleanup 可按 digest/scope purge quarantined cache | 真实远端 registry quarantine feed、签名吊销同步和 CI release report 仍待 Phase 6。 |

04-03 不声明真实 ANP Agent registry、远端 zip 下载、生产签名算法 verifier 或生产 publisher trust policy 配置已完成；这些仍是 Phase 4/6 的后续 release blocker。

### 3.4 Runtime Config 与 Secret Store

先冻结配置和 secret 边界，再分别实现具体持久化 backend，避免一个 Step 同时跨 token、storage、audit、cache 和 migration。

配置范围：

| 配置 | 策略 |
|---|---|
| runtime profile | dev/demo/production 明确区分 |
| identity/resolver | 保存 provider reference，不保存 private key material |
| token issuer | 保存 secret reference，不保存 token 或 issuer secret |
| storage/audit/cache path | 保存 workspace/runtime 相对或配置路径，输出时 redacted |
| Host providers | 声明 provider handle、capability 和 mock/dev 标记 |

要求：

- config 文件只允许 non-secret value 和 secret reference；
- secret material 只能来自 env、Secret Store 或 Host credential provider；
- production profile 缺少 required provider 时 fail closed 或 release blocked。

当前 Step 04-04 已在 `dock-core` 冻结 `dock.runtime.config.v1` contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| profile | `development`、`demo`、`production` 明确区分；默认是 development | production 缺少 required provider 时只产生 release blocker，不自动创建真实 provider。 |
| config load priority | `builtInDefault -> configFile -> environment -> cliArgument -> hostOverride` | 本 Step 冻结优先级枚举，不实现完整文件/环境合并器。 |
| secret reference | 只允许 `env`、`secretStore`、`hostCredentialProvider` reference | 不解析、不读取、不缓存真实 secret material。 |
| provider/path handle | identity、resolver、allowlist、token issuer、storage、audit、cache、Host provider 均使用 reference/handle | storage/audit/cache 的实际 backend 由 04-05 至 04-08 分别实现。 |
| diagnostics | `redactedDiagnostics()` 只输出 configured 状态、profile、capability summary 和 redaction metadata | 不输出 secret store key、private key path、本机绝对路径、token、Authorization、signature 或 raw secret。 |
| production release blockers | 缺少 identity/resolver/allowlist/token issuer secret、使用 in-memory backend、缺少 render/consent Host provider、启用 mock/dev-only provider 均 block release | 不把 demo/mock/in-memory 配置写成 production-ready。 |

### 3.5 持久化

持久化范围：

| 数据 | 策略 |
|---|---|
| token cache | secure store / encrypted SQLite，短期 TTL |
| scoped storage | SQLite，按 DID/merchant/Skill scope，quota |
| audit | append-only 或 SQLite，retention |
| skill cache | digest-keyed directory，read-only after verify |
| runtime config | 只由 3.4 负责 schema、secret boundary 和 provider reference |

当前 Step 04-05 已在 `anp-adapter` 冻结 capability token cache persistence contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| persistence backend | `TokenCachePersistenceBackend` trait 支持 load/replace entry；profile 区分 `inMemoryDev`、`hostSecureStore`、`encryptedBackend` | 当前只提供 `InMemoryTokenCachePersistenceBackend` 作为 dev/test backend；生产必须接 Host secure store 或 encrypted backend。 |
| persisted entry | entry 绑定 scope、issuer、audience、jti、expiry 和 raw token secure boundary | `PersistentCapabilityTokenEntry` Debug redacted，且不作为公开 JSON diagnostics 输出。 |
| restart restore | `PersistentCapabilityTokenCache::restore()` 只恢复未过期、signature/trust 有效、metadata/claims/scope 匹配、未 revoked、未 consumed once 的 token | rejected entry 会从 backend snapshot 清掉，并只在 report 中暴露 scope summary 与 reason。 |
| redacted report | `TokenCacheRestoreReport` 只输出 backend profile、production-ready flag、计数、rejection reason 和 redaction metadata | 不输出 raw token、Authorization、signature、private key path 或 secret。 |
| failure policy | fallible `try_put()` / `try_clear()` 先写 persistence snapshot，成功后才更新内存 cache；`CapabilityTokenCache` trait 方法复用 fail-closed 路径 | 若 production backend 不可用，写入/清理不会先污染内存状态；调用方若需要错误详情应使用 fallible API。 |

当前 Step 04-06 已在 `wx-compat` 冻结 scoped storage persistence contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| scope | `StorageScope = user DID + merchant DID + Skill id + namespace`，默认 namespace 为 `default` | Atomic API VM 当前仍使用默认 namespace；更多 Host-managed namespace 需后续显式 wiring。 |
| persistence backend | `ScopedStoragePersistenceBackend` trait 支持 load/restore snapshot 和 replace entries；profile 区分 `inMemoryDev`、`localFileUnencrypted`、`hostEncryptedStore`、`encryptedSqlite` | 只有 Host encrypted store / encrypted SQLite profile 标记 production-ready；local file JSON backend 未加密，只能 dev/test/local evidence。 |
| restart restore | `PersistentScopedStorage::restore()` 恢复合法 entry，拒绝 invalid 或超 quota entry，并重写 backend snapshot 清理 rejected entry | corrupt JSON snapshot 整体 fail closed，需 Host repair/reset；生产 repair/backup 策略后续补。 |
| quota / cleanup | `try_set_storage()` 先校验 aggregate quota 并写 backend snapshot，成功后才更新内存；支持 remove、clear 和 delete scope 持久化 | 生产 privacy deletion 仍需 Host/ops runbook 串联 token/audit/cache 清理。 |
| redacted report | `StorageRestoreReport` 只输出 backend profile、production-ready flag、计数、scope summary、key/value bytes、reason 和 redaction metadata | 不输出 raw key、raw value、token、Authorization、private path 或隐私原文。 |

当前 Step 04-07 已在 `consent-audit` 和 `dock-core` 冻结 persistent audit retention/export contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| persistence profile | `AuditPersistenceProfile` 区分 `inMemoryDev`、`localFileJsonl`、`hostPersistentSink`、`encryptedSqlite` | 只有 Host persistent sink / encrypted SQLite profile 标记 `productionReady = true`；`FileAuditSink` 的 `localFileJsonl` 未加密，只能作为 dev/test/local evidence。 |
| record schema | `AuditRecord` 保存 user/agent/merchant/session/Skill/API、risk、outcome、redacted parameter summary、redacted `permissionDecision` 和 redacted consent proof | 不保存 raw token、Authorization、signature、private key material、手机号、地址或文件内容。 |
| retention/export | `AuditExportReport` 和 `AuditRetentionReport` 输出 backend profile、production-ready flag、计数和 redaction metadata；`FileAuditSink` 支持 restart/query/export/retention | JSONL 文件后端仍缺少部署级加密、访问控制、迁移、锁策略、export approval 和 privacy deletion。 |
| Runtime wiring | `RuntimePersistentAuditSink` 把 `consent-audit::AuditSink` 接入 `dock-core::AuditSink` 和 `RuntimeAuditReader`；`runtime.getAuditRecords` 读取失败返回稳定 `audit_unavailable` | 当前仅提供本地文件 reader wiring；真实 Host audit provider 需要 04-09/06-06 补 conformance 和运维配置。 |
| unavailable policy | L3/L4 consent 通过后、executor 执行前调用 audit sink `ensure_available()`；不可用时返回 `audit_unavailable`，不执行高风险 API | preflight 只能证明当下可写；生产 backend 仍需事务性/append durability、告警和监控。 |

拆分顺序：

1. token cache 持久化与恢复；已由 04-05 完成本地 contract 和 dev-only backend gate；
2. scoped storage 持久化与 quota；已由 04-06 完成本地 contract、quota/restore/delete-scope gate 和 dev-only local file backend gate；
3. persistent audit sink retention/export；已由 04-07 完成本地 contract、retention/export report、Runtime wiring 和高风险 audit unavailable fail-closed gate；
4. Skill cache cleanup 与版本清理；已由 04-08 完成本地 API contract、dry-run/report、scope cleanup、rollback pin protection 和 quarantine fail-closed gate。

### 3.6 Host Adapter Contract

Host 必须实现或声明不支持：

- Render IR renderer；
- CardSpec fallback renderer；
- consent prompt；
- phone/address/media/file/location/payment providers；
- openDetailPage fallback；
- event dispatch；
- secure identity provider。

当前 Step 04-09 已在 `dock-core` 冻结 `dock.host-adapter.v1` 本地 contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| capability declaration | `HostAdapterContract` 声明 required、optional 和 unsupported-by-design capability；`runtime.hostContract` 可通过 Runtime facade / IPC 查询 | `HeadlessHostAdapter` 只用于 headless/mock conformance，`productionReady = false`，不得冒充真实 Host。 |
| action routing | `api/call` 固定走 `runtime-orchestrator`，重新经过 input validation、permission、ConsentGate、audit 和 executor；`sendFollowUpMessage`、`openDetailPage`、`expirePreviousCards` 进入 Host adapter outcome | Host 不允许把 `api/call`、payment、phone、address、location、file/media 等高风险动作直接执行成系统调用。 |
| fail closed / unsupported | custom Host 未声明能力时默认 `Unsupported`；unknown Host action 返回 stable unsupported outcome | 不出现 silent success；真实 Host 支持新 action 前必须更新 contract、测试和矩阵。 |
| detail page canonicalize | `openDetailPage` 只接受 safe relative path/query；拒绝外部 URL、`javascript:`、`file:`、`..`、encoded traversal 和敏感 query | 真实外链/详情页 UI 仍需 Host policy、consent 和 allowlist。 |
| redaction | `HostActionOutcome` 带 `dock.host-action.redaction.v1` metadata；payload 默认脱敏 token、Authorization、signature、private、secret、credential 和本机路径 | 不声明完整 production Host renderer/provider 已完成。 |

Host 不允许：

- 直接把组件 action 变成高风险系统调用；
- 向 Skill JS 暴露 token/private key；
- 绕过 Runtime audit。

### 3.7 并发、取消与幂等

当前 Step 04-10 已在 `dock-core` 冻结 Runtime 本地并发、取消、retry 和幂等 contract：

| 能力 | 当前 contract | 边界 |
|---|---|---|
| policy query | `RuntimeService::concurrency_policy()` / `runtime.concurrencyPolicy` 输出 `dock.runtime.concurrency.v1`；低风险 API 可同 session 并发，高风险串行 scope 为 `session + api + idempotencyKey` | 这是本地 RuntimeService contract，不是跨进程或跨 Host 的全局锁协议。 |
| operation metadata | `RuntimeOperationOptions` 可携带 `operationId`、`cancellationToken`、`timeoutMs`、`idempotencyKey`；`RuntimeCallRequest` 和 `RuntimeDispatchComponentActionRequest` 均可传入 | `operationId` 当前保留用于 Host 关联；不进入 executor 参数。 |
| cancellation / timeout | `cancel_operation()` 把 token 写入 session 绑定的取消注册表；dispatch 前若 session 已关闭、token 已取消或 `timeoutMs = 0`，Runtime 在 executor/Host action 前 fail closed | 当前是 pre-dispatch/deadline check；同步 executor/provider 一旦开始执行，Runtime 不声明抢占式中断。 |
| 高风险串行 | `RiskLevel::requires_consent()` 的 L3/L4 API 在同一 session、同一 API、同一 optional idempotency key 下只能有一个 in-flight；重复并发返回稳定 permission error | 不带 idempotency key 的旧调用仍按 `session + api + none` 串行拒绝同类并发，但不强制旧调用必须补 key。 |
| 幂等 key | `operation.idempotencyKey` 会注入/校验为 `arguments.idempotencyKey`，进入 Orchestrator、ConsentGate 和 audit parameter summary；显式 key 的高风险成功结果可在同一 RuntimeService 内 replay，避免重复 executor 调用 | replay 是内存级、本地 session 级；session close 会清理本地 replay cache；生产耐久幂等仍需 merchant/provider contract。 |
| retry policy | RequestBroker 保持只对 401 auth challenge / stale token 做受控认证重试；非幂等业务 5xx/4xx 默认返回给调用方，不自动重试 | 真实 merchant/provider 如需业务重试必须提供幂等 contract 和 audit/ops policy。 |
| session close | `close_session()` 关闭本地 Runtime session，清理本地 cancellation、in-flight 高风险状态和 replay cache，后续 API/Host action dispatch fail closed | 不等同删除 token/storage/audit/cache；隐私删除仍待后续 Host/ops runbook 串联。 |
| component cleanup | dynamic request/timer 的 expire/detach cleanup 继续由 Component VM gate 证明；04-10 复用现有 focused cleanup test 作为证据 | 不声明 production Host background scheduler 已完成。 |

后续仍需保留的 release blocker：

- 分布式 / 跨进程高风险 transaction lock；
- merchant/provider 侧耐久 idempotency store；
- 真实 Host provider 的 cancellation、timeout 和 background lifecycle；
- request audit persistence、metrics 和 CI gate 自动化。

## 4. 测试计划

| 测试 | 内容 |
|---|---|
| runtime API tests | load/call/render/action/expire/audit |
| IPC tests | JSON schema、错误码、version |
| persistence tests | restart 后 storage/token/audit 行为 |
| multi-session tests | user/merchant/skill 隔离 |
| host contract tests | mock Host provider success/fail |
| rollback tests | skill version pin/rollback |

## 5. 阶段完成检查

- [x] CLI 使用同一 Runtime API。
- [x] Headless/local process Host boundary 通过稳定 JSON envelope 和 `dock.host-adapter.v1` contract 接入；真实 production Host UI/provider 仍是 release blocker，不能写成已完成。
- [x] Skill package 本地 registry/cache 支持 digest-keyed cache、校验、version pin、rollback、cleanup 和 quarantine；真实远端 registry download、生产签名 verifier、publisher trust policy 配置和部署级 cache hardening 仍是 release blocker。
- [x] runtime config 与 secret store 边界冻结，且 storage/token/audit/cache 分别有 focused local contract、dev/test backend 或明确 release blocker；未加密 local backend 不得 production-ready。
- [x] 多 session 隔离、高风险串行、pre-dispatch cancellation/timeout、idempotency key forward/replay 和非幂等 no-retry 策略有测试；分布式 lock、耐久幂等和 provider 抢占式取消仍是 release blocker。
