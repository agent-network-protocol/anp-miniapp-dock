# Phase 3 子文档：Threat Model 与安全控制

本文是 `docs/security/threat-model.md` 的 Phase 3 摘要。完整风险等级、L3/L4 控制矩阵、owner、required gate、残余风险和 release blocker 以 `docs/security/threat-model.md` 为准；Step 03-02 sandbox/resource、Step 03-03 permission/allowlist/decision audit、Step 03-04 DID/token lifecycle、Step 03-05 consent/audit persistence、Step 03-06 package integrity/supply-chain 本地 release gate 已补齐；真实 registry/cache、生产签名 verifier、CI 自动化仍不得写成已完成。

## 0. 风险等级

| 等级 | 范围 | 默认发布策略 | 必需控制 |
|---|---|---|---|
| L0 | 公开读、常量、普通 Render IR 节点 | 可默认允许 | schema validation、redaction、unsupported fail shape |
| L1 | session 标识、最小设备/应用信息、DID 绑定 | 最小字段，禁止真实指纹和 secret | DID/session binding、字段最小化、redaction |
| L2 | 普通写、storage、card expiration、follow-up | scope + input validation + audit summary | permission decision、scoped storage、audit redaction |
| L3 | 下单、支付、退款、外部交易、分享外发 | 默认 Prompt 或 Deny | ConsentGate、Host provider boundary、audit、idempotency/replay plan |
| L4 | 手机号、地址、身份、位置、文件、媒体、扫码、电话、生物识别、crypto private operation | 默认 Deny 或 Prompt | least-privilege Host provider、opaque handle、no raw output、audit redaction、retention/export policy |

## 1. 资产清单

| 资产 | 保护目标 |
|---|---|
| DID private key | 永不进入 JS、日志、Render IR、audit export |
| capability token | 只在 host/request boundary 使用，短期有效，可撤销 |
| user DID / agent DID / merchant DID | 正确绑定 session 和 token scope |
| Skill package | 完整性、来源、版本、路径边界 |
| scoped storage | DID + merchant + Skill 隔离 |
| audit records | 完整、可追溯、默认脱敏 |
| Render IR | 不含私密 `_meta` 或 token |
| Host providers | 不能被 Skill 绕过 consent 调用 |

## 2. 攻击者模型

### 2.1 恶意 Skill

能力：提交含恶意 JS、路径逃逸、无限循环、读取 token、发起外部请求、伪造高风险 action。

控制措施：

- QuickJS sandbox；
- 包内 require；
- request allowlist；
- token 不暴露给 JS；
- action 回 Orchestrator；
- resource limits；
- package signing。

### 2.2 被篡改 Skill 包

能力：替换 API JS、组件、manifest、组件路径。

控制措施：

- digest/signature；
- publisher DID；
- cache quarantine；
- manifest validation；
- path canonicalization。

### 2.3 恶意商家 Agent

能力：返回恶意 component metadata、诱导请求、发错 scope、诱导泄露隐私。

控制措施：

- trusted merchant policy；
- token audience/scope；
- Host consent；
- model-visible filtering；
- audit。

### 2.4 网络中间人

能力：篡改 challenge/login/business response。

控制措施：

- HTTPS production requirement；
- DID HTTP signature；
- challenge nonce/audience/TTL；
- token signature verification；
- response size/type validation。

### 2.5 日志读取者

能力：读取 CLI 输出、server logs、audit export。

控制措施：

- centralized redaction；
- sensitive key/value detection；
- no raw token/proof/private path output；
- audit export redacted by default。

## 3. 控制矩阵

| 威胁 | 控制 | 当前证据 | Phase 3 required gate |
|---|---|---|---|
| JS escape | 禁用 eval/Function/prototype constructor/process/fetch/WebSocket，限制 timer/result/console/resource | Step 03-02 本地 gate：Atomic API VM sandbox/limit/console/pending job tests，Component VM sandbox/dynamic/snapshot size tests | CI 自动化和 resource metrics 待 Phase 6 |
| Path traversal | canonicalize + validate inside root；symlink outside package、zip slip、remote require fail closed | skill-loader package tests、QuickJS remote require test | Step 03-06 本地 gate 已完成；真实 registry zip extraction 待 Phase 4 |
| Unauthorized network | allowlist + broker only | RequestBroker deny-by-default、auth header deny tests、scheme/host/port/path/method/scope mismatch tests | Step 03-03 本地 gate 已完成；生产 Host transport、registry 配置和 persistent request audit 待 Phase 4/03-05 |
| Token leakage | host-only token + redaction | CLI/log/audit redaction tests；Step 03-04 token/session Debug redaction、JS `wx.login` receipt redaction 和 coffee E2E redaction 仍通过；Step 03-05 audit export redaction 覆盖 token/signature/private/phone/address/file content | Step 03-04/03-05 本地 gate 已完成；部署级 audit encryption/config 待 Phase 4 |
| Consent bypass | Orchestrator enforcement order | dock-core / consent-audit tests；Host consent adapter 可接 CLI/headless/mock 和未来 Host UI；provider unavailable fail closed 且记录 audit | Step 03-05 本地 gate 已完成；真实 Host UI/conformance 待 Phase 4 |
| Replay challenge | nonce one-time + TTL | demo-server/anp-adapter challenge tests；登录尝试开始即消费 challenge；`ChallengeNonceStore` 和 `TrustedDidDocumentResolver` 覆盖 replay、TTL、trust anchor 和 resolver mismatch | Step 03-04 本地 gate 已完成；跨进程 replay store 和 DID network/rotation 待 Phase 4/6 |
| Scope mismatch | token verifier expected capability | demo API tests；token claims version、scope derivation source、revoke/logout、expired eviction 和 high-risk `ConsumeOnce` jti gate 已测 | Step 03-04 本地 gate 已完成；持久化 token cache/revocation restore 待 Phase 4 |
| Permission drift | manifest、Host override、mock/dev-only、merchant trust policy 分散 | high-risk provider fail closed tests；`wx-compat::PermissionPolicyEngine`、`dock-core::permissionDecision` audit tests | Step 03-03 unified PermissionDecision audit 本地 gate 已完成；生产 Host policy UI/config 待 Phase 4 |
| Package tamper | digest/signature contract、publisher DID、trusted allowlist、quarantine；unsigned local package 只允许 dev/demo-only | skill-loader package tests、mcp-schema supply-chain validation、dock-cli validate supply-chain report | Step 03-06 本地 gate 已完成；真实 registry/cache 和生产签名 verifier 待 Phase 4/6 |

## 4. 安全红线

以下情况不得发布：

- 任何 API 可绕过 broker 直接网络出站；
- raw token/signature/private key 出现在 stdout/log/audit/Render IR；
- L3/L4 API 可在无 consent proof 下执行；
- package path 可逃逸 Skill root；
- production profile 接受 unsigned package、digest mismatch、signature mismatch 或 unknown publisher；
- sandbox escape regression 失败；
- permission decision 默认 allow 或 mock provider 被 production profile 静默启用；
- dynamic request/timer 绕过 `scope.dynamic`、RequestBroker、allowlist、resource limit 或 expire cleanup；
- unsupported API 静默成功。

## 5. 残余风险记录模板

```text
Risk:
Impact:
Likelihood:
Control:
Residual risk:
Owner:
Review date:
Release blocker: yes/no
```
