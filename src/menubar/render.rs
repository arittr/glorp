//! Build an `NSAttributedString` from a `WatchViewModel`. Two regions:
//! the shared habitat scene (rendered via `build_round_scene_draw_list` +
//! `scene_draw_list_to_attributed`) and a stats block beneath it (dim labels,
//! accent values).
//!
//! All char/UTF-16 arithmetic assumes BMP-only content. The scene glyphs are
//! all in the BMP; if a future template introduces a non-BMP codepoint, the
//! span offsets will need a UTF-16 conversion pass.

#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2_app_kit::{
    NSBackgroundColorAttributeName, NSColor, NSFont, NSFontAttributeName, NSFontWeightBold,
    NSForegroundColorAttributeName,
};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};

use crate::format::format_tokens;
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::{PresentationHelperHealth, PresentationScene};
use crate::tui::view_model::WatchViewModel;

/// Approximate width in columns of the wider stats lines; used to size the
/// popover. Stats lines are short labels + short values, comfortably under 36
/// columns.
pub const POPOVER_COLUMNS: usize = 36;
pub const POPOVER_ROWS: usize = 22;
pub const FONT_POINT_SIZE: f64 = 13.0;

/// Height of the habitat scene region in the popover, in rows.
///
/// TUNABLE — aesthetic default (Drew's call). At 36×14 the scene trips
/// `compact=true` (trims activity budget + Orbit→Glow), the same
/// parameterization as the round companion. The remaining 8 rows are given to
/// the stats block (plus spacing). Raise `POPOVER_ROWS` or lower
/// `MENU_SCENE_ROWS` if the scene + stats block overflows the popover height.
const MENU_SCENE_ROWS: usize = 14;

pub struct RenderedBlock {
    pub attr: Retained<NSMutableAttributedString>,
    pub char_len: usize,
}

/// Render the habitat scene region as an `NSMutableAttributedString`.
///
/// Builds the full shared habitat scene via `build_round_scene_draw_list` and
/// rasterizes it with `scene_draw_list_to_attributed`. The scene is
/// `POPOVER_COLUMNS × MENU_SCENE_ROWS` — at 36×14 `compact=true` is active
/// (same parameterization as the round companion). Returned `char_len` is the
/// count of `char` codepoints; callers use it as the upper bound of the
/// `NSRange` to replace when animating just the pet region.
///
/// Pet colors come from `render_pet_to_draw_list`/`PetSceneModel` rather than
/// the old `MENU_STYLE` source-accent path — the color change is intentional
/// and flagged for Drew's visual review.
pub fn render_pet_block(vm: &WatchViewModel) -> RenderedBlock {
    let now = time::OffsetDateTime::now_utc();
    let list = crate::round::scene::build_round_scene_draw_list(
        vm,
        now,
        POPOVER_COLUMNS as u16,
        MENU_SCENE_ROWS as u16,
    );
    let attr = scene_draw_list_to_attributed(&list, POPOVER_COLUMNS as u16, MENU_SCENE_ROWS as u16);
    // Count chars so the caller can form a tight NSRange over just this region.
    // The scene text is BMP-only so char count == UTF-16 unit count.
    let char_len = attr.string().to_string().chars().count();
    RenderedBlock { attr, char_len }
}

pub fn render_stats_block(vm: &WatchViewModel) -> RenderedBlock {
    let mut runs: Vec<StyledRun> = Vec::new();
    append_stats(&mut runs, vm);
    materialize(runs)
}

