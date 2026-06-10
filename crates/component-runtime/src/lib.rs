#![doc = "MiniApp MCP Component Runtime, WXML/WXSS subset, events, and Render IR crate."]

pub const CRATE_NAME: &str = "component-runtime";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "component-runtime");
    }
}
