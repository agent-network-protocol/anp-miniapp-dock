# Phase 3：安全增强与可信执行实施计划

## 1. 阶段目标

Phase 3 要把安全能力从“Demo 中有边界”升级为“线上默认安全”。所有新增接口和组件能力都必须通过权限、沙箱、DID、token、consent、audit 和供应链控制。

深入威胁模型见：[Threat Model 与安全控制](phase-3-threat-model-and-controls.md)。

当前执行状态：Step 03-01 冻结风险分级、owner、required gate 和 release blocker 口径；Step 03-02 已把 sandbox/resource gate 升级为本地 required release gate；Step 03-03 已补齐 permission decision、network allowlist 和 decision audit 的本地 required gate；Step 03-04 已补齐 DID/token lifecycle、challenge replay 和 resolver trust anchor 的本地 required gate；Step 03-05 已补齐 Host consent adapter、ConsentProof 元数据、provider unavailable audit 和 append-only JSONL audit sink 本地 gate；Step 03-06 已补齐 package digest、manifest supply-chain contract、trusted publisher allowlist、quarantine、remote require/path/symlink/zip slip deny 和 validate supply-chain report 的本地 gate；Step 03-07 已完成 Phase 3 最终 Review 与整体验证，作为 Phase 4 启动前 gate。本文不把尚未落地的 CI 自动化、生产 Host 配置、真实 registry/cache、生产签名 verifier 或部署级持久化加密能力写成已完成。

## 2. 涉及模块

| 模块 | 安全职责 |
|---|---|
| `skill-loader` | 包路径、symlink、digest、签名、publisher DID |
| `js-runtime-quickjs` | API VM sandbox、资源限制、escape regression |
| `component-runtime` | Component VM sandbox、dynamic request/timer 限制 |
| `wx-compat` | capability profile、permission decision、unsupported fail closed |
| `anp-adapter` | DID proof、signed request、token lifecycle、resolver |
| `consent-audit` | risk policy、proof、redaction、audit sink |
| `dock-core` | enforcement order：validation -> permission -> consent -> execution -> audit |
| `demo-server` | server-side token validation、scope、audit redaction |

## 3. 开发顺序

### 3.1 Threat model 先行

先完成 `docs/security/threat-model.md`，至少覆盖：

- 恶意 Skill；
- 被篡改 Skill 包；
- 恶意商家 Agent；
- 网络中间人；
- 恶意或误配置 Host provider；
- 日志/审计读取者；
- 本地文件系统攻击者。

每个威胁必须有：控制措施、测试 gate、残余风险、owner。Step 03-01 的输出是 `docs/security/threat-model.md` 中的 L0-L4 风险等级和 L3/L4 能力控制矩阵，后续实现 Step 必须反向链接到该矩阵。

### 3.2 Sandbox 加固

加固项：

- 禁用 `eval`、`Function`、async/generator constructor escape；
- 禁用 `process`、`fetch`、`WebSocket`、timer，除非 broker 显式开放；
- CommonJS 只能包内 require；
- memory、stack、CPU timeout、Promise job drain、console size、result size 限制；
- 每次 API call 独立 context；
- component expire/detach 后不可继续执行事件或 timer。

验收：Step 03-02 已让 sandbox escape/resource limit tests 成为本地 release gate；在 CI 自动化落地前，runbook 必须记录本地命令、测试范围和残余风险。

### 3.3 权限策略引擎

策略输入：

- `mcp.json` 标准字段；
- `components[].permissions.scope.dynamic`；
- `_meta.anp` / `x_anp`；
- Host policy override；
- 用户 consent decision；
- merchant trust policy。

策略输出：

```text
Allow | Deny(reason) | Prompt(consent_request) | MockAllowed(dev_only)
```

原则：

- 未声明敏感权限默认 deny；
- mock provider 只能在 dev/headless explicit flag 下启用；
- permission decision 必须进 audit。
- Host deny override 优先于 Skill manifest；
- 网络 allowlist 至少覆盖 scheme、host、port、path prefix、method 和 scope。

