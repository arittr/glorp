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

const FORBIDDEN_ADAPTER_DEPENDENCIES: &[&str] = &["WatchViewModel"];

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
fn compiled_generation_provenance_is_not_publicly_mutable() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presentation/companion_scene/scene.rs");
    let source = fs::read_to_string(path).expect("read scene contract source");
    let generation = source
        .split("pub struct SceneGenerationData")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) struct SceneDeltaScratch").next())
        .expect("SceneGenerationData declaration");
    for forbidden in [
        "pub generation_key:",
        "pub source_revisions:",
        "pub source_snapshot:",
        "pub template:",
        "pub content:",
        "pub frame:",
        "pub content_checksum:",
        "pub frame_checksum:",
    ] {
        assert!(
            !generation.contains(forbidden),
            "compiled generation exposes mutable provenance field {forbidden}"
        );
    }
}

#[test]
fn neutral_capture_identity_is_closed_and_contains_no_private_or_renderer_evidence() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presentation/companion_scene/contract.rs");
    let source = fs::read_to_string(path).expect("read capture contract source");
    let identity = source
        .split("pub struct CaptureSourceIdentity")
        .nth(1)
        .and_then(|tail| tail.split("impl CaptureSourceIdentity").next())
        .expect("CaptureSourceIdentity declaration");
    let normalized_identity = normalize_boundary_text(identity);
    for required in ["template_checksum", "content_checksum", "frame_checksum"] {
        assert!(
            normalized_identity.contains(&normalize_boundary_text(required)),
            "capture source identity is missing {required}"
        );
    }
    for forbidden in [
        "Smooth",
        "plan",
        "draw_order",
        "wall_clock",
        "frame_id",
        "renderer_generation",
        "source_name",
        "project_name",
        "path",
        "diagnostic",
        "prompt",
        "response",
        "token",
        "user_text",
        "String",
        "Vec",
    ] {
        assert!(
            !normalized_identity.contains(&normalize_boundary_text(forbidden)),
            "capture source identity contains forbidden evidence {forbidden}"
        );
    }

    let request = source
        .split("pub struct CompanionCaptureRequest")
        .nth(1)
        .and_then(|tail| tail.split("impl CompanionCaptureRequest").next())
        .expect("CompanionCaptureRequest declaration");
    let normalized_request = normalize_boundary_text(request);
    for required in [
        "SceneVersion",
        "CaptureSourceIdentity",
        "CompanionCaptureStateAlias",
        "PrivacyProjection",
        "CaptureSurfaceArtifact",
        "CanonicalReadbackRequest",
        "CompanionSceneMetricsArtifact",
    ] {
        assert!(
            normalized_request.contains(&normalize_boundary_text(required)),
            "capture request is missing typed field {required}"
        );
    }
    for forbidden in ["String", "Path", "SystemTime", "Instant", "Smooth", "wgpu"] {
        assert!(
            !normalized_request.contains(&normalize_boundary_text(forbidden)),
            "capture request contains forbidden type {forbidden}"
        );
    }

    let metrics = source
        .split("pub struct CompanionSceneMetricsArtifact")
        .nth(1)
        .and_then(|tail| tail.split("impl CompanionSceneMetricsArtifact").next())
        .expect("CompanionSceneMetricsArtifact declaration");
    let normalized_metrics = normalize_boundary_text(metrics);
    for required in [
        "node_high_water",
        "primitive_high_water",
        "blended_draw_high_water",
        "persistent_gpu_objects_created",
        "static_upload_bytes",
        "content_write_bytes",
        "frame_write_bytes",
    ] {
        assert!(
            normalized_metrics.contains(&normalize_boundary_text(required)),
            "capture metrics are missing {required}"
        );
    }
    for forbidden in [
        "String",
        "Path",
        "source_name",
        "project_name",
        "user_text",
        "prompt",
        "response",
        "diagnostic",
        "token",
    ] {
        assert!(
            !normalized_metrics.contains(&normalize_boundary_text(forbidden)),
            "capture metrics contain forbidden evidence {forbidden}"
        );
    }

    let alpha = source
        .split("pub enum CaptureCompositeAlpha")
        .nth(1)
        .and_then(|tail| tail.split("pub enum CanonicalReadbackFormat").next())
        .expect("CaptureCompositeAlpha declaration");
    let normalized_alpha = normalize_boundary_text(alpha);
    assert!(normalized_alpha.contains(&normalize_boundary_text("PostMultiplied")));
    assert!(!normalized_alpha.contains(&normalize_boundary_text("Premultiplied")));

    let aliases = source
        .split("pub enum CompanionCaptureStateAlias")
        .nth(1)
        .and_then(|tail| tail.split("impl CompanionCaptureStateAlias").next())
        .expect("CompanionCaptureStateAlias declaration");
    assert!(
        !normalize_boundary_text(aliases).contains(&normalize_boundary_text("Fault")),
        "capture fault must be a sanitized outcome, not a logical state"
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
    let tokens = boundary_tokens(source);
    if relative_path == Path::new("input.rs") {
        for index in tui_path_segment_indices(&tokens) {
            match canonical_input_tui_module(&tokens, index) {
                Some(module) if ALLOWED_INPUT_TUI_MODULES.contains(&module) => {}
                Some(module) => violations.push(format!("crate::tui::{module}")),
                None => violations.push("non-canonical tui root".to_owned()),
            }
        }
    } else {
        violations.extend(
            tui_path_segment_indices(&tokens)
                .into_iter()
                .map(|_| "tui module boundary".to_owned()),
        );
        let normalized = normalize_boundary_text(source);
        for forbidden in FORBIDDEN_ADAPTER_DEPENDENCIES {
            if normalized.contains(&normalize_boundary_text(forbidden)) {
                violations.push((*forbidden).to_owned());
            }
        }
    }
    violations
}

