const BINARY_NAME: &str = "demo-server";

fn main() {
    println!("{BINARY_NAME} scaffold");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binary_name() {
        assert_eq!(BINARY_NAME, "demo-server");
    }
}
