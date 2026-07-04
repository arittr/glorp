use std::process::ExitCode;

fn main() -> ExitCode {
    let repo_root = match std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
        Some(path) => path.to_path_buf(),
        None => {
            eprintln!("xtask: failed to resolve repository root");
            return ExitCode::FAILURE;
        }
    };

    match xtask::run_xtask(std::env::args().skip(1), &repo_root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