### 3.4 DID / Token 安全

开发项：

- token refresh / revoke / logout；
- challenge nonce 一次性和 TTL；
- DID document resolver cache + TTL + trust anchor；
- token claims version；
- jti replay 防护；
- scope derivation 记录来源；
- secret store integration 规划。

验收：

- wrong DID、wrong audience、expired token、missing scope、replay challenge 全部失败；
- 私钥路径和 token 不进入任何输出。
- resolver 不可信、replay store 不可用或 revoke 状态不可确认时默认 fail closed。

### 3.5 Consent 与 Audit 生产化

开发项：

- host consent adapter trait；
- consent prompt digest；
- ConsentProof policy version；
- persistent audit sink（SQLite 或 append-only 文件）；
- audit retention policy；
- redaction regression suite；
- audit export 默认脱敏。

高风险 API：

- L3：下单、支付、退款、外部交易；
- L4：手机号、地址、身份、位置、文件、外部链接。
- dev/headless mock consent 不能作为 production-ready provider。

### 3.6 Skill 包供应链

开发项：

- Skill package digest；
- package signature；
- publisher DID；
- trusted publisher allowlist；
- package cache quarantine；
- symlink / path canonicalization；
- remote code 禁止。
- 本地 coffee demo 未签名状态必须保持 dev/demo-only，不得被 validate 或文档标为 production-ready。

## 4. 安全测试 Gate

| Gate | 示例 | 对应 Step | 当前状态 |
|---|---|---|---|
| threat classification | L0-L4、L3/L4 控制矩阵、owner、release blocker | 03-01 | 当前 Step 收敛 |
| sandbox escape | Function constructor、prototype constructor、process/fetch/WebSocket、timer/result/console limit | 03-02 | 本地 required release gate 已升级；CI 自动化待 Phase 6 |
| path escape | absolute path、`..`、symlink outside package、zip slip、remote require | 03-06 | 本地 required release gate 已升级：loader path/digest traversal、archive entry validator 和 QuickJS remote require tests 已覆盖；真实 zip extraction/registry download 待 Phase 4 |
| network deny | non-allowlist host、scheme/path/method/scope mismatch、Authorization override | 03-03 | 本地 required release gate 已升级；生产 Host transport/registry 配置来源和 persistent request audit 待 Phase 4/03-05 |
| token security | replay、expired、wrong scope、wrong audience、resolver mismatch、revoke/logout | 03-04 | 本地 required release gate 已升级；生产持久化 token cache/revocation restore、跨进程 replay store、DID network/rotation 和 secret store 待 Phase 4/6 |
| consent bypass | L3/L4 API without consent、denied、provider unavailable | 03-05 | 本地 required release gate 已升级：Host consent adapter、ConsentProof policy/prompt/actor/digest 字段、provider unavailable fail-closed audit |
| redaction | token/signature/private/phone/address/file content、audit export | 03-05 | 本地 required release gate 已升级：append-only JSONL audit sink 支持 restart/query/export/retention，export 默认脱敏 |
| package integrity | digest mismatch、signature mismatch、unknown publisher、quarantine | 03-06 | 本地 required release gate 已升级：normalized package digest、`_meta.anp.supplyChain` contract、production quarantine、validate supply-chain report；真实 registry/cache 和生产签名 verifier 待 Phase 4/6 |

## 5. 阶段完成检查

- [x] threat model 完成并链接到 release gates，作为 Step 03-01 控制矩阵基线。
- [x] sandbox escape/resource tests 成为本地 required release gate。
- [ ] sandbox escape tests 进入 CI。
- [x] permission engine 默认 fail closed，并覆盖 Host deny override、manifest/meta/dynamic 声明、mock dev/headless、decision audit。
- [x] DID/token lifecycle 覆盖 refresh/revoke/replay。
- [x] audit 可持久化且默认脱敏。
- [x] Skill 包 digest/signature 有实现计划和初版实现。
- [x] Step 03-07 完成 Phase 3 最终 Review 与整体验证后，才能作为 Phase 4 启动 gate。
