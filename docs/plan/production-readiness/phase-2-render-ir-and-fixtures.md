# Phase 2 子文档：Render IR 与 Fixture 体系

## 1. 目标

Render IR 是容器与 Host renderer 的稳定边界。本文定义 Render IR 版本化、snapshot、fixture 和 fallback 的开发方案，确保组件运行时可以不断增强，而不会强迫 Host 立即支持完整 UI。

## 2. Render IR Contract

每个 Render IR 输出建议包含：

```json
{
  "schemaVersion": "dock.render-ir.v1",
  "componentPath": "components/order-confirm/index",
  "renderId": "...",
  "root": {},
  "actions": [],
  "warnings": [],
  "accessibility": {},
  "debug": { "redacted": true }
}
```

`debug` 只允许在 dev/headless 输出，且必须 redacted。

## 3. Node Kind Registry

P0 已有：

- `view`
- `text`
- `image`
- `button`
- `scroll-view`

P1 建议新增：

- `input`
- `textarea`
- `radio`
- `checkbox`
- `picker`
- `map-preview`
- `canvas-static`

Host 处理规则：

- unknown node kind：渲染 fallback placeholder 或整卡 fallback；
- unknown style：忽略并记录 warning；
- unknown action：不执行，回传 unsupported action。

## 4. Action Registry

动作必须是数据，不是宿主函数引用：

| Action | 来源 | 处理方 |
|---|---|---|
| `sendFollowUpMessage` | modelContext | Host / Agent message layer |
| `api/call` | modelContext content block | Orchestrator |
| `expirePreviousCards` | viewContext | Runtime card manager |
| `expireAllCards` | modelContext | Runtime card manager |
| `openDetailPage` | viewContext | Host fallback |
| `setRelatedPage` | viewContext | Host metadata |

高风险动作不得直接通过 Render IR 执行；必须回到 Orchestrator 或 Host provider consent。

## 5. Snapshot 规则

1. Snapshot 不包含随机 id、时间戳、token、signature。
2. 动态字段使用 stable render id 或测试 normalization。
3. Snapshot 分层：
   - `render.root`；
   - `actions`；
   - `warnings`；
   - `audit summary`。
4. Render IR breaking change 必须：
   - bump schema version；
   - 更新 migration note；
   - 更新 Host adapter contract。

## 6. Fixture 目录建议

```text
examples/fixtures/
  coffee/
  address-form/
  media-review/
  dynamic-status/
  location-map-preview/

testdata/render-ir/
  coffee.searchDrinks.render.json
  coffee.confirmOrder.render.json
  address-form.submit.render.json
```

如果暂不新增目录，也可先把 fixture 放在 owning crate tests 下，但长期应集中到 `testdata/`，便于不同 crate 共享。

## 7. Fallback Contract

Fallback 原因枚举建议：

- `no_component_path`
- `component_missing`
- `component_load_failed`
- `component_vm_failed`
- `wxml_parse_failed`
- `wxss_parse_warning_threshold`
- `unsupported_node_kind`
- `host_renderer_unavailable`
- `api_error`
- `empty_structured_content`

Fallback 输出顺序：

```text
Component Runtime
  -> native adapter if registered
  -> CardSpec from structuredContent
  -> text from content
```

## 8. 完成标准

- [ ] Render IR 有 schema version。
- [ ] Node/action registry 文档与代码枚举一致。
- [ ] 每个 fixture 有 snapshot。
- [ ] Host adapter 可以根据 contract 单独实现。
- [ ] fallback reason 可被 CLI 和 audit 观察。
