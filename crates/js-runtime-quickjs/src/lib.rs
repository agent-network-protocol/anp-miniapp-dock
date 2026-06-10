#![doc = "QuickJS-backed Atomic API VM and Component VM integration crate."]

pub const CRATE_NAME: &str = "js-runtime-quickjs";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "js-runtime-quickjs");
    }
}
