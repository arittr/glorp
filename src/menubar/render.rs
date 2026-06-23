//! Build an `NSAttributedString` from a `WatchViewModel`. Two regions:
//! the 10-row pet frame (uses `StyledSegment.role` to color each cell) and
//! a stats block beneath it (dim labels, accent values).
//!
//! All char/UTF-16 arithmetic assumes BMP-only content. The pet templates and
//! particle glyphs are all in the BMP; if a future template introduces a
//! non-BMP codepoint, the span offsets will need a UTF-16 conversion pass.

#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSTextAlignment,
};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};

use crate::format::format_tokens;
use crate::pet::render::PaletteRoleName;
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::{PresentationHelperHealth, PresentationScene};
use crate::tui::identity::SourceDiversity;
use crate::tui::life::SourceAccent;
use crate::tui::view_model::WatchViewModel;

/// Approximate width in columns of the wider stats lines; used to size the
/// popover. The pet frame is 13 columns wide; stats lines are short labels +
/// short values, comfortably under 36 columns.
pub const POPOVER_COLUMNS: usize = 36;
pub const POPOVER_ROWS: usize = 22;
pub const FONT_POINT_SIZE: f64 = 13.0;

pub struct RenderedBlock {
    pub attr: Retained<NSMutableAttributedString>,
    pub char_len: usize,
}

/// Render the pet region (framed art + trailing newline). Returned `char_len`
/// is the count of `char` codepoints in the attributed string; callers use it
/// as the upper bound of the `NSRange` to replace when animating just the pet.
///
/// The pet block is center-aligned via a paragraph-style attribute so the
/// 13-char art rows sit centered in the wider popover instead of pinned to
/// the left text-container inset.
pub fn render_pet_block(vm: &WatchViewModel) -> RenderedBlock {
    let scene = PresentationScene::from_watch_view_model(
        vm,
        time::OffsetDateTime::now_utc(),
        PresentationSurface::MenubarPopover,
    );
    debug_assert!(scene.privacy.source_names_visible);
    debug_assert!(scene.privacy.exact_counts_visible);

    let mut runs: Vec<StyledRun> = Vec::new();
    append_pet(&mut runs, vm);
    runs.push(StyledRun::plain("\n"));
    let mut block = materialize(runs);
    apply_paragraph_alignment(&mut block, NSTextAlignment::Center);
    block
}

