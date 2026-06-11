# Phase 3：安全增强与可信执行实施计划

## 1. 阶段目标

Phase 3 要把安全能力从“Demo 中有边界”升级为“线上默认安全”。所有新增接口和组件能力都必须通过权限、沙箱、DID、token、consent、audit 和供应链控制。

深入威胁模型见：[Threat Model 与安全控制](phase-3-threat-model-and-controls.md)。

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

每个威胁必须有：控制措施、测试、残余风险、owner。

### 3.2 Sandbox 加固

加固项：

- 禁用 `eval`、`Function`、async/generator constructor escape；
- 禁用 `process`、`fetch`、`WebSocket`、timer，除非 broker 显式开放；
- CommonJS 只能包内 require；
- memory、stack、CPU timeout、Promise job drain、console size、result size 限制；
- 每次 API call 独立 context；
- component expire/detach 后不可继续执行事件或 timer。

验收：sandbox escape tests 必须进入 CI。

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

### 3.6 Skill 包供应链

开发项：

- Skill package digest；
- package signature；
- publisher DID；
- trusted publisher allowlist；
- package cache quarantine；
- symlink / path canonicalization；
- remote code 禁止。

## 4. 安全测试 Gate

| Gate | 示例 |
|---|---|
| sandbox escape | Function constructor、prototype constructor、process/fetch/WebSocket |
| path escape | absolute path、`..`、symlink outside package |
| network deny | non-allowlist host、Authorization override |
| token security | replay、expired、wrong scope、wrong audience |
| consent bypass | L3/L4 API without consent |
| redaction | token/signature/private/phone/address/file content |
| package integrity | digest mismatch、signature mismatch、unknown publisher |

## 5. 阶段完成检查

- [ ] threat model 完成并链接到 release gates。
- [ ] sandbox escape tests 进入 CI。
- [ ] permission engine 默认 fail closed。
- [ ] DID/token lifecycle 覆盖 refresh/revoke/replay。
- [ ] audit 可持久化且默认脱敏。
- [ ] Skill 包 digest/signature 有实现计划或初版实现。
