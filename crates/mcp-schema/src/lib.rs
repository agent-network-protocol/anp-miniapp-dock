#![doc = "MiniApp MCP manifest, result, and validation contract crate."]

pub const CRATE_NAME: &str = "mcp-schema";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "mcp-schema");
    }
}
