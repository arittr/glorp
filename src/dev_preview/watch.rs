use crate::commands::watch::build_watch_view_model_at;
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;
use crate::game::evolution::Stage;
use crate::pet::generation::Species;
use crate::storage::{
    state::{PetState, Vitals},
    usage_store::{NormalizedUsageEvent, UsageStore},
};
use crate::tui::layout::render_watch_frame_with_layout;
use crate::tui::{component::layout_watch_with_context, component::preview_layout};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::path::Path;
use time::{Duration, UtcOffset};

pub fn watch_frames(ctx: &PreviewRenderContext, scratch_dir: &Path) -> Result<Vec<PreviewFrame>> {
    std::fs::create_dir_all(scratch_dir)?;

    Ok(vec![
        render_watch_frame(
            "watch-wide-normal",
            "Watch Wide Normal",
            120,
            32,
            ctx,
            scratch_dir,
        )?,
        render_watch_frame(
            "watch-tall-wide",
            "Watch Tall Wide",
            180,
            50,
            ctx,
            scratch_dir,
        )?,
        render_watch_frame(
            "watch-compact-normal",
            "Watch Compact Normal",
            72,
            24,
            ctx,
            scratch_dir,
        )?,
    ])
}

fn render_watch_frame(
    id: &str,
    title: &str,
    width: u16,
    height: u16,
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
) -> Result<PreviewFrame> {
    let state = seeded_pet_state(ctx);
    let usage_path = scratch_dir.join(format!("{id}.sqlite"));
    seed_usage_store(&usage_path, ctx)?;
    let vm = build_watch_view_model_at(&state, &usage_path, ctx.fixed_now, UtcOffset::UTC)?;
    let layout = layout_watch_with_context(Rect::new(0, 0, width, height), &vm, &ctx.render);

    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| {
        render_watch_frame_with_layout(frame, &vm, &ctx.render, &layout);
    })?;

    let mut frame = frame_from_buffer(id, title, terminal.backend().buffer());
    frame.layout = Some(preview_layout(id, &layout));
    Ok(frame)
}

fn seeded_pet_state(ctx: &PreviewRenderContext) -> PetState {
    let mut state = PetState::new_for_test("glorp-preview-watch", "Mochi");
    state.pet.generated_species = Species::Fuzz;
    state.stage = Stage::S4;
    state.xp = 8.5;
    state.lifetime_effective_tokens = 52_000.0;
    state.vitals = Vitals {
        fed: 70.0,
        happiness: 72.0,
        energy: 68.0,
    };
    state.created_at = ctx.fixed_now - Duration::days(18);
    state.last_updated_at = ctx.fixed_now;
    state.last_usage_poll_at = Some(ctx.fixed_now - Duration::minutes(5));
    state.recent_events = Vec::new();
    state
}

fn seed_usage_store(path: &Path, ctx: &PreviewRenderContext) -> Result<()> {
    let mut usage = UsageStore::open(path)?;
    for (surface, observed_at, effective_tokens, model) in [
        (
            "claude-code",
            ctx.fixed_now - Duration::minutes(5),
            12_500.0,
            "claude-sonnet",
        ),
        (
            "codex",
            ctx.fixed_now - Duration::minutes(8),
            4_200.0,
            "gpt-5-codex",
        ),
        (
            "claude-code",
            ctx.fixed_now - Duration::days(1),
            8_800.0,
            "claude-sonnet",
        ),
    ] {
        usage.insert_event(&NormalizedUsageEvent {
            provider_surface: surface.to_string(),
            model: Some(model.to_string()),
            provider_delta_id: Some(format!("preview-{surface}-{effective_tokens}")),
            ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_frames_include_wide_tall_wide_and_compact() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = PreviewRenderContext::deterministic();

        let frames = watch_frames(&ctx, dir.path()).unwrap();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].id, "watch-wide-normal");
        assert_eq!((frames[0].width, frames[0].height), (120, 32));
        assert_eq!(frames[1].id, "watch-tall-wide");
        assert_eq!((frames[1].width, frames[1].height), (180, 50));
        assert_eq!(frames[2].id, "watch-compact-normal");
        assert_eq!((frames[2].width, frames[2].height), (72, 24));
    }

    #[test]
    fn watch_frames_are_stable_for_fixed_time() {
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        let ctx = PreviewRenderContext::deterministic();

        let first = watch_frames(&ctx, first_dir.path()).unwrap();
        let second = watch_frames(&ctx, second_dir.path()).unwrap();

        assert_eq!(first, second);
    }
}
