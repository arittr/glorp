use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_RENDERER_IDENTIFIERS: &[&str] = &[
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

const FORBIDDEN_ADAPTER_DEPENDENCIES: &[&str] = &["WatchViewModel", "crate::tui"];

const ALLOWED_INPUT_TUI_MODULES: &[&str] = &["life", "room", "view_model", "wander"];

const FORBIDDEN_IMPORT_ROOTS: &[&str] = &[
    "crate::presentation::smooth",
    "crate::presentation::rasterize",
    "crate::presentation::draw_list",
    "crate::round::draw",
    "crate::tui::component",
    "crate::tui::panels",
];

const FORBIDDEN_NEUTRAL_DEPENDENCIES: &[&str] = &[
    "WatchViewModel",
    "crate::tui",
    "Smooth",
    "ratatui",
    "wgpu",
    "objc2",
    "AppKit",
];

const NEUTRAL_DEPENDENCY_MANIFEST: &[(&str, &str)] = &[
    ("src/round/motion.rs", "crate::round::motion"),
    (
        "src/presentation/habitat_inventory.rs",
        "crate::presentation::habitat_inventory",
    ),
    (
        "src/presentation/tank_life.rs",
        "crate::presentation::tank_life",
    ),
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
        let relative = file
            .strip_prefix(&root)
            .expect("companion source below root");
        for violation in companion_source_violations(relative, &source) {
            violations.push(format!("{} contains {violation}", file.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "renderer-neutral boundary violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn companion_scene_neutral_dependency_closure_has_no_view_or_renderer_ownership() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    for (relative_path, _) in NEUTRAL_DEPENDENCY_MANIFEST {
        let path = root.join(relative_path);
        assert!(
            path.is_file(),
            "neutral dependency manifest file is missing: {relative_path}"
        );
        let source = fs::read_to_string(&path).expect("read neutral dependency source");
        for forbidden in neutral_dependency_violations(&source) {
            violations.push(format!("{} contains {forbidden}", path.display()));
        }
    }
    assert!(
        violations.is_empty(),
        "neutral dependency closure violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn companion_scene_neutral_imports_are_listed_in_the_enforced_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presentation/companion_scene");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    let combined = files
        .iter()
        .map(|path| fs::read_to_string(path).expect("read companion scene source"))
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = normalize_boundary_text(&combined);

    for (_, module_path) in NEUTRAL_DEPENDENCY_MANIFEST {
        assert!(
            normalized.contains(&normalize_boundary_text(module_path)),
            "neutral companion dependency is not referenced: {module_path}"
        );
    }
}

fn neutral_dependency_violations(source: &str) -> Vec<&'static str> {
    let normalized = normalize_boundary_text(source);
    FORBIDDEN_NEUTRAL_DEPENDENCIES
        .iter()
        .copied()
        .filter(|forbidden| normalized.contains(&normalize_boundary_text(forbidden)))
        .collect()
}

fn boundary_violations(source: &str) -> Vec<&'static str> {
    let normalized_source = normalize_boundary_text(source);
    let mut violations = Vec::new();
    for forbidden in FORBIDDEN_RENDERER_IDENTIFIERS {
        let normalized_forbidden = normalize_boundary_text(forbidden);
        if normalized_source.contains(&normalized_forbidden) {
            violations.push(*forbidden);
        }
    }
    for forbidden in FORBIDDEN_IMPORT_ROOTS {
        let normalized_forbidden = normalize_boundary_text(forbidden);
        if normalized_source.contains(&normalized_forbidden) {
            violations.push(*forbidden);
        }
    }
    violations
}

fn companion_source_violations(relative_path: &Path, source: &str) -> Vec<String> {
    let mut violations = boundary_violations(source)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if relative_path == Path::new("input.rs") {
        for module in referenced_tui_modules(source) {
            if !ALLOWED_INPUT_TUI_MODULES.contains(&module.as_str()) {
                violations.push(format!("crate::tui::{module}"));
            }
        }
    } else {
        let normalized = normalize_boundary_text(source);
        for forbidden in FORBIDDEN_ADAPTER_DEPENDENCIES {
            if normalized.contains(&normalize_boundary_text(forbidden)) {
                violations.push((*forbidden).to_owned());
            }
        }
    }
    violations
}

fn referenced_tui_modules(source: &str) -> Vec<String> {
    let tokens = boundary_tokens(source);
    let mut modules = Vec::new();
    for index in 0..tokens.len() {
        if !matches!(tokens.get(index), Some(BoundaryToken::Ident(root)) if root == "crate")
            || !matches!(tokens.get(index + 1), Some(BoundaryToken::PathSeparator))
        {
            continue;
        }
        match tokens.get(index + 2) {
            Some(BoundaryToken::Ident(root)) if root == "tui" => {
                let module = match (tokens.get(index + 3), tokens.get(index + 4)) {
                    (Some(BoundaryToken::PathSeparator), Some(BoundaryToken::Ident(module))) => {
                        module.clone()
                    }
                    _ => "<root>".to_owned(),
                };
                modules.push(module);
            }
            Some(BoundaryToken::OpenBrace)
                if grouped_use_tree_contains_tui_root(&tokens, index + 2) =>
            {
                modules.push("<grouped-root>".to_owned());
            }
            _ => {}
        }
    }
    modules
}

#[derive(Debug, PartialEq, Eq)]
enum BoundaryToken {
    Ident(String),
    PathSeparator,
    OpenBrace,
    CloseBrace,
    Comma,
}

fn boundary_tokens(source: &str) -> Vec<BoundaryToken> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if character.is_ascii_alphanumeric() || character == '_' {
            let mut identifier = String::from(character.to_ascii_lowercase());
            while let Some(next) = chars.peek() {
                if !next.is_ascii_alphanumeric() && *next != '_' {
                    break;
                }
                identifier.push(
                    chars
                        .next()
                        .expect("peeked identifier character")
                        .to_ascii_lowercase(),
                );
            }
            tokens.push(BoundaryToken::Ident(identifier));
        } else if character == ':' && chars.peek() == Some(&':') {
            chars.next();
            tokens.push(BoundaryToken::PathSeparator);
        } else {
            match character {
                '{' => tokens.push(BoundaryToken::OpenBrace),
                '}' => tokens.push(BoundaryToken::CloseBrace),
                ',' => tokens.push(BoundaryToken::Comma),
                _ => {}
            }
        }
    }
    tokens
}