pub fn render_stats_block(vm: &WatchViewModel) -> RenderedBlock {
    let mut runs: Vec<StyledRun> = Vec::new();
    append_stats(&mut runs, vm);
    materialize(runs)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgb(u8, u8, u8);

const COLOR_FG: Rgb = Rgb(0xef, 0xeb, 0xe4);
const COLOR_DIM: Rgb = Rgb(0x97, 0x91, 0x8a);
const COLOR_ACCENT: Rgb = Rgb(0xf0, 0xa6, 0x46);

#[cfg(test)]
fn role_color_base(role: PaletteRoleName, palette: &crate::pet::palette::ResolvedPalette) -> Rgb {
    let rgb = crate::pet::palette::role_color(role, palette);
    Rgb(rgb.r, rgb.g, rgb.b)
}

/// Sleep dim factor for the popover pet (the menubar's only palette channel).
const SLEEP_DIM: f32 = 0.7;

/// Extract the source-accent color override for Accent/Particle roles, or `None`
/// for all other roles and for the no-override (base) case.
fn menubar_source_override(
    role: PaletteRoleName,
    vm: &WatchViewModel,
) -> Option<crate::pet::palette::Rgb> {
    if !matches!(role, PaletteRoleName::Accent | PaletteRoleName::Particle) {
        return None;
    }
    // Activity identity wins over life-profile accent for accent/particle glyphs.
    match vm.activity_identity.source_diversity {
        SourceDiversity::Ensemble => Some(crate::pet::palette::Rgb::new(0xf0, 0xc4, 0x6a)),
        _ => match vm.life_profile.source_accent {
            Some(SourceAccent::Codex) => Some(crate::pet::palette::Rgb::new(0x86, 0xd9, 0xef)),
            Some(SourceAccent::Claude) => Some(crate::pet::palette::Rgb::new(0xb3, 0x9d, 0xff)),
            Some(SourceAccent::Balanced) => Some(crate::pet::palette::Rgb::new(0xf0, 0xc4, 0x6a)),
            Some(SourceAccent::Ensemble) => Some(crate::pet::palette::Rgb::new(0xf0, 0xc4, 0x6a)),
            None => None,
        },
    }
}

/// Pure resolver shim: applies the shared color pipeline with MENU_STYLE
/// (source_accent + energy_droop only) and converts back to the menubar Rgb.
fn menubar_resolve(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
    source_override: Option<crate::pet::palette::Rgb>,
    asleep: bool,
) -> Rgb {
    let inputs = crate::presentation::surface::LiveColorInputs {
        source_override,
        droop_mult: if asleep { SLEEP_DIM } else { 1.0 },
        ..crate::presentation::surface::LiveColorInputs::passthrough()
    };
    let resolved = crate::presentation::surface::resolve_pet_colors(
        palette,
        &inputs,
        &crate::presentation::surface::MENU_STYLE,
    );
    let rgb = crate::presentation::surface::role_rgb(&resolved, role);
    Rgb(rgb.r, rgb.g, rgb.b)
}

fn role_color_for_profile(role: PaletteRoleName, vm: &WatchViewModel) -> Rgb {
    let source_override = menubar_source_override(role, vm);
    menubar_resolve(
        role,
        &vm.pet_palette,
        source_override,
        vm.day_context.asleep,
    )
}

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

fn append_pet(runs: &mut Vec<StyledRun>, vm: &WatchViewModel) {
    // Convert the line/start/end spans to flat run order. For each line we
    // walk the spans that cover it (in start order), filling unstyled gaps
    // with body color.
    let mut spans_by_line: Vec<Vec<&crate::pet::render::StyledSegment>> =
        vec![Vec::new(); vm.pet_art.len()];
    for span in &vm.pet_spans {
        if span.line < spans_by_line.len() {
            spans_by_line[span.line].push(span);
        }
    }
    for spans in &mut spans_by_line {
        spans.sort_by_key(|s| s.start);
    }

    for (line_index, line) in vm.pet_art.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut cursor = 0usize;
        let line_len = chars.len();
        for span in &spans_by_line[line_index] {
            let span_start = span.start.min(line_len);
            let span_end = span.end.min(line_len);
            if span_start > cursor {
                let gap: String = chars[cursor..span_start].iter().collect();
                runs.push(StyledRun::new(gap, COLOR_FG));
            }
            if span_end > span_start {
                let body: String = chars[span_start..span_end].iter().collect();
                runs.push(StyledRun::new(body, role_color_for_profile(span.role, vm)));
                cursor = span_end;
            }
        }
        if cursor < line_len {
            let tail: String = chars[cursor..line_len].iter().collect();
            runs.push(StyledRun::new(tail, COLOR_FG));
        }
        runs.push(StyledRun::plain("\n"));
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

fn apply_paragraph_alignment(block: &mut RenderedBlock, alignment: NSTextAlignment) {
    if block.char_len == 0 {
        return;
    }
    unsafe {
        let style: Retained<NSMutableParagraphStyle> = NSMutableParagraphStyle::new();
        style.setAlignment(alignment);
        block.attr.addAttribute_value_range(
            NSParagraphStyleAttributeName,
            &style,
            NSRange::from(0..block.char_len),
        );
    }
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
    use crate::pet::render::StyledSegment;
    use crate::tui::life::{PetLifeProfile, SourceAccent};
    use crate::tui::view_model::{SourceStatus, WatchViewModel};

    #[test]
    fn menubar_role_base_matches_resolved_palette() {
        use crate::pet::palette::{default_theme_palette, role_color};
        use crate::pet::render::PaletteRoleName::*;
        let p = default_theme_palette();
        for role in [Body, Eye, Mouth, Pattern] {
            let rgb = role_color(role, &p);
            assert_eq!(role_color_base(role, &p), Rgb(rgb.r, rgb.g, rgb.b));
        }
    }

    /// The animation tick uses `pet_block.char_len` as the upper bound of the
    /// `NSRange` it replaces in the text storage. If that count ever drifts
    /// from the actual UTF-16 length of the materialized attributed string,
    /// the popover will slice the stats block on every frame. This test pins
    /// the invariant for the standard 13×10 framed pet (10 framed rows + a
    /// trailing blank line).
    #[test]
    fn pet_block_char_len_matches_attributed_string_length() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = (0..10).map(|_| "             ".to_string()).collect(); // 13 spaces
        vm.pet_spans = Vec::new();

        let block = render_pet_block(&vm);

        let expected = 13 * 10 + 10 + 1; // 10 rows of 13 chars + 10 row newlines + 1 trailing newline
        assert_eq!(block.char_len, expected);
        let ns_len = block.attr.length();
        assert_eq!(
            ns_len, block.char_len,
            "BMP-only content should keep NSString UTF-16 length in sync with char count"
        );
    }

    #[test]
    fn menubar_profile_accent_is_poll_bound_and_bmp_safe() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = vec!["EAPB".to_string()];
        vm.pet_spans = vec![
            StyledSegment {
                line: 0,
                start: 0,
                end: 1,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 0,
                start: 1,
                end: 2,
                role: PaletteRoleName::Accent,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Particle,
            },
            StyledSegment {
                line: 0,
                start: 3,
                end: 4,
                role: PaletteRoleName::Body,
            },
        ];
        vm.life_profile = PetLifeProfile {
            activity_level: 1.5,
            source_accent: Some(SourceAccent::Codex),
            ..Default::default()
        };

        let block = render_pet_block(&vm);

        assert_eq!(
            block.char_len,
            block.attr.length(),
            "profile accents must not disturb the menubar BMP length invariant"
        );

        let runs = pet_runs_for_test(&vm);
        assert_eq!(runs[0].text, "E");
        assert_eq!(
            rgb_tuple(runs[0].color),
            rgb_tuple(role_color_base(PaletteRoleName::Eye, &vm.pet_palette))
        );
        assert_eq!(runs[1].text, "A");
        assert_eq!(rgb_tuple(runs[1].color), (0x86, 0xd9, 0xef));
        assert_eq!(runs[2].text, "P");
        assert_eq!(rgb_tuple(runs[2].color), (0x86, 0xd9, 0xef));
        assert_eq!(runs[3].text, "B");
        assert_eq!(
            rgb_tuple(runs[3].color),
            rgb_tuple(role_color_base(PaletteRoleName::Body, &vm.pet_palette))
        );

        let base_accent = rgb_tuple(role_color_base(PaletteRoleName::Accent, &vm.pet_palette));
        let profile_accent = rgb_tuple(role_color_for_profile(PaletteRoleName::Accent, &vm));
        assert_ne!(
            profile_accent, base_accent,
            "Codex profile should recolor the accent role"
        );
        assert_eq!(profile_accent, (0x86, 0xd9, 0xef));
    }

    #[test]
    fn sleeping_pet_dims_the_menubar_palette_and_keeps_the_bmp_invariant() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = vec!["EAPB".to_string()];
        vm.pet_spans = vec![
            StyledSegment {
                line: 0,
                start: 0,
                end: 1,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 0,
                start: 1,
                end: 2,
                role: PaletteRoleName::Accent,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Particle,
            },
            StyledSegment {
                line: 0,
                start: 3,
                end: 4,
                role: PaletteRoleName::Body,
            },
        ];
        vm.day_context.asleep = true;

        let block = render_pet_block(&vm);
        assert_eq!(
            block.char_len,
            block.attr.length(),
            "sleep dimming must not disturb the menubar BMP length invariant"
        );

        let dimmed = rgb_tuple(role_color_for_profile(PaletteRoleName::Body, &vm));
        let base = rgb_tuple(role_color_base(PaletteRoleName::Body, &vm.pet_palette));
        assert!(
            dimmed.0 < base.0 && dimmed.1 < base.1 && dimmed.2 < base.2,
            "asleep must dim every role: {dimmed:?} vs {base:?}"
        );
    }

    fn pet_runs_for_test(vm: &WatchViewModel) -> Vec<StyledRun> {
        let mut runs = Vec::new();
        append_pet(&mut runs, vm);
        runs.push(StyledRun::plain("\n"));
        runs
    }

    #[test]
    fn menubar_uses_ensemble_diversity_accent() {
        let mut vm = WatchViewModel::fixture();
        vm.activity_identity.source_diversity = SourceDiversity::Ensemble;
        vm.pet_art = vec!["EAPB".to_string()];
        vm.pet_spans = vec![
            StyledSegment {
                line: 0,
                start: 1,
                end: 2,
                role: PaletteRoleName::Accent,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Particle,
            },
        ];

        let runs = pet_runs_for_test(&vm);
        assert_eq!(rgb_tuple(runs[1].color), (0xf0, 0xc4, 0x6a));
        assert_eq!(rgb_tuple(runs[2].color), (0xf0, 0xc4, 0x6a));
    }

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

    fn rgb_tuple(rgb: Rgb) -> (u8, u8, u8) {
        let Rgb(r, g, b) = rgb;
        (r, g, b)
    }

    #[test]
    fn menubar_accent_uses_source_override_and_sleep_dim() {
        // Ensemble + asleep: accent override (0xf0,0xc4,0x6a) x0.7 truncating = (168,137,74)
        use crate::pet::palette::default_theme_palette;
        use crate::pet::render::PaletteRoleName;
        let palette = default_theme_palette();
        let out = menubar_resolve(
            PaletteRoleName::Accent,
            &palette,
            Some(crate::pet::palette::Rgb::new(0xf0, 0xc4, 0x6a)),
            true,
        );
        assert_eq!(out, Rgb(168, 137, 74));
        // Body, awake: base body unchanged
        let body = crate::pet::palette::role_color(PaletteRoleName::Body, &palette);
        let out_body = menubar_resolve(PaletteRoleName::Body, &palette, None, false);
        assert_eq!(out_body, Rgb(body.r, body.g, body.b));
    }
}
