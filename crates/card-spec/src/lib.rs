#![doc = "CardSpec fallback schema and action model crate."]

pub const CRATE_NAME: &str = "card-spec";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "card-spec");
    }
}
