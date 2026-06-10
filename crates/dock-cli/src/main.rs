fn main() {
    if let Err(error) = dock_cli::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
