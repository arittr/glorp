use glorp::game::evolution::Stage;
use glorp::game::metabolism::Mood;
use glorp::pet::generation::Species;
use glorp::round::model::{
    derive_round_scene_model, RoundActivityPulse, RoundHelperHealth, RoundPetModel,
    RoundSourceDiversity, RoundVitalBucket,
};
use glorp::storage::state::HabitatPropId;
use glorp::tui::identity::SourceDiversity;
use glorp::tui::view_model::{EventView, SourceStatus, WatchViewModel};
use time::macros::datetime;

#[test]
fn round_scene_excludes_watch_dashboard_and_private_fields() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.helper_status = "provider poll failed: /Users/drew/private/project".into();
    vm.errors = vec!["secret prompt response tool payload /tmp/private.rs".into()];
    vm.recent_events = vec![EventView {
        timestamp: "13:40".into(),
        kind: glorp::tui::style::LogKind::Usage,
        text: "opened /Users/drew/private/project/main.rs".into(),
    }];
    vm.source_breakdown[0].display_name = "client-secret-project".into();
    vm.today_effective_tokens = 123_456.0;
    vm.progress.rate_per_hour = 99_999.0;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));

    // Direct backstops: prove sensitive WatchViewModel fields were not copied.
    assert!(
        matches!(
            scene.halo.vitals.fed,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        matches!(
            scene.halo.vitals.happiness,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        matches!(
            scene.halo.vitals.energy,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    let _: &[HabitatPropId] = &scene.room.prop_landmarks;
    assert!(
        scene.moments.is_empty(),
        "private data leaked into RoundSceneModel"
    );
    assert_eq!(
        scene.pet,
        RoundPetModel {
            seed: "fixture-seed".into(),
            species: Species::Fuzz,
            stage: Stage::S0,
            mood: Mood::Content,
            art_lines: vm.pet_art.clone(),
            art_spans: vm.pet_spans.clone(),
            asleep: false,
            breath_offset_y: 0,
            facing: 1,
        },
        "private data leaked into RoundSceneModel"
    );

    let debug = format!("{scene:?}");

    assert!(
        !debug.contains("secret"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("/Users/drew"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("prompt"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("response"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("tool payload"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("123456"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("99999"),
        "private data leaked into RoundSceneModel"
    );
}

#[test]
fn round_scene_maps_required_v1_signals() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.activity_identity.source_diversity = SourceDiversity::DualLane;
    vm.last_feed_pulse_at = Some(datetime!(2026-06-13 17:59:59 UTC));
    let codex = vm
        .source_health
        .iter_mut()
        .find(|s| s.name == "codex")
        .expect("codex source health entry");
    codex.status = SourceStatus::Diagnostic;
    codex.diagnostic_code = Some("missing_helper".into());
    codex.diagnostic_message = Some("private helper path".into());

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));

    assert_eq!(scene.pet.seed, "fixture-seed", "expected fixture seed");
    assert_eq!(
        scene.pet.species, vm.pet_render.generated_species,
        "expected pet species"
    );
    assert_eq!(scene.pet.stage, vm.pet_render.stage, "expected pet stage");
    assert_eq!(
        scene.room.prop_landmarks.len(),
        2,
        "expected two prop landmarks"
    );
    assert_eq!(
        scene.halo.source_diversity,
        RoundSourceDiversity::Dual,
        "expected Dual source diversity"
    );
    assert_eq!(
        scene.halo.helper_health,
        RoundHelperHealth::Trouble,
        "expected helper health trouble"
    );
    assert!(
        matches!(scene.halo.activity_pulse, RoundActivityPulse::Recent { .. }),
        "expected Recent activity pulse"
    );
}

#[test]
fn round_scene_uses_night_calm_for_asleep_state() {
    let mut vm = WatchViewModel::fixture();
    vm.day_context.asleep = true;
    vm.life_profile.calm_mode = true;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 08:00 UTC));

    assert!(scene.lifecycle.asleep);
    assert!(scene.lifecycle.calm);
    assert!(scene.halo.activity_pulse.is_quiet());
}

/// The round companion renders `WatchViewModel.pet_art` into a circular
/// aperture; a Heavy-tier Glitch S6 day earns up to three persistent repair
/// marks (one-cell `Pattern`/`Accent` spans carrying a `+ = : .` glyph — see
/// `glitch_repair_marks_use_soft_roles_not_corruption` in `src/pet/render.rs`
/// for the same identification method). This proves at least one of those
/// marks actually lands on a visible (non-masked) cell once the pet art is
/// positioned in the round scene's grid, not clipped by the circle.
#[cfg(feature = "dev-preview")]
#[test]
fn round_glitch_s6_keeps_a_repair_mark_inside_aperture() {
    use glorp::pet::render::PaletteRoleName;
    use glorp::round::layout::RoundAperture;
    use glorp::round::preview::render_round_preview_frame_from_vm;
    use glorp::round::scene::{build_round_scene_draw_list, CompanionMotion};

    const WIDTH: u16 = 52;
    const HEIGHT: u16 = 52;
    let now = datetime!(2026-06-13 18:00 UTC);

    // Mirror the `round-glitch-patched-s6` preview fixture
    // (src/dev_preview/round.rs): a calm S6 Glitch on a Heavy-tier day
    // (today_ratio 1.7, date_seed 42) earns 3 repair marks.
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.pet_render.seed = "glorp-preview-glitch-persistence".to_string();
    vm.pet_render.generated_species = Species::Glitch;
    vm.pet_render.stage = Stage::S6;
    vm.day_context.date_seed = 42;
    vm.day_context.today_ratio = 1.7;
    vm.life_profile.burst_level = 0.0;
    vm.life_profile.calm_mode = true;
    glorp::commands::watch::rerender_pet_for_view_model(
        &mut vm,
        now.unix_timestamp().max(0) as u64,
        false,
        now,
    )
    .expect("fixture should rerender");

    // Find a declared repair mark: a one-cell Pattern/Accent span carrying one
    // of the soldered repair glyphs.
    let repair_span = vm
        .pet_spans
        .iter()
        .find(|span| {
            matches!(
                span.role,
                PaletteRoleName::Pattern | PaletteRoleName::Accent
            ) && span.end == span.start + 1
        })
        .expect("Heavy-tier Glitch S6 fixture should carry at least one repair mark span");
    let mark_row = repair_span.line;
    let mark_col = repair_span.start;
    let mark_glyph = vm.pet_art[mark_row]
        .chars()
        .nth(mark_col)
        .expect("repair mark column should be in-bounds");
    assert!(
        "+=:.".contains(mark_glyph),
        "repair mark glyph should be one of the soldered repair glyphs, got {mark_glyph:?}"
    );

    // Map the mark's art-local (row, col) to the round scene's absolute grid
    // position for this vm/now, using the same seam and default companion
    // motion the round-glitch-patched-s6 preview renders with. The seam may
    // horizontally mirror the art (facing left) before placing it at
    // `pet_rect`, which — per `mirror_spans` in
    // `src/tui/panels/pet/art_lines.rs` — maps a one-cell span's column `c` to
    // `line_len - 1 - c` (none of the repair glyphs are direction-swapped by
    // `mirror_char`, so the glyph itself is unaffected). Check both the
    // unmirrored and mirrored column so the assertion holds regardless of
    // which way the pet is facing at this deterministic `now`.
    let companion_scene =
        build_round_scene_draw_list(&vm, now, WIDTH, HEIGHT, &CompanionMotion::default());
    let line_len = vm.pet_art[mark_row].chars().count();
    let mirrored_col = line_len - 1 - mark_col;
    let candidate_cols = [mark_col, mirrored_col];

    let aperture = RoundAperture::new(WIDTH, HEIGHT);
    let frame = render_round_preview_frame_from_vm(
        "round-glitch-s6-repair-test",
        "Round Glitch S6 Repair Test",
        &vm,
        now,
        WIDTH,
        HEIGHT,
        glorp::round::layout::RoundRenderCapabilities::preview_truecolor(),
    );

    let visible_match = candidate_cols.iter().any(|&col| {
        let abs_x = companion_scene.pet_rect.x + col as u16;
        let abs_y = companion_scene.pet_rect.y + mark_row as u16;
        if !aperture.contains(abs_x as f32, abs_y as f32) {
            return false;
        }
        frame.cells.iter().any(|c| {
            c.x == abs_x
                && c.y == abs_y
                && !c.outside_aperture
                && c.symbol == mark_glyph.to_string()
        })
    });

    assert!(
        visible_match,
        "expected the repair mark glyph {mark_glyph:?} to land on a visible \
         (non-masked) cell at row {mark_row}, cols {candidate_cols:?} \
         (pet_rect={:?}); it was clipped or overwritten",
        companion_scene.pet_rect
    );
}
