#![doc = "MiniApp MCP Skill package loading and path resolution crate."]

pub const CRATE_NAME: &str = "skill-loader";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "skill-loader");
    }
}
