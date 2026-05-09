fn main() {
    if let Err(err) = glorp::run() {
        eprintln!("glorp: {err}");
        std::process::exit(1);
    }
}
