const BINARY_NAME: &str = "dock-cli";

fn main() {
    println!("{BINARY_NAME} scaffold");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binary_name() {
        assert_eq!(BINARY_NAME, "dock-cli");
    }
}