/// Convert a [`crate::presentation::SceneDrawList`] to an `NSMutableAttributedString`
/// suitable for display in the menubar popover's `NSTextView`.
///
/// Calls the pure [`crate::presentation::rasterize`] to build a dense grid, then
/// coalesces consecutive cells with identical `(fg, bg, bold)` into single runs and
/// appends each run with the appropriate `NSAttributedString` attributes:
/// - `NSForegroundColorAttributeName` — `fg` mapped to `NSColor`; if `None`, uses
///   `COLOR_FG` (the popover's default text color, matching `render_pet_block`).
/// - `NSBackgroundColorAttributeName` — `bg` mapped to `NSColor`; attribute is
///   **omitted** when `bg` is `None` (transparent background inherits the text view's
///   surface color).
/// - `NSFontAttributeName` — `bold_font` when `bold`, else `font`.
///
/// Rows are joined with `"\n"`.
///
/// # Note
/// AppKit attributed-string rendering is unverified in automated tests — the pure
/// rasterize step is covered by unit tests in `src/presentation/rasterize.rs`.
///
/// wired in Plan 08 Task 2
pub fn scene_draw_list_to_attributed(
    list: &crate::presentation::SceneDrawList,
    cols: u16,
    rows: u16,
) -> Retained<NSMutableAttributedString> {
    let grid = crate::presentation::rasterize(list, cols, rows);

    let bold_font = monospace_bold_font();
    let regular_font = monospace_font();

    // Build the full string and collect run intervals in one pass.
    struct Run {
        start: usize,
        end: usize,
        fg: Option<Rgb>,
        bg: Option<Rgb>,
        bold: bool,
    }

    let mut full_text = String::new();
    let mut runs: Vec<Run> = Vec::new();

    for (row_idx, row) in grid.iter().enumerate() {
        if row_idx > 0 {
            full_text.push('\n');
        }

        // Coalesce consecutive cells with the same (fg, bg, bold) into one run.
        let mut run_start = full_text.chars().count();
        let mut run_fg: Option<crate::pet::palette::Rgb> = None;
        let mut run_bg: Option<crate::pet::palette::Rgb> = None;
        let mut run_bold = false;
        let mut first = true;

        for cell in row.iter() {
            let cell_fg = cell.fg;
            let cell_bg = cell.bg;
            let cell_bold = cell.bold;

            if first {
                run_fg = cell_fg;
                run_bg = cell_bg;
                run_bold = cell_bold;
                first = false;
            } else if cell_fg != run_fg || cell_bg != run_bg || cell_bold != run_bold {
                // Flush the current run.
                let run_end = full_text.chars().count();
                if run_end > run_start {
                    runs.push(Run {
                        start: run_start,
                        end: run_end,
                        fg: run_fg.map(|c| Rgb(c.r, c.g, c.b)),
                        bg: run_bg.map(|c| Rgb(c.r, c.g, c.b)),
                        bold: run_bold,
                    });
                }
                run_start = run_end;
                run_fg = cell_fg;
                run_bg = cell_bg;
                run_bold = cell_bold;
            }
            full_text.push(cell.glyph);
        }
        // Flush final run of the row.
        let run_end = full_text.chars().count();
        if !first && run_end > run_start {
            runs.push(Run {
                start: run_start,
                end: run_end,
                fg: run_fg.map(|c| Rgb(c.r, c.g, c.b)),
                bg: run_bg.map(|c| Rgb(c.r, c.g, c.b)),
                bold: run_bold,
            });
        }
    }

    let ns_text = NSString::from_str(&full_text);
    let mut attr_str = NSMutableAttributedString::from_nsstring(&ns_text);
    let total_chars = full_text.chars().count();
    let full_range = NSRange::from(0..total_chars);

    unsafe {
        // Apply defaults across the whole string first.
        attr_str.addAttribute_value_range(NSFontAttributeName, &regular_font, full_range);
        attr_str.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &color_for(COLOR_FG),
            full_range,
        );

        // Apply per-run attributes.
        for run in &runs {
            if run.end <= run.start {
                continue;
            }
            let range = NSRange::from(run.start..run.end);

            // Foreground color
            let fg_color = run.fg.map(color_for).unwrap_or_else(|| color_for(COLOR_FG));
            attr_str.addAttribute_value_range(NSForegroundColorAttributeName, &fg_color, range);

            // Background color — omit attribute if None (transparent)
            if let Some(bg) = run.bg {
                attr_str.addAttribute_value_range(
                    NSBackgroundColorAttributeName,
                    &color_for(bg),
                    range,
                );
            }

            // Font
            let font: &NSFont = if run.bold { &bold_font } else { &regular_font };
            attr_str.addAttribute_value_range(NSFontAttributeName, font, range);
        }
    }

    attr_str
}

