#![cfg(feature = "renderer-spike")]

use std::path::Path;

#[test]
fn production_modules_do_not_import_renderer_spike_dtos() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed = root.join("renderer_spike");
    let mut violations = Vec::new();
    visit(&root, &allowed, &mut violations);
    assert!(
        violations.is_empty(),
        "production modules import renderer_spike: {violations:?}"
    );
}

fn visit(path: &Path, allowed: &Path, violations: &mut Vec<String>) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.starts_with(allowed) {
            continue;
        }
        if path.is_dir() {
            visit(&path, allowed, violations);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            if text.contains("renderer_spike::fixture") || text.contains("renderer_spike::Decision")
            {
                let relative = path
                    .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                    .unwrap();
                violations.push(relative.display().to_string());
            }
        }
    }
}
