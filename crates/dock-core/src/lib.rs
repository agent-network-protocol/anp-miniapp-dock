#![doc = "Core orchestrator, API registry, host boundary, and shared error crate."]

pub const CRATE_NAME: &str = "dock-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "dock-core");
    }
}
