#![cfg(feature = "renderer-spike")]

use glorp::renderer_spike::artifacts::{
    aggregate_samples, missed_frame_percent, run_median_divergence_percent, validate_run,
    write_manifest,
};
use glorp::renderer_spike::fixture::{
    canonical_atlas, canonical_fixture, expected_frame, resolve_frame, semantic_fixture,
    DYNAMIC_PRIMITIVE_COUNT, PET_GLYPH_COUNT, SHAPE_COUNT, STATIC_GLYPH_COUNT,
};
use glorp::renderer_spike::{RendererSpikeCandidate, RendererSpikeTrack};

#[test]
fn software_candidate_has_stable_contract_name() {
    assert_eq!(RendererSpikeCandidate::Software.as_str(), "software");
    assert_eq!(
        serde_json::to_string(&RendererSpikeCandidate::Software).unwrap(),
        "\"software\""
    );
}

#[test]
fn software_candidate_requires_native_frame_evidence() {
    let required = glorp::renderer_spike::artifacts::required_artifacts(
        RendererSpikeCandidate::Software,
        RendererSpikeTrack::Ambient,
        360,
    );
    assert!(required.contains(&"host-boundary.json".to_string()));
    assert!(required.contains(&"frame-metrics.jsonl".to_string()));
}

#[test]
fn canonical_fixture_has_exact_frozen_workload() {
    let fixture = canonical_fixture();
    assert_eq!(fixture.id, "renderer-decision-companion-v1");
    assert_eq!(
        fixture.primitives.len(),
        PET_GLYPH_COUNT + STATIC_GLYPH_COUNT + SHAPE_COUNT
    );
    assert_eq!(
        fixture
            .primitives
            .iter()
            .filter(|primitive| primitive.dynamic)
            .count(),
        DYNAMIC_PRIMITIVE_COUNT
    );
    assert_eq!(
        fixture
            .primitives
            .iter()
            .map(|primitive| primitive.depth_band)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        fixture
            .primitives
            .iter()
            .map(|primitive| primitive.motion)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn dynamic_frames_change_only_declared_primitives() {
    let fixture = canonical_fixture();
    let initial = resolve_frame(&fixture, 0);
    let changed = resolve_frame(&fixture, 250);
    assert!(initial.changed_primitive_ids.is_empty());
    assert_eq!(changed.changed_primitive_ids.len(), DYNAMIC_PRIMITIVE_COUNT);
    assert!(changed
        .changed_primitive_ids
        .iter()
        .all(|id| id.0 < DYNAMIC_PRIMITIVE_COUNT as u16));
    let expected = expected_frame(&fixture, 250);
    assert_eq!(expected.expected_changes, changed.changed_primitive_ids);
}

#[test]
fn fixture_and_known_frames_serialize_deterministically() {
    let fixture = canonical_fixture();
    let first = serde_json::to_vec(&fixture).unwrap();
    let second = serde_json::to_vec(&canonical_fixture()).unwrap();
    assert_eq!(first, second);
    for elapsed in [0, 250, 1_000, 5_000] {
        assert_eq!(
            serde_json::to_vec(&resolve_frame(&fixture, elapsed)).unwrap(),
            serde_json::to_vec(&resolve_frame(&fixture, elapsed)).unwrap()
        );
    }
}

#[test]
fn atlas_covers_replacement_non_bmp_and_multi_scalar_keys() {
    let atlas = canonical_atlas();
    let keys = atlas
        .entries
        .iter()
        .map(|entry| entry.key.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(keys.contains("�"));
    assert!(keys.contains("🫧"));
    assert!(keys.contains("o\u{308}"));
    assert_eq!(
        atlas.rgba.len(),
        usize::from(atlas.width) * usize::from(atlas.height) * 4
    );
}

#[test]
fn semantic_fixture_has_one_group_and_three_values() {
    let nodes = semantic_fixture(720, false);
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0].role, "group");
    assert!(nodes.iter().all(|node| !node.hidden));
    assert_eq!(nodes.iter().filter(|node| node.value.is_some()).count(), 3);
}

#[test]
fn aggregates_known_samples() {
    let (mean, median, p95) = aggregate_samples(&[0.0, 10.0, 20.0, 30.0, 40.0]).unwrap();
    assert_eq!(mean, 20.0);
    assert_eq!(median, 20.0);
    assert_eq!(p95, 40.0);
    assert!(aggregate_samples(&[]).is_none());
    assert!(aggregate_samples(&[f64::NAN]).is_none());
}

#[test]
fn p95_uses_nearest_rank_for_small_and_boundary_samples() {
    assert_eq!(aggregate_samples(&[9.0]).unwrap().2, 9.0);
    let samples = (1..=20).map(f64::from).collect::<Vec<_>>();
    assert_eq!(aggregate_samples(&samples).unwrap().2, 19.0);
    let samples = (1..=21).map(f64::from).collect::<Vec<_>>();
    assert_eq!(aggregate_samples(&samples).unwrap().2, 20.0);
}

#[test]
fn missed_frame_percentage_uses_requested_visible_frames() {
    assert_eq!(missed_frame_percent(1_000, 9), Some(0.8999999999999999));
    assert_eq!(missed_frame_percent(0, 0), None);
    assert_eq!(missed_frame_percent(10, 20), Some(100.0));
}

#[test]
fn run_median_divergence_uses_median_as_denominator() {
    assert_eq!(run_median_divergence_percent(&[4.0, 5.0, 6.0]), Some(40.0));
    assert_eq!(run_median_divergence_percent(&[0.0, 0.0]), Some(0.0));
    assert_eq!(run_median_divergence_percent(&[0.0, 0.0, 1.0]), None);
    assert_eq!(run_median_divergence_percent(&[]), None);
}

#[test]
fn manifest_detects_changed_artifact() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "environment.json",
        "binary.json",
        "fixture.json",
        "atlas.json",
        "accessibility-tree.json",
        "privacy-scan.json",
        "process-cleanup.json",
        "summary.json",
    ] {
        std::fs::write(dir.path().join(name), "{}").unwrap();
    }
    write_manifest(
        dir.path(),
        glorp::renderer_spike::RendererSpikeCandidate::Smooth,
        glorp::renderer_spike::RendererSpikeTrack::Static,
        360,
    )
    .unwrap();
    validate_run(dir.path()).unwrap();
    std::fs::write(dir.path().join("fixture.json"), "changed").unwrap();
    assert!(validate_run(dir.path()).is_err());
}