fn tui_path_segment_indices(tokens: &[BoundaryToken]) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            if !matches!(token, BoundaryToken::Ident(identifier) if identifier == "tui") {
                return None;
            }
            let previous = index
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous));
            let follows_path_syntax = matches!(
                previous,
                Some(
                    BoundaryToken::PathSeparator
                        | BoundaryToken::RawIdentifier
                        | BoundaryToken::OpenBrace
                        | BoundaryToken::Comma
                )
            ) || matches!(
                previous,
                Some(BoundaryToken::Ident(identifier))
                    if matches!(identifier.as_str(), "crate" | "mod" | "use")
            );
            let precedes_path_syntax =
                matches!(tokens.get(index + 1), Some(BoundaryToken::PathSeparator));
            (follows_path_syntax || precedes_path_syntax).then_some(index)
        })
        .collect()
}

fn canonical_input_tui_module(tokens: &[BoundaryToken], tui_index: usize) -> Option<&str> {
    let crate_index = tui_index.checked_sub(2)?;
    if !matches!(
        (tokens.get(crate_index), tokens.get(tui_index - 1)),
        (
            Some(BoundaryToken::Ident(root)),
            Some(BoundaryToken::PathSeparator)
        ) if root == "crate"
    ) || (crate_index > 0
        && matches!(
            tokens.get(crate_index - 1),
            Some(BoundaryToken::PathSeparator | BoundaryToken::RawIdentifier)
        ))
    {
        return None;
    }

    match (tokens.get(tui_index + 1), tokens.get(tui_index + 2)) {
        (Some(BoundaryToken::PathSeparator), Some(BoundaryToken::Ident(module))) => Some(module),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BoundaryToken {
    Ident(String),
    RawIdentifier,
    PathSeparator,
    OpenBrace,
    CloseBrace,
    Comma,
}

fn boundary_tokens(source: &str) -> Vec<BoundaryToken> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        let raw_identifier = character == 'r' && chars.peek() == Some(&'#') && {
            let mut lookahead = chars.clone();
            lookahead.next();
            lookahead
                .peek()
                .is_some_and(|next| next.is_ascii_alphabetic() || *next == '_')
        };
        if raw_identifier {
            chars.next();
            let first = chars.next().expect("peeked raw identifier character");
            let mut identifier = String::from(first.to_ascii_lowercase());
            while let Some(next) = chars.peek() {
                if !next.is_ascii_alphanumeric() && *next != '_' {
                    break;
                }
                identifier.push(
                    chars
                        .next()
                        .expect("peeked raw identifier character")
                        .to_ascii_lowercase(),
                );
            }
            tokens.push(BoundaryToken::RawIdentifier);
            tokens.push(BoundaryToken::Ident(identifier));
        } else if character.is_ascii_alphanumeric() || character == '_' {
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
        } else if character == '{' {
            tokens.push(BoundaryToken::OpenBrace);
        } else if character == '}' {
            tokens.push(BoundaryToken::CloseBrace);
        } else if character == ',' {
            tokens.push(BoundaryToken::Comma);
        }
    }
    tokens
}

fn normalize_boundary_text(source: &str) -> String {
    normalize_rust_raw_identifiers(source)
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn normalize_rust_raw_identifiers(source: &str) -> String {
    let mut normalized = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        let raw_identifier = character == 'r' && chars.peek() == Some(&'#') && {
            let mut lookahead = chars.clone();
            lookahead.next();
            lookahead
                .peek()
                .is_some_and(|next| next.is_ascii_alphabetic() || *next == '_')
        };
        if raw_identifier {
            chars.next();
        } else {
            normalized.push(character);
        }
    }
    normalized
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
        "use crate::{room as neutral, tui as adapter};",
    ] {
        assert!(
            !companion_source_violations(Path::new("input.rs"), source).is_empty(),
            "input adapter accepted grouped TUI import: {source}"
        );
    }
}

#[test]
fn companion_scene_tui_boundary_rejects_raw_and_relative_path_bypasses() {
    for source in [
        "use crate::r#tui::room::RoomLifeProfile;",
        "use crate::{r#tui as adapter};",
        "use super::super::tui::room::RoomLifeProfile;",
        "use self::tui::view_model::WatchViewModel;",
    ] {
        assert!(
            !companion_source_violations(Path::new("scene.rs"), source).is_empty(),
            "non-input companion source accepted TUI path bypass: {source}"
        );
        assert!(
            !companion_source_violations(Path::new("input.rs"), source).is_empty(),
            "input adapter accepted non-canonical TUI path: {source}"
        );
    }

    assert!(
        !companion_source_violations(
            Path::new("input.rs"),
            "use ::crate::tui::room::RoomLifeProfile;",
        )
        .is_empty(),
        "input adapter accepted a leading-separator crate root"
    );
}

#[test]
fn input_rs_accepts_only_direct_canonical_tui_roots() {
    for source in [
        "use crate::tui::view_model::WatchViewModel;",
        "use crate::tui::room::RoomLifeProfile;",
        "use crate::tui::wander::resolve_wander_offset;",
        "use crate::tui::life::PetLifeProfile;",
    ] {
        assert!(
            companion_source_violations(Path::new("input.rs"), source).is_empty(),
            "input adapter rejected canonical TUI root: {source}"
        );
    }
}
