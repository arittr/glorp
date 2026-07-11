//! Pure, cross-platform geometry and color helpers for the round companion HUD
//! (growth ring, stat gap, mood aura color). No AppKit; golden-testable.

use crate::game::metabolism::Mood;
use crate::round::draw::RoundColor;

/// Open-bottom growth ring geometry. Angles are degrees, CCW from +x (AppKit).
/// The gap is centered at the bottom (270°); the track sweeps CCW over the top
/// from the gap's right edge to its left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrowthRing {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub track_start_deg: f64,
    pub track_sweep_deg: f64,
}

pub const COMPANION_GAUGE_GAP_DEG: f64 = 70.0;
pub const PACE_SOFT_CAP_10M_TOKENS: f64 = 15_000_000.0;

/// The colour the companion tank's depth falloff lifts its opaque core toward.
/// Backend-neutral: both the AppKit dithered-bitmap path and the retained shader
/// derive the core tint from this one constant so the two never diverge.
pub const TANK_DEPTH_TINT: RoundColor = RoundColor(0.10, 0.11, 0.20, 1.0);

/// How much of [`TANK_DEPTH_TINT`] reaches the core. Tuned against the shipping
/// round accessory panel, which lifts blacks and eats subtle deltas: the falloff
/// has to survive that tone curve, not merely read on a calibrated Mac display.
pub const TANK_CORE_TINT_WEIGHT: f32 = 0.42;

/// The opaque core colour the tank falloff runs out from: `background` lifted
/// toward [`TANK_DEPTH_TINT`] by [`TANK_CORE_TINT_WEIGHT`], alpha preserved. Both
/// backends consume this identical value so the depth core never diverges.
pub fn tank_core_color(background: RoundColor) -> RoundColor {
    let mix = |base: f32, tint: f32| base + (tint - base) * TANK_CORE_TINT_WEIGHT;
    RoundColor(
        mix(background.0, TANK_DEPTH_TINT.0),
        mix(background.1, TANK_DEPTH_TINT.1),
        mix(background.2, TANK_DEPTH_TINT.2),
        background.3,
    )
}

/// Deterministic per-pixel tank dither in `[-1.5, 1.5]` output levels. A smooth
/// dark gradient quantised to 8 bits shows its steps as visible bands; dithering
/// trades them for imperceptible grain. Shared hash so the AppKit bitmap and the
/// shader grain agree.
pub fn tank_dither_noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    ((h & 0xFFFF) as f32 / 65535.0 - 0.5) * 3.0
}

