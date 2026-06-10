#![doc = "ANP DID, signed HTTP, challenge, and capability token adapter crate."]

pub const CRATE_NAME: &str = "anp-adapter";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "anp-adapter");
    }
}