fn grouped_use_tree_contains_tui_root(tokens: &[BoundaryToken], open_brace: usize) -> bool {
    let mut depth = 0usize;
    let mut at_item_start = true;
    let mut index = open_brace + 1;
    while let Some(token) = tokens.get(index) {
        match token {
            BoundaryToken::OpenBrace => depth += 1,
            BoundaryToken::CloseBrace if depth == 0 => return false,
            BoundaryToken::CloseBrace => depth -= 1,
            BoundaryToken::Comma if depth == 0 => at_item_start = true,
            BoundaryToken::Ident(identifier) if depth == 0 && at_item_start => {
                if identifier == "tui" {
                    return true;
                }
                at_item_start = false;
            }
            BoundaryToken::PathSeparator if depth == 0 && at_item_start => {}
            _ if depth == 0 && at_item_start => at_item_start = false,
            _ => {}
        }
        index += 1;
    }
    false
}

fn normalize_boundary_text(source: &str) -> String {
    source
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
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

#[test]
fn boundary_scan_normalizes_case_and_identifier_separators() {
    for source in [
        "use RAW_WINDOW_HANDLE::HasWindowHandle;",
        "use raw-window-handle::HasWindowHandle;",
        "type Layer = CAMetal_Layer;",
    ] {
        assert!(
            !boundary_violations(source).is_empty(),
            "separator/case bypass was accepted: {source}"
        );
    }
}

#[test]
fn boundary_scan_rejects_aliased_renderer_and_terminal_painter_imports() {
    for source in [
        "use crate::presentation::smooth as neutral;",
        "use crate::presentation::{smooth as scene_math};",
        "use crate::tui::panels::pet as domain_pet;",
        "use crate::tui::component::habitat_props as inventory;",
        "use crate::round::draw as semantic_round;",
        "use crate::presentation::rasterize as projection;",
    ] {
        assert!(
            !boundary_violations(source).is_empty(),
            "aliased renderer import was accepted: {source}"
        );
    }
}

#[test]
fn neutral_dependency_scan_rejects_alias_case_and_separator_bypasses() {
    for source in [
        "use crate :: tui :: view_model :: Watch_View_Model as Input;",
        "use crate::presentation::Smo_oth as neutral;",
        "use RaTaTuI as geometry;",
        "use ObJc_2 as domain;",
    ] {
        assert!(
            !neutral_dependency_violations(source).is_empty(),
            "neutral dependency bypass was accepted: {source}"
        );
    }
}

#[test]
fn only_input_rs_may_use_the_tui_adapter_boundary() {
    for source in [
        "use crate::tui::view_model::WatchViewModel;",
        "use crate::tui::room::RoomLifeProfile;",
        "use crate::tui::wander::resolve_wander_offset;",
        "use crate::tui::life::PetLifeProfile;",
    ] {
        assert!(
            !companion_source_violations(Path::new("scene.rs"), source).is_empty(),
            "non-input companion source accepted adapter import: {source}"
        );
        assert!(
            companion_source_violations(Path::new("input.rs"), source).is_empty(),
            "input adapter rejected its precise import: {source}"
        );
    }

    assert!(!companion_source_violations(
        Path::new("input.rs"),
        "use crate::tui::component::TankLifeSurfaceGeometry;",
    )
    .is_empty());
    assert!(
        !companion_source_violations(Path::new("input.rs"), "use crate::tui as adapter;",)
            .is_empty()
    );
}

#[test]
fn input_rs_rejects_grouped_tui_root_import_bypasses() {
    for source in [
        "use crate::{tui as adapter};",
        "use crate::{tui};",
        "use crate::{tui::{view_model::WatchViewModel}};",
        "use crate::{tui::{view_model::WatchViewModel as Input}};",
        "use crate::{tui::{self as adapter}};",
        "use crate::{tui::{view_model::{WatchViewModel as Input}}};",
    ] {
        assert!(
            !companion_source_violations(Path::new("input.rs"), source).is_empty(),
            "input adapter accepted grouped TUI import: {source}"
        );
    }
}