/// One straight-sRGB8 pixel of the tank's radial depth falloff, dithered: the
/// normalized radius interpolates `core`->`rim` in sRGB space, the dither grain is
/// added, and the result is quantised to 8 bits. The retained shader reproduces
/// this exact math per fragment; this is the single Rust source of truth the
/// AppKit tank bitmap is built from.
pub fn tank_background_sample(
    x: u32,
    y: u32,
    center: (f32, f32),
    radius: f32,
    core: RoundColor,
    rim: RoundColor,
) -> [u8; 4] {
    let dx = x as f32 + 0.5 - center.0;
    let dy = y as f32 + 0.5 - center.1;
    let t = if radius > 0.0 {
        ((dx * dx + dy * dy).sqrt() / radius).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let noise = tank_dither_noise(x, y);
    let channel = |core_c: f32, rim_c: f32| {
        ((core_c + (rim_c - core_c) * t) * 255.0 + noise)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [
        channel(core.0, rim.0),
        channel(core.1, rim.1),
        channel(core.2, rim.2),
        255,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLane {
    pub ring: GrowthRing,
    pub stroke_width: f64,
    pub cap: LineCap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeLayout {
    pub xp: GaugeLane,
    pub daily: GaugeLane,
    pub pace: GaugeLane,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLaneColors {
    pub track: RoundColor,
    pub fill: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeColors {
    pub xp: GaugeLaneColors,
    pub daily: GaugeLaneColors,
    pub pace: GaugeLaneColors,
}

pub fn growth_ring_layout(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> GrowthRing {
    let gap = gap_deg.clamp(0.0, 180.0);
    GrowthRing {
        cx,
        cy,
        radius,
        track_start_deg: 270.0 + gap / 2.0,
        track_sweep_deg: 360.0 - gap,
    }
}

/// Angle (deg) where the violet fill ends for `fraction` of stage progress.
pub fn growth_ring_fill_end_deg(ring: &GrowthRing, fraction: f64) -> f64 {
    ring.track_start_deg + ring.track_sweep_deg * fraction.clamp(0.0, 1.0)
}

pub fn perimeter_gauge_layout(
    cx: f64,
    cy: f64,
    aperture_radius: f64,
    gap_deg: f64,
) -> PerimeterGaugeLayout {
    let outer_inset_px = 3.0_f64.max(aperture_radius * 0.012);
    let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
    let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
    let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
    let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);

    let xp_radius = aperture_radius - outer_inset_px - xp_width / 2.0;
    let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
    let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;

    PerimeterGaugeLayout {
        xp: GaugeLane {
            ring: growth_ring_layout(cx, cy, xp_radius, gap_deg),
            stroke_width: xp_width,
            cap: LineCap::Round,
        },
        daily: GaugeLane {
            ring: growth_ring_layout(cx, cy, daily_radius, gap_deg),
            stroke_width: daily_width,
            cap: LineCap::Round,
        },
        pace: GaugeLane {
            ring: growth_ring_layout(cx, cy, pace_radius, gap_deg),
            stroke_width: pace_width,
            cap: LineCap::Round,
        },
    }
}

pub fn perimeter_gauge_colors() -> PerimeterGaugeColors {
    PerimeterGaugeColors {
        xp: GaugeLaneColors {
            track: RoundColor(0.71, 0.71, 0.78, 0.16),
            fill: RoundColor(0.61, 0.48, 0.88, 0.90),
        },
        daily: GaugeLaneColors {
            track: RoundColor(0.47, 0.63, 0.43, 0.12),
            fill: RoundColor(0.50, 0.74, 0.56, 0.76),
        },
        pace: GaugeLaneColors {
            track: RoundColor(0.96, 0.68, 0.31, 0.13),
            fill: RoundColor(0.98, 0.67, 0.27, 0.86),
        },
    }
}

/// The region (in pixels) the token stat must fit inside: centered in the ring's
/// bottom gap, below center, clamped to the gap chord so it never clips the ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatGap {
    pub center_x: f64,
    pub baseline_y: f64,
    pub max_width: f64,
}

pub fn stat_gap_box(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> StatGap {
    let gap = gap_deg.clamp(0.0, 180.0);
    let half_chord = radius * (gap / 2.0).to_radians().sin();
    StatGap {
        center_x: cx,
        // Place the readout in the lower band, a bit above the gap mouth.
        baseline_y: cy + radius * 0.55,
        // A small inset keeps the text off the ring stroke.
        max_width: (2.0 * half_chord * 0.92).max(0.0),
    }
}

/// The starting HUD stack size as a multiple of the derived HUD font size.
pub const HUD_STACK_INITIAL_SCALE: f64 = 1.45;
/// The smallest HUD stack size the shrink loop will step down to.
pub const HUD_STACK_MIN: f64 = 6.0;
/// The big (token total) line's font size as a multiple of the stack size.
pub const HUD_BIG_LINE_SCALE: f64 = 1.08;
/// The two sub-lines' font size as a multiple of the stack size.
pub const HUD_SUB_LINE_SCALE: f64 = 0.68;
/// Vertical advance between HUD lines as a multiple of a line's own height.
pub const HUD_LINE_ADVANCE: f64 = 0.82;
/// How far below the gap top the whole stack starts, as a fraction of its height.
pub const HUD_STACK_TOP_FRACTION: f64 = 0.38;
/// The stack must fit within this fraction of the aperture radius, else it shrinks.
pub const HUD_HEIGHT_LIMIT_FRACTION: f64 = 0.34;

/// The three HUD line font sizes for a stack size: the big token total, then the
/// two smaller sub-lines. Backend-neutral so both renderers scale identically.
pub fn hud_line_font_sizes(stack_size: f64) -> [f64; 3] {
    [
        stack_size * HUD_BIG_LINE_SCALE,
        stack_size * HUD_SUB_LINE_SCALE,
        stack_size * HUD_SUB_LINE_SCALE,
    ]
}

/// A single HUD line's measured extent at a given font size, as each backend's own
/// text engine reports it (AppKit's attributed-string metrics or the retained
/// glyph atlas). The shared layout consumes these; only the measurement differs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudLineMetrics {
    pub width: f64,
    pub height: f64,
}

/// One placed HUD line: where its run starts (already centered in the gap), the
/// baseline it draws at, its measured extent, and the font size it renders at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedHudLine {
    pub origin_x: f64,
    pub baseline_y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size: f64,
}

/// The placed HUD stack: three centered lines and the stack size they settled at.
/// Backend-neutral — both renderers compute this from the same shrink policy and
/// positioning math, differing only in the measurement closure they supply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedHudLayout {
    pub lines: [PreparedHudLine; 3],
    pub stack_size: f64,
}

/// Places the three HUD lines inside the bottom `gap`: the stack starts at
/// `hud_font_size * HUD_STACK_INITIAL_SCALE` and shrinks a point at a time while
/// its widest run overflows `gap.max_width` or its height overflows
/// `aperture_radius * HUD_HEIGHT_LIMIT_FRACTION` (never below `HUD_STACK_MIN`).
/// Each line is centered in the gap and the stack descends from the gap top.
///
/// `measure` reports each line's `(width, height)` at the three font sizes; it is
/// the only backend-specific input, so two renderers that measure the same runs
/// the same way place them identically.
pub fn prepare_hud_layout(
    gap: StatGap,
    aperture_radius: f64,
    view_height: f64,
    hud_font_size: f64,
    mut measure: impl FnMut([f64; 3]) -> [HudLineMetrics; 3],
) -> PreparedHudLayout {
    let mut stack_size = hud_font_size * HUD_STACK_INITIAL_SCALE;
    let mut metrics = measure(hud_line_font_sizes(stack_size));
    let stack_extent = |metrics: &[HudLineMetrics; 3]| {
        let max_width = metrics
            .iter()
            .map(|line| line.width)
            .fold(0.0_f64, f64::max);
        let total_height = metrics[0].height
            + metrics[1].height * HUD_LINE_ADVANCE
            + metrics[2].height * HUD_LINE_ADVANCE;
        (max_width, total_height)
    };
    let (mut max_width, mut total_height) = stack_extent(&metrics);
    while (max_width > gap.max_width || total_height > aperture_radius * HUD_HEIGHT_LIMIT_FRACTION)
        && stack_size > HUD_STACK_MIN
    {
        stack_size -= 1.0;
        metrics = measure(hud_line_font_sizes(stack_size));
        let extent = stack_extent(&metrics);
        max_width = extent.0;
        total_height = extent.1;
    }

    let font_sizes = hud_line_font_sizes(stack_size);
    let top = view_height - gap.baseline_y;
    let mut baseline_y = top + total_height * HUD_STACK_TOP_FRACTION;
    let mut lines = [PreparedHudLine {
        origin_x: 0.0,
        baseline_y: 0.0,
        width: 0.0,
        height: 0.0,
        font_size: 0.0,
    }; 3];
    for (index, line) in lines.iter_mut().enumerate() {
        let width = metrics[index].width;
        let height = metrics[index].height;
        *line = PreparedHudLine {
            origin_x: gap.center_x - width / 2.0,
            baseline_y,
            width,
            height,
            font_size: font_sizes[index],
        };
        baseline_y -= height * HUD_LINE_ADVANCE;
    }
    PreparedHudLayout { lines, stack_size }
}

/// Soft-glow aura hue for the pet's mood. Opaque (alpha 1.0); the renderer
/// applies its own translucency. Sad and Sleepy are deliberately distinct hues
/// (different needs: happiness<35 vs energy<20). Starting palette — tuned on device.
pub fn mood_aura_color(mood: Mood) -> RoundColor {
    match mood {
        Mood::Content => RoundColor(0.25, 0.71, 0.60, 1.0), // teal
        Mood::Happy => RoundColor(0.82, 0.45, 0.62, 1.0),   // warm pink
        Mood::Ecstatic => RoundColor(0.95, 0.40, 0.70, 1.0), // bright magenta-pink
        Mood::Hungry => RoundColor(0.85, 0.62, 0.30, 1.0),  // amber
        Mood::Sad => RoundColor(0.40, 0.50, 0.78, 1.0),     // muted blue
        Mood::Sleepy => RoundColor(0.55, 0.50, 0.80, 1.0),  // indigo/violet
        Mood::Wilted => RoundColor(0.45, 0.40, 0.48, 1.0),  // dim grey-mauve
    }
}

pub fn rate_direction_color(direction: crate::tui::view_model::RateDirection) -> RoundColor {
    match direction {
        crate::tui::view_model::RateDirection::Up => RoundColor(0.45, 0.84, 0.51, 1.0),
        crate::tui::view_model::RateDirection::Down => RoundColor(0.95, 0.38, 0.36, 1.0),
        crate::tui::view_model::RateDirection::Neutral => RoundColor(0.62, 0.63, 0.77, 1.0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionHudText {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

pub fn companion_pace_fraction(current_10m_tokens: f64) -> f64 {
    if !current_10m_tokens.is_finite() || current_10m_tokens <= 0.0 {
        return 0.0;
    }
    (1.0 - (-current_10m_tokens / PACE_SOFT_CAP_10M_TOKENS).exp()).clamp(0.0, 1.0)
}

pub fn daily_fraction_for_gauge(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

pub fn daily_overage_marker_fraction(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 1.0)
        .map(|value| (value - 1.0).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

pub fn daily_overage_color() -> RoundColor {
    RoundColor(0.72, 0.95, 0.34, 0.95)
}

pub fn daily_overage_marker_arc(ring: &GrowthRing, marker_fraction: f64) -> Option<(f64, f64)> {
    let clamped = marker_fraction.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return None;
    }

    Some((
        ring.track_start_deg,
        growth_ring_fill_end_deg(ring, clamped),
    ))
}

/// The four gauge fractions a frame carries, in `[0, 1+]`. `daily_overage` is the
/// amount past 100% of yesterday; the base lanes are already clamped to `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeFractions {
    pub xp: f64,
    pub daily: f64,
    pub daily_overage: f64,
    pub pace: f64,
}

/// One perimeter-gauge arc to stroke: a ring, stroke width, cap, angular span, and
/// colour. Backend-neutral — both the AppKit painter and the retained GPU prep
/// consume the identical list from [`prepared_perimeter_gauge_arcs`], so the gauge
/// geometry is derived in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedGaugeArc {
    pub ring: GrowthRing,
    pub stroke_width: f64,
    pub cap: LineCap,
    pub start_deg: f64,
    pub end_deg: f64,
    pub color: RoundColor,
}

/// The ordered back-to-front list of perimeter-gauge arcs for a frame: each lane's
/// full track, then its fill (only when its fraction is positive), the daily
/// overage marker (only when present), and the pace lane last. The order matches
/// the AppKit painter's historical draw order so both backends composite the same
/// arcs in the same sequence.
pub fn prepared_perimeter_gauge_arcs(
    layout: &PerimeterGaugeLayout,
    colors: &PerimeterGaugeColors,
    fractions: GaugeFractions,
) -> Vec<PreparedGaugeArc> {
    let mut arcs = Vec::new();
    push_lane_arcs(&mut arcs, &layout.xp, &colors.xp, fractions.xp);
    push_lane_arcs(&mut arcs, &layout.daily, &colors.daily, fractions.daily);
    if let Some((start_deg, end_deg)) =
        daily_overage_marker_arc(&layout.daily.ring, fractions.daily_overage)
    {
        arcs.push(PreparedGaugeArc {
            ring: layout.daily.ring,
            stroke_width: layout.daily.stroke_width,
            cap: layout.daily.cap,
            start_deg,
            end_deg,
            color: daily_overage_color(),
        });
    }
    push_lane_arcs(&mut arcs, &layout.pace, &colors.pace, fractions.pace);
    arcs
}

/// Appends a lane's full track and, when `fraction` is positive, its fill arc.
fn push_lane_arcs(
    arcs: &mut Vec<PreparedGaugeArc>,
    lane: &GaugeLane,
    colors: &GaugeLaneColors,
    fraction: f64,
) {
    let track_end = lane.ring.track_start_deg + lane.ring.track_sweep_deg;
    arcs.push(PreparedGaugeArc {
        ring: lane.ring,
        stroke_width: lane.stroke_width,
        cap: lane.cap,
        start_deg: lane.ring.track_start_deg,
        end_deg: track_end,
        color: colors.track,
    });
    let clamped = fraction.clamp(0.0, 1.0);
    if clamped > 0.0 {
        arcs.push(PreparedGaugeArc {
            ring: lane.ring,
            stroke_width: lane.stroke_width,
            cap: lane.cap,
            start_deg: lane.ring.track_start_deg,
            end_deg: growth_ring_fill_end_deg(&lane.ring, clamped),
            color: colors.fill,
        });
    }
}

pub fn format_daily_percent(fraction_of_yesterday: Option<f64>) -> String {
    let Some(fraction) = fraction_of_yesterday else {
        return "--% yday".to_string();
    };
    if !fraction.is_finite() || fraction < 0.0 {
        return "--% yday".to_string();
    }

    let percent = (fraction * 100.0).round();
    if percent > 999.0 {
        "999%+ yday".to_string()
    } else {
        format!("{percent:.0}% yday")
    }
}

pub fn companion_hud_text(
    today_tokens: f64,
    daily_fraction: Option<f64>,
    pulse_10m_tokens: f64,
) -> CompanionHudText {
    CompanionHudText {
        today_total: compact_hud_tokens(today_tokens),
        daily_percent: format_daily_percent(daily_fraction),
        pace: format!("{}/10m", compact_hud_tokens(pulse_10m_tokens.max(0.0))),
    }
}

/// The redacted HUD a review-pair capture renders in place of live values so no
/// token counts leak into the parity artifacts. Review captures render this by
/// default (`redacts_live_hud`), so its glyphs are part of the companion's
/// declared HUD charset that the retained atlas preflight must enumerate — even
/// though a normal live frame never shows them.
pub fn review_capture_hud_text() -> CompanionHudText {
    CompanionHudText {
        today_total: "review".into(),
        daily_percent: "privacy".into(),
        pace: "redacted".into(),
    }
}

fn compact_hud_tokens(value: f64) -> String {
    let formatted = crate::format::format_tokens(value);
    formatted
        .strip_suffix(".0B")
        .map(|prefix| format!("{prefix}B"))
        .or_else(|| {
            formatted
                .strip_suffix(".0M")
                .map(|prefix| format!("{prefix}M"))
        })
        .or_else(|| {
            formatted
                .strip_suffix(".0k")
                .map(|prefix| format!("{prefix}k"))
        })
        .unwrap_or(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tank_core_lifts_the_background_toward_the_depth_tint() {
        let bg = RoundColor(0.05, 0.05, 0.06, 1.0);
        let core = tank_core_color(bg);
        // The core is a blend toward the tint, so it sits strictly between the two.
        assert!(core.0 > bg.0 && core.0 < TANK_DEPTH_TINT.0);
        assert!(core.1 > bg.1 && core.1 < TANK_DEPTH_TINT.1);
        assert!(core.2 > bg.2 && core.2 < TANK_DEPTH_TINT.2);
        // The porthole is opaque; only the tint's weight varies.
        assert_eq!(core.3, 1.0);
    }

    #[test]
    fn tank_core_reproduces_the_configured_tint_weight() {
        let bg = RoundColor(0.0, 0.0, 0.0, 1.0);
        let core = tank_core_color(bg);
        assert!((core.0 - TANK_DEPTH_TINT.0 * TANK_CORE_TINT_WEIGHT).abs() < 1e-6);
    }

    #[test]
    fn tank_dither_noise_is_deterministic_bounded_and_varied() {
        let mut seen = std::collections::BTreeSet::new();
        for x in 0..64u32 {
            for y in 0..64u32 {
                let n = tank_dither_noise(x, y);
                assert_eq!(n, tank_dither_noise(x, y));
                assert!((-1.5..=1.5).contains(&n), "noise {n} out of range");
                seen.insert((n * 1000.0) as i32);
            }
        }
        assert!(
            seen.len() > 100,
            "noise must vary, got {} values",
            seen.len()
        );
    }

    #[test]
    fn tank_background_sample_runs_core_to_rim_within_one_output_level() {
        // Core, mid, and rim each land within one dither step (<= 2 output levels)
        // of the analytic core->rim interpolation, so both backends read the same
        // falloff. A center pixel is t=0 (core); the frame edge is t=1 (rim).
        let core = RoundColor(0.20, 0.22, 0.30, 1.0);
        let rim = RoundColor(0.05, 0.05, 0.06, 1.0);
        let center = (100.0, 100.0);

        let at_core = tank_background_sample(100, 100, center, 100.0, core, rim);
        let at_mid = tank_background_sample(150, 100, center, 100.0, core, rim);
        let at_rim = tank_background_sample(199, 100, center, 100.0, core, rim);
        assert!((f32::from(at_core[0]) - 0.20 * 255.0).abs() <= 2.0);
        assert!((f32::from(at_rim[0]) - 0.05 * 255.0).abs() <= 2.0);
        // The midpoint sits between the two endpoints (t≈0.5).
        let mid_ideal = (0.20 + 0.05) / 2.0 * 255.0;
        assert!((f32::from(at_mid[0]) - mid_ideal).abs() <= 4.0);
        assert_eq!(at_core[3], 255);

        // Past the radius the falloff clamps: only the dither grain remains.
        let beyond = tank_background_sample(390, 100, center, 100.0, core, rim);
        assert!((f32::from(beyond[0]) - 0.05 * 255.0).abs() <= 2.0);
    }

    #[test]
    fn prepared_hud_layout_centers_and_stacks_lines_from_shared_measurements() {
        let gap = StatGap {
            center_x: 90.0,
            baseline_y: 150.0,
            max_width: 40.0,
        };
        // A measurement both backends could produce: each line 2.0 wide, 1.0 tall,
        // regardless of font size — so nothing forces a shrink.
        let measure = |_sizes: [f64; 3]| [HudLineMetrics { width: 2.0, height: 1.0 }; 3];
        let layout = prepare_hud_layout(gap, 100.0, 300.0, 8.0, measure);

        // No shrink: the stack keeps its initial size and per-line scales.
        assert!((layout.stack_size - 8.0 * HUD_STACK_INITIAL_SCALE).abs() < 1e-9);
        let sizes = hud_line_font_sizes(layout.stack_size);
        assert!((layout.lines[0].font_size - sizes[0]).abs() < 1e-9);
        assert!((layout.lines[1].font_size - sizes[1]).abs() < 1e-9);
        assert_eq!(layout.lines[1].font_size, layout.lines[2].font_size);

        // Each run is centered in the gap.
        for line in &layout.lines {
            assert!((line.origin_x - (gap.center_x - line.width / 2.0)).abs() < 1e-9);
        }

        // The stack descends from the top of the gap by one advance per line.
        let total_height = 1.0 + 1.0 * HUD_LINE_ADVANCE + 1.0 * HUD_LINE_ADVANCE;
        let top = 300.0 - gap.baseline_y;
        let y0 = top + total_height * HUD_STACK_TOP_FRACTION;
        assert!((layout.lines[0].baseline_y - y0).abs() < 1e-9);
        assert!((layout.lines[1].baseline_y - (y0 - HUD_LINE_ADVANCE)).abs() < 1e-9);
        assert!((layout.lines[2].baseline_y - (y0 - 2.0 * HUD_LINE_ADVANCE)).abs() < 1e-9);
    }

    #[test]
    fn prepared_hud_layout_shrinks_toward_the_floor_when_runs_overflow_the_gap() {
        let gap = StatGap {
            center_x: 90.0,
            baseline_y: 150.0,
            max_width: 40.0,
        };
        // Runs far wider than the gap force the shrink loop down to its floor.
        let measure = |_sizes: [f64; 3]| [HudLineMetrics { width: 1.0e9, height: 1.0 }; 3];
        let layout = prepare_hud_layout(gap, 100.0, 300.0, 8.0, measure);
        assert!(layout.stack_size < 8.0 * HUD_STACK_INITIAL_SCALE);
        assert!(layout.stack_size <= HUD_STACK_MIN + 1.0);
    }

    #[test]
    fn prepared_hud_layout_is_identical_for_both_backends_on_the_same_measurements() {
        let gap = StatGap {
            center_x: 64.0,
            baseline_y: 120.0,
            max_width: 30.0,
        };
        // Two backends that measure the same run metrics must land the same layout.
        let smooth_measure =
            |sizes: [f64; 3]| sizes.map(|size| HudLineMetrics { width: size * 0.6, height: size });
        let retained_measure =
            |sizes: [f64; 3]| sizes.map(|size| HudLineMetrics { width: size * 0.6, height: size });
        let a = prepare_hud_layout(gap, 90.0, 260.0, 7.0, smooth_measure);
        let b = prepare_hud_layout(gap, 90.0, 260.0, 7.0, retained_measure);
        assert_eq!(a, b);
    }

    #[test]
    fn prepared_gauge_arcs_cover_zero_partial_full_and_overage_identically_for_both_backends() {
        let layout = perimeter_gauge_layout(180.0, 180.0, 180.0, COMPANION_GAUGE_GAP_DEG);
        let colors = perimeter_gauge_colors();

        // Zero everywhere: only the three lane tracks are drawn, no fills, no overage.
        let zero = prepared_perimeter_gauge_arcs(
            &layout,
            &colors,
            GaugeFractions {
                xp: 0.0,
                daily: 0.0,
                daily_overage: 0.0,
                pace: 0.0,
            },
        );
        assert_eq!(zero.len(), 3);
        assert!(zero.iter().all(|arc| (arc.end_deg
            - (arc.ring.track_start_deg + arc.ring.track_sweep_deg))
            .abs()
            < 1e-9));
        assert_eq!(zero[0].color, colors.xp.track);
        assert_eq!(zero[1].color, colors.daily.track);
        assert_eq!(zero[2].color, colors.pace.track);

        // Partial fills (no overage): each lane draws track then fill, in order.
        let partial = prepared_perimeter_gauge_arcs(
            &layout,
            &colors,
            GaugeFractions {
                xp: 0.5,
                daily: 0.3,
                daily_overage: 0.0,
                pace: 0.7,
            },
        );
        assert_eq!(partial.len(), 6);
        assert_eq!(partial[1].color, colors.xp.fill);
        assert!((partial[1].end_deg - growth_ring_fill_end_deg(&layout.xp.ring, 0.5)).abs() < 1e-9);
        assert_eq!(partial[3].color, colors.daily.fill);
        assert_eq!(partial[5].color, colors.pace.fill);

        // Full xp fill reaches the track end.
        let full = prepared_perimeter_gauge_arcs(
            &layout,
            &colors,
            GaugeFractions {
                xp: 1.0,
                daily: 0.0,
                daily_overage: 0.0,
                pace: 0.0,
            },
        );
        assert!(
            (full[1].end_deg - (layout.xp.ring.track_start_deg + layout.xp.ring.track_sweep_deg))
                .abs()
                < 1e-9
        );

        // Overage inserts one daily-coloured marker arc after the daily fill and
        // before the pace lane, matching the Smooth draw order.
        let overage = prepared_perimeter_gauge_arcs(
            &layout,
            &colors,
            GaugeFractions {
                xp: 0.0,
                daily: 1.0,
                daily_overage: 0.25,
                pace: 0.0,
            },
        );
        // xp track, daily track, daily fill, daily overage, pace track = 5 arcs.
        assert_eq!(overage.len(), 5);
        let marker = overage[3];
        assert_eq!(marker.color, daily_overage_color());
        let expected = daily_overage_marker_arc(&layout.daily.ring, 0.25).unwrap();
        assert!((marker.start_deg - expected.0).abs() < 1e-9);
        assert!((marker.end_deg - expected.1).abs() < 1e-9);
        assert_eq!(marker.ring, layout.daily.ring);
        assert_eq!(marker.stroke_width, layout.daily.stroke_width);
    }

    #[test]
    fn ring_gap_is_centered_at_bottom_and_excluded() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
        // Track spans 360 - gap = 290 degrees.
        assert!((ring.track_sweep_deg - 290.0).abs() < 1e-6);
        // Track starts at the right edge of the bottom gap: 270 + 35 = 305 deg.
        assert!((ring.track_start_deg - 305.0).abs() < 1e-6);
        // Bottom (270°) is inside the gap, i.e. NOT covered by [start, start+sweep].
        // 270° < track_start (305°) so it lies before the track begins; 630° (≡ 270° + 360°)
        // must also be absent from [start, end] to confirm the gap is not wrapped-over.
        let end = ring.track_start_deg + ring.track_sweep_deg; // 595
        assert!(
            !(ring.track_start_deg..=end).contains(&270.0_f64)
                && !(ring.track_start_deg..=end).contains(&630.0_f64),
            "270° must lie in the gap, not on the track"
        );
    }

    #[test]
    fn fill_end_spans_fraction_of_the_track() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
        assert!((growth_ring_fill_end_deg(&ring, 0.0) - ring.track_start_deg).abs() < 1e-6);
        assert!(
            (growth_ring_fill_end_deg(&ring, 1.0) - (ring.track_start_deg + ring.track_sweep_deg))
                .abs()
                < 1e-6
        );
        let half = growth_ring_fill_end_deg(&ring, 0.5);
        assert!((half - (ring.track_start_deg + 145.0)).abs() < 1e-6);
        // Clamps out-of-range fractions.
        assert!(
            (growth_ring_fill_end_deg(&ring, 2.0) - (ring.track_start_deg + ring.track_sweep_deg))
                .abs()
                < 1e-6
        );
        assert!((growth_ring_fill_end_deg(&ring, -1.0) - ring.track_start_deg).abs() < 1e-6);
    }

    #[test]
    fn every_mood_has_a_distinct_aura_color() {
        let moods = [
            Mood::Content,
            Mood::Happy,
            Mood::Ecstatic,
            Mood::Hungry,
            Mood::Sad,
            Mood::Sleepy,
            Mood::Wilted,
        ];
        let colors: Vec<RoundColor> = moods.iter().map(|m| mood_aura_color(*m)).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(
                    colors[i], colors[j],
                    "moods {:?} and {:?} must have distinct aura colors",
                    moods[i], moods[j]
                );
            }
        }
    }

    #[test]
    fn sad_and_sleepy_are_distinct() {
        assert_ne!(mood_aura_color(Mood::Sad), mood_aura_color(Mood::Sleepy));
    }

    #[test]
    fn rate_direction_colors_are_distinct() {
        use crate::tui::view_model::RateDirection;

        assert_ne!(
            rate_direction_color(RateDirection::Up),
            rate_direction_color(RateDirection::Down)
        );
        assert_ne!(
            rate_direction_color(RateDirection::Neutral),
            rate_direction_color(RateDirection::Up)
        );
    }

    #[test]
    fn daily_gauge_color_is_muted_sage_not_cyan() {
        let colors = perimeter_gauge_colors();
        let RoundColor(track_red, track_green, track_blue, track_alpha) = colors.daily.track;
        let RoundColor(fill_red, fill_green, fill_blue, fill_alpha) = colors.daily.fill;

        assert!(
            track_green > track_blue + 0.08,
            "daily track should not read as blue/cyan: {:?}",
            colors.daily.track
        );
        assert!(
            fill_green > fill_blue + 0.10,
            "daily fill should not read as blue/cyan: {:?}",
            colors.daily.fill
        );
        assert!(track_green > track_red);
        assert!(fill_green > fill_red + 0.12);
        assert!(track_alpha <= 0.14);
        assert!(fill_alpha <= 0.78);
        assert_ne!(colors.daily.fill, colors.xp.fill);
        assert_ne!(colors.daily.fill, colors.pace.fill);
    }

    #[test]
    fn daily_gauge_overage_marker_tracks_extra_fraction_without_recoloring_base_lane() {
        assert_eq!(daily_overage_marker_fraction(Some(0.99)), 0.0);
        assert!((daily_overage_marker_fraction(Some(1.07)) - 0.07).abs() < 0.001);
        assert!((daily_overage_marker_fraction(Some(1.25)) - 0.25).abs() < 0.001);
        assert_eq!(daily_overage_marker_fraction(Some(2.5)), 1.0);
        assert_eq!(daily_overage_marker_fraction(None), 0.0);
        assert_eq!(daily_overage_marker_fraction(Some(f64::NAN)), 0.0);
        assert_eq!(daily_overage_color(), RoundColor(0.72, 0.95, 0.34, 0.95));
    }

    #[test]
    fn daily_gauge_overage_marker_wraps_to_the_right_edge() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, COMPANION_GAUGE_GAP_DEG);
        let Some((start, end)) = daily_overage_marker_arc(&ring, 0.77) else {
            panic!("expected visible overage marker arc");
        };

        assert_eq!(start, ring.track_start_deg);
        assert!((end - (ring.track_start_deg + ring.track_sweep_deg * 0.77)).abs() < 1e-6);
    }

    #[test]
    fn stat_gap_box_sits_below_center_and_within_the_chord() {
        let gap = stat_gap_box(100.0, 100.0, 90.0, 70.0);
        assert!((gap.center_x - 100.0).abs() < 1e-6, "centered horizontally");
        assert!(
            gap.baseline_y > 100.0,
            "stat sits below the vertical center (lower half)"
        );
        // The gap chord half-width at the ring edges is radius * sin(gap/2).
        let expected_half = 90.0 * (35.0_f64.to_radians()).sin();
        assert!(
            gap.max_width <= 2.0 * expected_half + 1e-6,
            "stat must fit within the gap chord"
        );
        assert!(gap.max_width > 0.0);
    }

    #[test]
    fn perimeter_gauge_layout_keeps_three_round_lanes_inside_aperture() {
        let layout = perimeter_gauge_layout(180.0, 180.0, 180.0, COMPANION_GAUGE_GAP_DEG);

        assert_eq!(layout.xp.cap, LineCap::Round);
        assert_eq!(layout.daily.cap, LineCap::Round);
        assert_eq!(layout.pace.cap, LineCap::Round);

        assert_eq!(
            layout.xp.ring.track_start_deg,
            layout.daily.ring.track_start_deg
        );
        assert_eq!(
            layout.daily.ring.track_start_deg,
            layout.pace.ring.track_start_deg
        );
        assert_eq!(
            layout.xp.ring.track_sweep_deg,
            layout.daily.ring.track_sweep_deg
        );
        assert_eq!(
            layout.daily.ring.track_sweep_deg,
            layout.pace.ring.track_sweep_deg
        );

        assert!(layout.xp.ring.radius > layout.daily.ring.radius);
        assert!(layout.daily.ring.radius > layout.pace.ring.radius);
        assert!(layout.xp.stroke_width > layout.daily.stroke_width);
        assert!(layout.daily.stroke_width > layout.pace.stroke_width);

        let xp_outer_edge = layout.xp.ring.radius + layout.xp.stroke_width / 2.0;
        let pace_inner_edge = layout.pace.ring.radius - layout.pace.stroke_width / 2.0;

        assert!(xp_outer_edge <= 177.0);
        assert!(pace_inner_edge > 180.0 * 0.72);
    }

    #[test]
    fn pace_fraction_uses_named_soft_cap_and_clamps_bad_inputs() {
        assert_eq!(PACE_SOFT_CAP_10M_TOKENS, 15_000_000.0);
        assert_eq!(companion_pace_fraction(0.0), 0.0);
        assert!((companion_pace_fraction(4_000_000.0) - 0.234).abs() < 0.002);
        assert!((companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS) - 0.632).abs() < 0.002);
        assert!((companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 2.0) - 0.865).abs() < 0.002);
        assert!(companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 100.0) <= 1.0);
        assert_eq!(companion_pace_fraction(-1.0), 0.0);
        assert_eq!(companion_pace_fraction(f64::NAN), 0.0);
        assert_eq!(companion_pace_fraction(f64::INFINITY), 0.0);
    }

    #[test]
    fn companion_hud_text_formats_total_daily_percent_and_pace_only() {
        let text = companion_hud_text(842_000_000.0, Some(1.244), 31_000_000.0);

        assert_eq!(text.today_total, "842M");
        assert_eq!(text.daily_percent, "124% yday");
        assert_eq!(text.pace, "31M/10m");
        assert!(!text.pace.contains("/hr"));
    }

    #[test]
    fn daily_percent_text_preserves_stack_when_unavailable_and_caps_extreme_values() {
        assert_eq!(format_daily_percent(None), "--% yday");
        assert_eq!(format_daily_percent(Some(0.944)), "94% yday");
        assert_eq!(format_daily_percent(Some(10.5)), "999%+ yday");
        assert_eq!(format_daily_percent(Some(f64::NAN)), "--% yday");
        assert_eq!(format_daily_percent(Some(f64::INFINITY)), "--% yday");
    }
}
