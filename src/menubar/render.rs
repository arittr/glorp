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

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

const COLOR_FG: Rgb = Rgb(0xef, 0xeb, 0xe4);
const COLOR_DIM: Rgb = Rgb(0x97, 0x91, 0x8a);
const COLOR_FAINT: Rgb = Rgb(0x50, 0x4c, 0x49);
const COLOR_ACCENT: Rgb = Rgb(0xf0, 0xa6, 0x46);
const COLOR_GOOD: Rgb = Rgb(0x82, 0xbc, 0x83);

fn role_color(role: PaletteRoleName) -> Rgb {
    match role {
        PaletteRoleName::Body => COLOR_FG,
        PaletteRoleName::Eye => COLOR_GOOD,
        PaletteRoleName::Mouth => COLOR_DIM,
        PaletteRoleName::Accent => COLOR_ACCENT,
        PaletteRoleName::Pattern => COLOR_FAINT,
        PaletteRoleName::Particle => COLOR_ACCENT,
    }
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
                runs.push(StyledRun::new(body, role_color(span.role)));
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
        push_stat_row(
            runs,
            "xp",
            format!("{}%  →  {}", pct, vm.progress.next_stage_label),
        );
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
    push_stat_row(runs, "helper", vm.helper_status.clone());

    if !vm.errors.is_empty() {
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
    use crate::tui::view_model::WatchViewModel;

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
}
