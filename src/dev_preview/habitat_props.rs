use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::dev_preview::watch::{render_watch_frame_from_state, WatchFrameFixture};
use crate::error::Result;
use crate::game::evolution::Stage;
use crate::game::habitat::{
    HabitatPropKind, HabitatPropSpec, CODEX_SIGNAL_LAMP, HABITAT_PROP_CATALOG,
    HEAVY_SESSION_PLANTER, WILT_RECOVERY_SPROUT,
};
use crate::pet::generation::Species;
use crate::storage::state::{
    EarnedHabitatProp, HabitatPropId, HabitatPropSource, NarrativeEvent, PetState, Vitals,
};
use crate::tui::component::{habitat_props_for, ComponentPath, PetSceneLayout, TargetPath};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::semantic_styles;
use crate::tui::view_model::{EarnedHabitatPropView, HabitatView};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use std::collections::BTreeMap;
use std::path::Path;
use time::{Duration, OffsetDateTime};

const CATALOG_FRAME_WIDTH: u16 = 120;
const CATALOG_COLUMNS: u16 = 3;
const CATALOG_CELL_WIDTH: u16 = CATALOG_FRAME_WIDTH / CATALOG_COLUMNS;
const CATALOG_CELL_HEIGHT: u16 = 10;
const WATCH_WIDTH: u16 = 120;
const WATCH_HEIGHT: u16 = 32;

pub fn habitat_prop_frames(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
) -> Result<Vec<PreviewFrame>> {
    std::fs::create_dir_all(scratch_dir)?;

    let mut frames = vec![render_habitat_props_catalog(ctx)];
    for variant in watch_variants(ctx) {
        frames.push(render_watch_frame_from_state(
            ctx,
            scratch_dir,
            WatchFrameFixture {
                id: variant.id,
                title: variant.title,
                width: WATCH_WIDTH,
                height: WATCH_HEIGHT,
                state: &variant.state,
                now: variant.now,
            },
        )?);
    }

    Ok(frames)
}

pub(crate) fn habitat_prop_ids() -> Vec<&'static str> {
    HABITAT_PROP_CATALOG.iter().map(|prop| prop.id).collect()
}

pub(crate) fn habitat_prop_motion_phase_timestamps(ctx: &PreviewRenderContext) -> Vec<i64> {
    motion_phase_times(ctx)
        .into_iter()
        .map(OffsetDateTime::unix_timestamp)
        .collect()
}

fn render_habitat_props_catalog(ctx: &PreviewRenderContext) -> PreviewFrame {
    let mut buffer = Buffer::empty(Rect::new(0, 0, CATALOG_FRAME_WIDTH, catalog_frame_height()));

    for (index, prop) in HABITAT_PROP_CATALOG.iter().enumerate() {
        let column = index as u16 % CATALOG_COLUMNS;
        let row = index as u16 / CATALOG_COLUMNS;
        let area = Rect::new(
            column * CATALOG_CELL_WIDTH,
            row * CATALOG_CELL_HEIGHT,
            CATALOG_CELL_WIDTH,
            CATALOG_CELL_HEIGHT,
        );
        render_prop_cell(area, &mut buffer, prop, ctx);
    }

    frame_from_buffer("habitat-props-catalog", "Habitat Props Catalog", &buffer)
}

fn catalog_frame_height() -> u16 {
    let rows = (HABITAT_PROP_CATALOG.len() as u16).div_ceil(CATALOG_COLUMNS);
    rows * CATALOG_CELL_HEIGHT
}

