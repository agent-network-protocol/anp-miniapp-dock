#![doc = "High-risk action consent and audit trail crate."]

pub const CRATE_NAME: &str = "consent-audit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "consent-audit");
    }
}
