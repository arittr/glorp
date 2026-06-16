use std::path::Path;

#[test]
fn pet_panel_keeps_root_file_and_child_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let panel_root = root.join("src/tui/panels/pet.rs");
    assert!(panel_root.is_file(), "pet.rs must remain the module root");
    assert!(
        !root.join("src/tui/panels/pet/mod.rs").exists(),
        "do not move the root to pet/mod.rs"
    );

    let source = std::fs::read_to_string(&panel_root).unwrap();
    for module in ["ambient", "colors", "art_lines", "performance", "props"] {
        assert!(
            source.contains(&format!("mod {module};")),
            "pet.rs should declare child module {module}"
        );
        assert!(
            root.join(format!("src/tui/panels/pet/{module}.rs"))
                .is_file(),
            "missing child module file {module}.rs"
        );
    }

    let root_lines = source.lines().count();
    assert!(
        root_lines < 1700,
        "pet.rs should shrink after mechanical split; got {root_lines} lines"
    );
}