fn render_prop_cell(
    area: Rect,
    buffer: &mut Buffer,
    prop: &HabitatPropSpec,
    ctx: &PreviewRenderContext,
) {
    let styles = semantic_styles();
    let kind_label = match prop.kind {
        HabitatPropKind::Trophy => "trophy",
        HabitatPropKind::Accent => "accent",
    };
    let title = Line::from(vec![
        Span::styled(prop.id, styles.primary_text),
        Span::raw(" "),
        Span::styled(kind_label, styles.label),
    ]);
    Paragraph::new(title).render(Rect::new(area.x, area.y, area.width, 1), buffer);

    for (index, now) in motion_phase_times(ctx).into_iter().enumerate() {
        let tank = Rect::new(
            area.x + 1 + index as u16 * 13,
            area.y + 2,
            12,
            area.height.saturating_sub(3),
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(styles.overlay_border);
        let inner = block.inner(tank);
        block.render(tank, buffer);
        render_single_prop(prop, inner, buffer, ctx, now);
    }
}

fn render_single_prop(
    prop: &HabitatPropSpec,
    habitat: Rect,
    buffer: &mut Buffer,
    ctx: &PreviewRenderContext,
    now: OffsetDateTime,
) {
    let habitat_view = HabitatView {
        earned_props: vec![earned_view(prop, now)],
    };
    let render_ctx = RenderContext::with_clock(ctx.render.color_capability, WatchClock::fixed(now));
    let scene = PetSceneLayout {
        id: ComponentPath::new("watch.pet"),
        panel: habitat,
        speech: None,
        content: habitat,
        pet_art: Rect::new(0, 0, 0, 0),
        hit_area: habitat,
        habitat,
        exclusions: Vec::new(),
        targets: BTreeMap::new(),
        effect_targets: vec![TargetPath::new("watch.pet.effect")],
    };

    for cell in habitat_props_for(
        &habitat_view,
        &scene,
        &[],
        Species::Fuzz,
        "glorp-preview-props",
        &render_ctx,
    ) {
        let pos = Position::new(cell.col, cell.row);
        if habitat.contains(pos) {
            buffer[pos].set_char(cell.glyph).set_style(cell.style);
        }
    }
}

fn earned_view(prop: &HabitatPropSpec, earned_at: OffsetDateTime) -> EarnedHabitatPropView {
    let source = match prop.lifetime_threshold {
        Some(threshold) => HabitatPropSource::LifetimeTokens { threshold },
        None => match prop.id {
            crate::game::habitat::CODEX_SIGNAL_LAMP => {
                HabitatPropSource::ProviderFirstUse { provider_surface: "codex".to_string() }
            }
            crate::game::habitat::WILT_RECOVERY_SPROUT => HabitatPropSource::WiltRecovery,
            _ => HabitatPropSource::HeavySession,
        },
    };
    EarnedHabitatPropView {
        id: HabitatPropId::new(prop.id),
        earned_at,
        kind: prop.kind,
        display_priority: prop.display_priority,
        source,
    }
}

fn motion_phase_times(ctx: &PreviewRenderContext) -> [OffsetDateTime; 3] {
    [
        ctx.fixed_now,
        ctx.fixed_now + Duration::seconds(4),
        ctx.fixed_now + Duration::seconds(10),
    ]
}

struct WatchVariant {
    id: &'static str,
    title: &'static str,
    state: PetState,
    now: OffsetDateTime,
}

fn watch_variants(ctx: &PreviewRenderContext) -> Vec<WatchVariant> {
    vec![
        WatchVariant {
            id: "watch-habitat-early",
            title: "Watch Habitat Early",
            state: watch_state(
                ctx,
                "Mochi",
                Stage::S2,
                Species::Fuzz,
                125_000.0,
                &["token_pebble_25k", "token_shell_100k"],
            ),
            now: ctx.fixed_now,
        },
        WatchVariant {
            id: "watch-habitat-lived-in",
            title: "Watch Habitat Lived In",
            state: watch_state(
                ctx,
                "Mochi",
                Stage::S4,
                Species::Crystal,
                1_000_000.0,
                &[
                    "token_pebble_25k",
                    "token_shell_100k",
                    "token_moss_tuft_250k",
                    "token_spark_500k",
                    "token_friendly_cloud_750k",
                    "token_shard_1m",
                    CODEX_SIGNAL_LAMP,
                    HEAVY_SESSION_PLANTER,
                ],
            ),
            now: ctx.fixed_now + Duration::seconds(4),
        },
        WatchVariant {
            id: "watch-habitat-full-phase-a",
            title: "Watch Habitat Full Phase A",
            state: watch_state(
                ctx,
                "Mochi",
                Stage::S6,
                Species::Mech,
                10_000_000.0,
                &habitat_prop_ids(),
            ),
            now: ctx.fixed_now,
        },
        WatchVariant {
            id: "watch-habitat-full-phase-b",
            title: "Watch Habitat Full Phase B",
            state: watch_state(
                ctx,
                "Mochi",
                Stage::S6,
                Species::Mech,
                10_000_000.0,
                &habitat_prop_ids(),
            ),
            now: ctx.fixed_now + Duration::seconds(10),
        },
    ]
}

fn watch_state(
    ctx: &PreviewRenderContext,
    name: &str,
    stage: Stage,
    species: Species,
    lifetime_tokens: f64,
    prop_ids: &[&str],
) -> PetState {
    let mut state = PetState::new_for_test("glorp-preview-habitat-props", name);
    state.pet.generated_species = species;
    state.stage = stage;
    state.xp = 40.0;
    state.lifetime_effective_tokens = lifetime_tokens;
    state.vitals = Vitals { fed: 82.0, happiness: 78.0, energy: 74.0 };
    state.created_at = ctx.fixed_now - Duration::days(42);
    state.last_updated_at = ctx.fixed_now;
    state.last_usage_poll_at = Some(ctx.fixed_now - Duration::minutes(4));
    state.recent_events = vec![
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(18),
            text: format!("{} nosed the floor glyphs", state.pet.accepted_name),
        },
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(9),
            text: format!("{} settled by the tank", state.pet.accepted_name),
        },
    ];
    state.habitat.earned_props = prop_ids
        .iter()
        .enumerate()
        .map(|(index, id)| earned_state_prop(id, ctx.fixed_now, index))
        .collect();
    state
}

