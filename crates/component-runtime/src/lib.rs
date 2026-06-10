#![doc = "MiniApp MCP Component Runtime, WXML/WXSS subset, events, and Render IR crate."]

pub mod compiler;
pub mod loader;
pub mod render_ir;
pub mod wxml;
pub mod wxss;

pub use compiler::{
    compile_component_to_render_ir, compile_wxml_to_render_ir, BindingContext,
    ComponentCompileError, ComponentRenderOutput,
};
pub use loader::{ComponentLoadError, ComponentPackage};
pub use render_ir::{
    ComponentAction, RenderEventBinding, RenderEventKind, RenderNode, RenderNodeKind, RenderStyle,
};
pub use wxml::{parse_wxml, WxmlElement, WxmlNode, WxmlParseError};
pub use wxss::{merge_styles, parse_inline_style, WxssStyleSheet};