/// Bold monospace font at the same point size as [`monospace_font`].
fn monospace_bold_font() -> Retained<NSFont> {
    // `monospacedSystemFontOfSize:weight:` requires macOS 10.15+.
    // NSFontWeightBold is the standard bold weight constant.
    unsafe { NSFont::monospacedSystemFontOfSize_weight(FONT_POINT_SIZE, NSFontWeightBold) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgb(u8, u8, u8);

const COLOR_FG: Rgb = Rgb(0xef, 0xeb, 0xe4);
const COLOR_DIM: Rgb = Rgb(0x97, 0x91, 0x8a);
const COLOR_ACCENT: Rgb = Rgb(0xf0, 0xa6, 0x46);

struct StyledRun {
    text: String,
    color: Rgb,
}

impl StyledRun {
    fn new(text: impl Into<String>, color: Rgb) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
    fn plain(text: impl Into<String>) -> Self {
        Self::new(text, COLOR_FG)
    }
    fn dim(text: impl Into<String>) -> Self {
        Self::new(text, COLOR_DIM)
    }
    fn accent(text: impl Into<String>) -> Self {
        Self::new(text, COLOR_ACCENT)
    }
}

fn append_stats(runs: &mut Vec<StyledRun>, vm: &WatchViewModel) {
    let scene = PresentationScene::from_watch_view_model(
        vm,
        time::OffsetDateTime::now_utc(),
        PresentationSurface::MenubarPopover,
    );
    debug_assert!(!scene.privacy.diagnostic_text_visible);

    runs.push(StyledRun::accent(vm.pet_name.clone()));
    runs.push(StyledRun::dim(format!("  ({})", vm.bio.age_label)));
    runs.push(StyledRun::plain("\n"));
    runs.push(StyledRun::dim(format!(
        "{} · {} · {}",
        vm.species, vm.stage, vm.mood
    )));
    runs.push(StyledRun::plain("\n\n"));

    push_stat_row(runs, "fed", percent(vm.fed));
    push_stat_row(runs, "happy", percent(vm.happiness));
    push_stat_row(runs, "energy", percent(vm.energy));
    if vm.progress.is_max_stage {
        push_stat_row(runs, "xp", "max evolved".into());
    } else {
        let pct = ((vm.progress.fraction * 100.0).round() as i32).clamp(0, 100);
        push_stat_row(runs, "xp", format!("{}%", pct));
    }
    runs.push(StyledRun::plain("\n"));

    push_stat_row(
        runs,
        "today",
        format!("{} tokens", format_tokens(vm.today_effective_tokens)),
    );
    push_stat_row(
        runs,
        "rate",
        format!("{} / hr", format_tokens(vm.progress.rate_per_hour)),
    );
    push_stat_row(runs, "helper", helper_status_for_menubar(vm, &scene));

    if scene.privacy.diagnostic_text_visible && !vm.errors.is_empty() {
        runs.push(StyledRun::plain("\n"));
        for err in vm.errors.iter().take(2) {
            runs.push(StyledRun::new(err.clone(), Rgb(0xea, 0x6a, 0x64)));
            runs.push(StyledRun::plain("\n"));
        }
    }
}

fn push_stat_row(runs: &mut Vec<StyledRun>, label: &str, value: String) {
    // 8-column left-aligned label, then value. Monospace font makes this
    // line up visually.
    runs.push(StyledRun::dim(format!("{label:<8}")));
    runs.push(StyledRun::plain(value));
    runs.push(StyledRun::plain("\n"));
}

fn helper_status_for_menubar(vm: &WatchViewModel, scene: &PresentationScene) -> String {
    if scene.privacy.diagnostic_text_visible || !looks_like_private_diagnostic(&vm.helper_status) {
        return vm.helper_status.clone();
    }

    match scene.activity.helper_health {
        PresentationHelperHealth::Ok => "ok".to_string(),
        PresentationHelperHealth::Trouble => "trouble".to_string(),
    }
}

fn looks_like_private_diagnostic(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("/users/")
        || lower.contains("/tmp/")
        || lower.contains("\\users\\")
        || lower.contains("prompt")
        || lower.contains("response")
        || lower.contains("tool payload")
        || lower.contains("transcript")
}

fn percent(fraction: f64) -> String {
    let v = (fraction * 100.0).round().clamp(0.0, 100.0) as i32;
    format!("{v}%")
}

fn materialize(runs: Vec<StyledRun>) -> RenderedBlock {
    let mut full_text = String::new();
    let mut intervals: Vec<(usize, usize, Rgb)> = Vec::with_capacity(runs.len());
    for run in runs {
        let start = full_text.chars().count();
        full_text.push_str(&run.text);
        let end = full_text.chars().count();
        if end > start {
            intervals.push((start, end, run.color));
        }
    }
    let ns_text = NSString::from_str(&full_text);
    let mut attr_str = NSMutableAttributedString::from_nsstring(&ns_text);

    let font = monospace_font();
    let total_chars = full_text.chars().count();
    let full_range = NSRange::from(0..total_chars);
    unsafe {
        attr_str.addAttribute_value_range(NSFontAttributeName, &font, full_range);
        attr_str.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &color_for(COLOR_FG),
            full_range,
        );
        for (start, end, rgb) in intervals {
            if end == start {
                continue;
            }
            let range = NSRange::from(start..end);
            attr_str.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &color_for(rgb),
                range,
            );
        }
    }
    RenderedBlock {
        attr: attr_str,
        char_len: total_chars,
    }
}