fn earned_state_prop(id: &str, now: OffsetDateTime, index: usize) -> EarnedHabitatProp {
    let earned_at = now - Duration::days(30 - index.min(29) as i64);
    EarnedHabitatProp {
        id: HabitatPropId::new(id),
        earned_at,
        source: prop_source(id),
    }
}

fn prop_source(id: &str) -> HabitatPropSource {
    if let Some(threshold) = HABITAT_PROP_CATALOG
        .iter()
        .find(|prop| prop.id == id)
        .and_then(|prop| prop.lifetime_threshold)
    {
        return HabitatPropSource::LifetimeTokens { threshold };
    }

    match id {
        CODEX_SIGNAL_LAMP => {
            HabitatPropSource::ProviderFirstUse { provider_surface: "codex".to_string() }
        }
        HEAVY_SESSION_PLANTER => HabitatPropSource::HeavySession,
        WILT_RECOVERY_SPROUT => HabitatPropSource::WiltRecovery,
        _ => HabitatPropSource::HeavySession,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_frame_lists_every_habitat_prop() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = render_habitat_props_catalog(&ctx);
        let text = frame_text(&frame);

        for id in habitat_prop_ids() {
            assert!(text.contains(id), "missing {id}");
        }
    }

    #[test]
    fn props_preview_has_catalog_and_contextual_watch_frames() {
        let ctx = PreviewRenderContext::deterministic();
        let dir = tempfile::tempdir().unwrap();

        let frames = habitat_prop_frames(&ctx, dir.path()).unwrap();
        let ids = frames
            .iter()
            .map(|frame| frame.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "habitat-props-catalog",
                "watch-habitat-early",
                "watch-habitat-lived-in",
                "watch-habitat-full-phase-a",
                "watch-habitat-full-phase-b",
            ]
        );
    }

    fn frame_text(frame: &PreviewFrame) -> String {
        let mut text = String::new();
        for y in 0..frame.height {
            for cell in frame.cells.iter().filter(|cell| cell.y == y) {
                if !cell.continuation {
                    text.push_str(&cell.symbol);
                }
            }
            text.push('\n');
        }
        text
    }
}