#[test]
fn manifest_rejects_duplicate_and_escaping_paths() {
    for path in ["fixture.json", "../fixture.json"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fixture.json"), "fixture").unwrap();
        let bytes = std::fs::read(dir.path().join("fixture.json")).unwrap();
        let artifact = serde_json::json!({
            "path": path,
            "bytes": bytes.len(),
            "sha256": glorp::renderer_spike::artifacts::sha256_hex(&bytes),
        });
        let artifacts = if path == "fixture.json" {
            vec![artifact.clone(), artifact]
        } else {
            vec![artifact]
        };
        std::fs::write(
            dir.path().join("run-manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "run_id": "unsafe",
                "required": [],
                "artifacts": artifacts,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(validate_run(dir.path()).is_err());
    }
}

#[test]
fn startup_artifact_requires_monotonic_checkpoints() {
    let startup = glorp::renderer_spike::artifacts::StartupArtifact::from_checkpoints(
        100,
        120,
        200,
        Some(260),
    )
    .unwrap();
    assert_eq!(startup.runner_to_harness_micros, 20);
    assert_eq!(startup.runner_to_host_ready_micros, 100);
    assert_eq!(startup.runner_to_first_present_micros, Some(160));
    assert_eq!(startup.host_ready_to_first_present_micros, Some(60));
    assert!(
        glorp::renderer_spike::artifacts::StartupArtifact::from_checkpoints(
            120,
            100,
            200,
            Some(260),
        )
        .is_err()
    );
    assert!(
        glorp::renderer_spike::artifacts::StartupArtifact::from_checkpoints(
            100,
            120,
            200,
            Some(190),
        )
        .is_err()
    );
}

#[test]
fn privacy_scanner_rejects_seeded_secret() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bad.json"),
        r#"{"value":"very-secret-seed"}"#,
    )
    .unwrap();
    let scan = glorp::renderer_spike::privacy::scan_owned_directory(dir.path()).unwrap();
    assert!(!scan.passed);
    assert!(scan
        .rejected_tokens
        .contains(&"very-secret-seed".to_string()));
}