fn monospace_font() -> Retained<NSFont> {
    // `monospacedSystemFontOfSize:weight:` requires macOS 10.15+. Weight 0.0
    // == NSFontWeightRegular.
    unsafe { NSFont::monospacedSystemFontOfSize_weight(FONT_POINT_SIZE, 0.0) }
}

fn color_for(rgb: Rgb) -> Retained<NSColor> {
    let Rgb(r, g, b) = rgb;
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
            1.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::view_model::{SourceStatus, WatchViewModel};

    #[test]
    fn stats_block_omits_raw_helper_diagnostics_for_menubar_privacy() {
        let mut vm = WatchViewModel::fixture();
        vm.helper_status = "helper failed in /Users/drew/private/project".into();
        vm.errors = vec!["prompt response tool payload /tmp/private.rs".into()];
        vm.source_health[0].status = SourceStatus::Diagnostic;

        let text = stats_text_for_test(&vm);

        assert!(text.contains("helper"));
        assert!(text.contains("trouble"));
        for forbidden in ["/Users/drew", "/tmp/", "prompt", "response", "tool payload"] {
            assert!(
                !text.contains(forbidden),
                "menubar stats leaked {forbidden}: {text:?}"
            );
        }
    }

    #[test]
    fn stats_block_preserves_safe_helper_status_for_menubar_privacy() {
        let mut vm = WatchViewModel::fixture();
        vm.helper_status = "helper ready: claude-code, codex".into();

        let text = stats_text_for_test(&vm);

        assert!(text.contains("helper ready: claude-code, codex"));
    }

    fn stats_text_for_test(vm: &WatchViewModel) -> String {
        let mut runs = Vec::new();
        append_stats(&mut runs, vm);
        runs.into_iter().map(|run| run.text).collect()
    }
}
