use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "ratatui",
    "drawcell",
    "scenedrawlist",
    "smoothcompanion",
    "wgpu",
    "objc2",
    "nsview",
    "nswindow",
    "cametallayer",
    "appkit",
    "rawwindowhandle",
    "windowhandle",
    "surfaceconfiguration",
    "surfacetexture",
];

#[test]
fn companion_scene_tree_is_renderer_and_host_neutral() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presentation/companion_scene");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    for required in ["mod.rs", "input.rs"] {
        assert!(root.join(required).is_file(), "missing {required}");
    }

    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("read companion scene source");
        let normalized = source.to_ascii_lowercase();
        for forbidden in FORBIDDEN {
            if normalized.contains(forbidden) {
                violations.push(format!("{} contains {forbidden}", file.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "renderer-neutral boundary violations:\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .expect("read companion scene directory")
        .map(|entry| entry.expect("read directory entry"))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
