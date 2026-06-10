#![doc = "MiniApp MCP Component Runtime, WXML/WXSS subset, events, and Render IR crate."]

pub mod render_ir;

pub use render_ir::{
    ComponentAction, RenderEventBinding, RenderEventKind, RenderNode, RenderNodeKind, RenderStyle,
};
