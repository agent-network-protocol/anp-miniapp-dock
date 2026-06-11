# anp-miniapp-dock 分享材料

本目录是 15 分钟技术分享的交付物，主题为：**开源的智能体原生小程序 MCP 容器：用 ANP DID 打通平台身份之外的开放服务入口**。

## 文件说明

- `anp-miniapp-dock-sharing.pptx`：可编辑 PowerPoint，12 页，适合 15 分钟分享。
- `anp-miniapp-dock-sharing-outline.md`：逐页大纲、时间分配和视觉规划。
- `anp-miniapp-dock-sharing-script.md`：逐页中文口播文字稿。
- `image-assets.md`：图片资产、用途和 `$imagegen` 生成提示词摘要。
- `source-notes.md`：本分享引用的架构/协议文档要点映射。
- `assets/`：PPT 使用的图片资产，其中主视觉、Runtime 容器图和咖啡 DID 流程图由 `$imagegen` 生成；`*-dim*.png` 是构建 PPT 时生成的暗色背景衍生图。
- `preview/anp-miniapp-dock-sharing.pptx.png`：Quick Look 生成的首页预览，用于快速检查风格。
- `build_anp_miniapp_dock_deck.py`：PPTX 生成脚本，便于后续修改重建。

## 重建 PPTX

```bash
python3 -m venv /tmp/anp-miniapp-ppt-venv
/tmp/anp-miniapp-ppt-venv/bin/pip install -r ppt/requirements.txt
/tmp/anp-miniapp-ppt-venv/bin/python ppt/build_anp_miniapp_dock_deck.py
```

在 macOS 上可用 Quick Look 生成首页预览：

```bash
rm -rf ppt/preview && mkdir -p ppt/preview
qlmanage -t -s 1280 -o ppt/preview ppt/anp-miniapp-dock-sharing.pptx
```
