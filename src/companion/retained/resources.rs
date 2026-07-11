//! Glyph atlas resources for the retained companion renderer: complete glyph
//! metrics, Unicode scalar-sequence atlas keys, a resolved font policy, and
//! native-color (emoji) atlas entries.
//!
//! The rasterizer draws through `NSBitmapImageRep::initWithBitmapDataPlanes...`,
//! a different selector from the AppKit view-cache selectors the retained
//! capture boundary forbids, so glyph rasterization lives beside the retained
//! renderer here rather than in `capture.rs`.

use objc2::rc::Retained;
use objc2::ClassType;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
    NSFontAttributeName, NSFontWeightBold, NSForegroundColorAttributeName, NSGraphicsContext,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSString};

use super::RetainedFailureCategory;

/// Deterministic FNV-1a so a font-policy id is a stable hash of its inputs,
/// independent of the standard hasher's seeded internals.
struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    fn write_u8(&mut self, value: u8) {
        self.write(&[value]);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// A complete authored Unicode scalar sequence for one atlas cell. Cell, pet,
/// room, and prop glyphs arrive as whole authored sequences — a base plus a
/// combining mark, or a multi-scalar emoji — and stay whole; they are never
/// split with `chars()`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct GlyphSequence(String);

impl GlyphSequence {
    pub(super) fn new(sequence: impl Into<String>) -> Self {
        Self(sequence.into())
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Atlas key: one complete scalar sequence plus its weight. The whole sequence
/// is a single key — a composed grapheme is never split into per-scalar keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct GlyphKey {
    pub(super) sequence: GlyphSequence,
    pub(super) bold: bool,
}

impl GlyphKey {
    pub(super) fn new(sequence: impl Into<String>, bold: bool) -> Self {
        Self {
            sequence: GlyphSequence::new(sequence),
            bold,
        }
    }
}

/// How a rasterized glyph stores its pixels in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GlyphEntryKind {
    /// White RGB plus coverage alpha; the fragment tints it by the authored
    /// foreground color.
    Mask,
    /// Full premultiplied RGBA (an emoji or other native-color glyph); the
    /// fragment samples the color directly and bypasses the foreground tint.
    PremultipliedColorRgba,
}

/// The fragment path the shader takes for a glyph primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FragmentGlyphMode {
    Mask,
    NativeColor,
}

/// A complete glyph atlas entry: where the ink lives, its metrics relative to
/// the pen and baseline, the raster geometry it was measured in, the font policy
/// that produced it, and whether it is a coverage mask or native color.
#[derive(Debug, Clone, Copy)]
pub(super) struct GlyphAtlasEntry {
    /// Atlas UV rect of the ink bounds, or `None` for a whitespace / inkless
    /// glyph that still advances the pen.
    pub(super) visible_uv: Option<[f32; 4]>,
    /// Ink offset from the cell draw origin: `[horizontal_bearing,
    /// vertical_bearing]` in raster pixels (top-down from the cell origin).
    pub(super) ink_origin: [f32; 2],
    /// Ink bounds in raster pixels: `[width, height]`.
    pub(super) ink_size: [f32; 2],
    /// Typographic line height, in raster pixels.
    pub(super) line_height: f32,
    /// Pen advance for this glyph, in raster pixels.
    pub(super) advance: f32,
    /// Whether the stored pixels are a coverage mask or premultiplied color.
    pub(super) kind: GlyphEntryKind,
    // The remaining metrics complete the atlas entry the way glyph-parity work
    // (Tasks 9/12) will consume it; the native metric-coverage tests validate
    // them against attributed measurement, but the renderer does not read them
    // yet.
    /// Baseline row measured top-down from the cell top, in raster pixels.
    #[allow(dead_code)]
    pub(super) baseline: f32,
    /// Font ascent above the baseline, in raster pixels.
    #[allow(dead_code)]
    pub(super) ascent: f32,
    /// Font descent below the baseline (positive magnitude), in raster pixels.
    #[allow(dead_code)]
    pub(super) descent: f32,
    /// The raster cell the glyph was measured in: `[width, height]`.
    #[allow(dead_code)]
    pub(super) raster_size: [f32; 2],
    /// Safe padding reserved around the glyph inside its raster cell.
    #[allow(dead_code)]
    pub(super) safe_padding: f32,
    /// Id of the [`ResolvedFontPolicy`] this entry was rasterized under; a policy
    /// change changes the id so the atlas can be invalidated.
    #[allow(dead_code)]
    pub(super) font_policy_id: u64,
}

impl GlyphAtlasEntry {
    /// The fragment path the shader takes for this entry.
    pub(super) fn fragment_mode(&self) -> FragmentGlyphMode {
        match self.kind {
            GlyphEntryKind::Mask => FragmentGlyphMode::Mask,
            GlyphEntryKind::PremultipliedColorRgba => FragmentGlyphMode::NativeColor,
        }
    }

    /// A whitespace / inkless glyph: it advances the pen but has no visible quad.
    pub(super) fn whitespace(advance: f32, line_height: f32) -> Self {
        Self {
            visible_uv: None,
            ink_origin: [0.0, 0.0],
            ink_size: [0.0, 0.0],
            baseline: 0.0,
            ascent: 0.0,
            descent: 0.0,
            line_height,
            advance,
            raster_size: [0.0, 0.0],
            safe_padding: 0.0,
            font_policy_id: 0,
            kind: GlyphEntryKind::Mask,
        }
    }

    /// A fully populated entry for contract tests that need a visible glyph of a
    /// given kind without rasterizing a real font.
    #[cfg(test)]
    pub(super) fn fixture(kind: GlyphEntryKind) -> Self {
        Self {
            visible_uv: Some([0.0, 0.0, 1.0, 1.0]),
            ink_origin: [1.0, 2.0],
            ink_size: [10.0, 20.0],
            baseline: 40.0,
            ascent: 44.0,
            descent: 10.0,
            line_height: 52.0,
            advance: 29.0,
            raster_size: [80.0, 80.0],
            safe_padding: 6.0,
            font_policy_id: 1,
            kind,
        }
    }
}

/// The atlas cell layout revision. Bump when the packing layout changes so an id
/// carried by an older layout no longer matches.
pub(super) const ATLAS_PACKING_VERSION: u32 = 1;

/// The retained fragment shader revision. Version 1 adds the native-color glyph
/// path; the mask path is unchanged. A shader change bumps this so entries from
/// an older shader can be invalidated.
pub(super) const SHADER_RESOURCE_VERSION: u32 = 1;

/// Antialiasing the atlas rasterizes under. The retained atlas draws through the
/// default AppKit grayscale smoothing path, matching Smooth's on-screen text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AntialiasingPolicy {
    DefaultSmoothing,
}

/// The font weight the policy resolves. Regular and bold resolve to distinct
/// system fonts with distinct PostScript names and descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FontWeightPolicy {
    Regular,
    Bold,
}

impl FontWeightPolicy {
    pub(super) fn from_bold(bold: bool) -> Self {
        if bold {
            Self::Bold
        } else {
            Self::Regular
        }
    }

    fn ns_weight(self) -> f64 {
        match self {
            // SAFETY: `NSFontWeightBold` is an AppKit-provided weight constant.
            Self::Bold => unsafe { NSFontWeightBold },
            Self::Regular => 0.0,
        }
    }
}

/// The resolved identity of the exact Smooth monospaced font an atlas is built
/// against. A change to the font, its descriptor, size, scale, weight,
/// antialiasing, or resource versions changes [`ResolvedFontPolicy::id`], which
/// ties every entry to the policy that produced it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResolvedFontPolicy {
    postscript_name: String,
    descriptor_hash: u64,
    point_size: f64,
    backing_scale: f64,
    weight: FontWeightPolicy,
    antialiasing: AntialiasingPolicy,
    atlas_packing_version: u32,
    shader_resource_version: u32,
}

impl ResolvedFontPolicy {
    /// Resolves the exact Smooth monospaced font for a weight and reads its
    /// identity. Uses `NSFont::monospacedSystemFontOfSize_weight` — the same
    /// font policy Smooth's companion grid and HUD measure with — so the
    /// retained atlas rasterizes the glyphs Smooth lays out.
    pub(super) fn resolve(point_size: f64, backing_scale: f64, weight: FontWeightPolicy) -> Self {
        let font = resolve_font(point_size, weight);
        // SAFETY: `font` is a valid `NSFont`; `fontName`/`fontDescriptor` read
        // immutable identity.
        let postscript_name = unsafe { font.fontName() }.to_string();
        let descriptor_hash = descriptor_hash(&font);
        Self {
            postscript_name,
            descriptor_hash,
            point_size,
            backing_scale,
            weight,
            antialiasing: AntialiasingPolicy::DefaultSmoothing,
            atlas_packing_version: ATLAS_PACKING_VERSION,
            shader_resource_version: SHADER_RESOURCE_VERSION,
        }
    }

    /// A stable id every entry rasterized under this policy carries.
    pub(super) fn id(&self) -> u64 {
        let mut hasher = Fnv1a::new();
        hasher.write(self.postscript_name.as_bytes());
        hasher.write_u64(self.descriptor_hash);
        hasher.write_u64(self.point_size.to_bits());
        hasher.write_u64(self.backing_scale.to_bits());
        hasher.write_u8(match self.weight {
            FontWeightPolicy::Regular => 0,
            FontWeightPolicy::Bold => 1,
        });
        hasher.write_u8(match self.antialiasing {
            AntialiasingPolicy::DefaultSmoothing => 0,
        });
        hasher.write_u32(self.atlas_packing_version);
        hasher.write_u32(self.shader_resource_version);
        hasher.finish()
    }
}

/// Resolves the exact Smooth monospaced `NSFont` for a point size and weight.
fn resolve_font(point_size: f64, weight: FontWeightPolicy) -> Retained<NSFont> {
    // SAFETY: `monospacedSystemFontOfSize_weight` returns a retained font for
    // any positive size and valid weight.
    unsafe { NSFont::monospacedSystemFontOfSize_weight(point_size, weight.ns_weight()) }
}

/// A stable hash of the font's canonical descriptor identity: its PostScript
/// name, point size, and symbolic traits.
fn descriptor_hash(font: &NSFont) -> u64 {
    // SAFETY: `fontDescriptor` and its accessors read immutable font identity.
    let descriptor = unsafe { font.fontDescriptor() };
    let postscript = unsafe { descriptor.postscriptName() }
        .map(|name| name.to_string())
        .unwrap_or_default();
    let point_size = unsafe { descriptor.pointSize() };
    let traits = unsafe { descriptor.symbolicTraits() }.0;
    let mut hasher = Fnv1a::new();
    hasher.write(postscript.as_bytes());
    hasher.write_u64(point_size.to_bits());
    hasher.write_u32(traits);
    hasher.finish()
}

/// The raster geometry one glyph is drawn into: a square cell, the safe padding
/// reserved inside it, and the physical point size the font renders at.
pub(super) struct GlyphRasterTarget {
    pub(super) cell: u32,
    pub(super) padding: u32,
    pub(super) point_size: f64,
}

/// A premultiplied channel spread above this (out of 255) marks a pixel as
/// carrying native chroma rather than a gray coverage mask. Grayscale-antialiased
/// white text stays at spread zero; emoji cross it decisively.
const CHROMA_THRESHOLD: u8 = 16;

/// Rasterizes one glyph into `atlas` at `(x, y)` and returns its complete entry.
///
/// The glyph is drawn white so mask glyphs carry pure coverage. If the
/// non-transparent pixels contain native chroma (an emoji), the entry keeps the
/// premultiplied RGBA the bitmap produced and is classified
/// [`GlyphEntryKind::PremultipliedColorRgba`]; otherwise it is stored as white
/// RGB plus coverage alpha ([`GlyphEntryKind::Mask`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn rasterize_glyph_entry(
    key: &GlyphKey,
    target: &GlyphRasterTarget,
    font_policy_id: u64,
    atlas: &mut [u8],
    atlas_width: u32,
    atlas_height: u32,
    x: u32,
    y: u32,
) -> std::result::Result<GlyphAtlasEntry, RetainedFailureCategory> {
    let cell = target.cell;
    let padding = target.padding;
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), cell as isize, cell as isize,
            8, 4, true, false, NSDeviceRGBColorSpace, (cell * 4) as isize, 32,
        ).ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
            .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let text = NSString::from_str(key.sequence.as_str());
        let font = resolve_font(target.point_size, FontWeightPolicy::from_bold(key.bold));
        let mut attributed = NSMutableAttributedString::from_nsstring(&text);
        let range = NSRange::from(0..text.length());
        attributed.addAttribute_value_range(NSFontAttributeName, &font, range);
        let white = NSColor::whiteColor();
        attributed.addAttribute_value_range(NSForegroundColorAttributeName, &white, range);
        let attributed: Retained<objc2_foundation::NSAttributedString> =
            Retained::into_super(attributed);
        let size = attributed.size();
        if size.width + f64::from(padding * 2) > f64::from(cell)
            || size.height + f64::from(padding * 2) > f64::from(cell)
        {
            NSGraphicsContext::setCurrentContext(previous.as_deref());
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        let draw_x = f64::from(padding);
        let draw_y = f64::from(padding);
        attributed.drawAtPoint(NSPoint::new(draw_x, draw_y));
        context.flushGraphics();
        NSGraphicsContext::setCurrentContext(previous.as_deref());
        let data = rep.bitmapData();
        if data.is_null() {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }

        // First pass: copy the premultiplied bitmap into the atlas cell, tracking
        // ink bounds and whether any covered pixel carries native chroma.
        let mut ink_min_x = cell;
        let mut ink_min_y = cell;
        let mut ink_max_x = 0;
        let mut ink_max_y = 0;
        let mut has_ink = false;
        let mut has_chroma = false;
        for row in 0..cell {
            for col in 0..cell {
                let src = ((row * cell + col) * 4) as usize;
                let dst = (((y + row) * atlas_width + x + col) * 4) as usize;
                let r = *data.add(src);
                let g = *data.add(src + 1);
                let b = *data.add(src + 2);
                let alpha = *data.add(src + 3);
                atlas[dst..dst + 4].copy_from_slice(&[r, g, b, alpha]);
                if alpha != 0 {
                    has_ink = true;
                    ink_min_x = ink_min_x.min(col);
                    ink_min_y = ink_min_y.min(row);
                    ink_max_x = ink_max_x.max(col + 1);
                    ink_max_y = ink_max_y.max(row + 1);
                    let spread = r.max(g).max(b) - r.min(g).min(b);
                    if spread > CHROMA_THRESHOLD {
                        has_chroma = true;
                    }
                }
            }
        }

        // An inkless glyph (whitespace) still advances the pen but has no quad.
        if !has_ink {
            return Ok(GlyphAtlasEntry::whitespace(
                size.width as f32,
                size.height as f32,
            ));
        }

        let kind = if has_chroma {
            GlyphEntryKind::PremultipliedColorRgba
        } else {
            // A coverage mask ignores stored RGB in the shader, but forcing white
            // keeps the atlas legible and matches the historical mask layout.
            for row in 0..cell {
                for col in 0..cell {
                    let dst = (((y + row) * atlas_width + x + col) * 4) as usize;
                    atlas[dst] = 255;
                    atlas[dst + 1] = 255;
                    atlas[dst + 2] = 255;
                }
            }
            GlyphEntryKind::Mask
        };

        let ascent = font.ascender() as f32;
        let descent = -(font.descender() as f32);
        // Text is drawn with the layout box bottom at `draw_y`, so the baseline
        // sits `descent` above it; converted to top-down rows from the cell top.
        let baseline = cell as f32 - draw_y as f32 - descent;

        Ok(GlyphAtlasEntry {
            visible_uv: Some([
                (x + ink_min_x) as f32 / atlas_width as f32,
                (y + ink_min_y) as f32 / atlas_height as f32,
                (x + ink_max_x) as f32 / atlas_width as f32,
                (y + ink_max_y) as f32 / atlas_height as f32,
            ]),
            ink_origin: [
                ink_min_x as f32 - draw_x as f32,
                ink_min_y as f32 - draw_y as f32,
            ],
            ink_size: [
                (ink_max_x - ink_min_x) as f32,
                (ink_max_y - ink_min_y) as f32,
            ],
            baseline,
            ascent,
            descent,
            line_height: size.height as f32,
            advance: size.width as f32,
            raster_size: [cell as f32, cell as f32],
            safe_padding: padding as f32,
            font_policy_id,
            kind,
        })
    }
}

#[cfg(test)]
mod metric_tests {
    use super::*;

    /// One declared physical pixel of geometry tolerance.
    const TOL: f32 = 1.0;
    /// Baseline placement carries a hair more sub-pixel rounding than advance.
    const BASELINE_TOL: f32 = 2.0;

    /// The kind of coverage a case expects.
    #[derive(Clone, Copy, PartialEq)]
    enum Coverage {
        Whitespace,
        Mask,
        Color,
    }

    /// A raster geometry to cover: a square cell, its safe padding, and the
    /// physical point size — scale 1 (48pt/80px) and scale 2 (96pt/160px).
    const SCALES: &[(u32, u32, f64)] = &[(80, 6, 48.0), (160, 12, 96.0)];

    /// Ordinary, narrow, descender, bold, whitespace, replacement, composed
    /// mark, and bubble emoji.
    const CASES: &[(&str, bool, Coverage, &str)] = &[
        ("x", false, Coverage::Mask, "ordinary"),
        ("i", false, Coverage::Mask, "narrow"),
        ("g", false, Coverage::Mask, "descender"),
        ("x", true, Coverage::Mask, "bold"),
        (" ", false, Coverage::Whitespace, "whitespace"),
        ("\u{fffd}", false, Coverage::Mask, "replacement"),
        ("e\u{0301}", false, Coverage::Mask, "composed"),
        ("🫧", false, Coverage::Color, "bubble-emoji"),
    ];

    fn rasterize(
        text: &str,
        bold: bool,
        cell: u32,
        padding: u32,
        point_size: f64,
    ) -> GlyphAtlasEntry {
        let key = GlyphKey::new(text, bold);
        let target = GlyphRasterTarget { cell, padding, point_size };
        let mut atlas = vec![0_u8; (cell * cell * 4) as usize];
        rasterize_glyph_entry(&key, &target, 7, &mut atlas, cell, cell, 0, 0)
            .expect("offscreen glyph rasterization succeeds")
    }

    /// The independent attributed Smooth measurement for a glyph: advance and
    /// line height from a fresh `NSAttributedString` with the same font.
    fn reference_size(text: &str, point_size: f64, bold: bool) -> (f32, f32) {
        unsafe {
            let ns = NSString::from_str(text);
            let font = resolve_font(point_size, FontWeightPolicy::from_bold(bold));
            let mut attributed = NSMutableAttributedString::from_nsstring(&ns);
            let range = NSRange::from(0..ns.length());
            attributed.addAttribute_value_range(NSFontAttributeName, &font, range);
            let attributed: Retained<objc2_foundation::NSAttributedString> =
                Retained::into_super(attributed);
            let size = attributed.size();
            (size.width as f32, size.height as f32)
        }
    }

    #[test]
    fn native_metrics_match_attributed_measurement_across_backing_scales() {
        for &(cell, padding, point) in SCALES {
            for &(text, bold, coverage, label) in CASES {
                let entry = rasterize(text, bold, cell, padding, point);
                let (ref_width, ref_height) = reference_size(text, point, bold);

                assert!(
                    (entry.advance - ref_width).abs() <= TOL,
                    "{label}@{cell}: advance {} vs attributed {ref_width}",
                    entry.advance,
                );
                assert!(
                    (entry.line_height - ref_height).abs() <= TOL,
                    "{label}@{cell}: line_height {} vs attributed {ref_height}",
                    entry.line_height,
                );
                assert!(entry.advance > 0.0, "{label}@{cell}: advances the pen");

                match coverage {
                    Coverage::Whitespace => {
                        assert_eq!(entry.visible_uv, None, "{label}@{cell}: no visible quad");
                        assert_eq!(entry.kind, GlyphEntryKind::Mask);
                        assert_eq!(entry.fragment_mode(), FragmentGlyphMode::Mask);
                    }
                    Coverage::Mask | Coverage::Color => {
                        assert_eq!(entry.raster_size, [cell as f32, cell as f32]);
                        assert_eq!(entry.safe_padding, padding as f32);
                        assert!(entry.visible_uv.is_some(), "{label}@{cell}: visible");
                        assert!(
                            entry.ink_size[0] > 0.0 && entry.ink_size[1] > 0.0,
                            "{label}@{cell}: has ink",
                        );
                        // Physical ink bounds are contained within the raster cell.
                        let left = entry.ink_origin[0] + entry.safe_padding;
                        let top = entry.ink_origin[1] + entry.safe_padding;
                        let right = left + entry.ink_size[0];
                        let bottom = top + entry.ink_size[1];
                        assert!(
                            left >= -TOL && top >= -TOL,
                            "{label}@{cell}: ink within cell top-left ({left}, {top})",
                        );
                        assert!(
                            right <= cell as f32 + TOL && bottom <= cell as f32 + TOL,
                            "{label}@{cell}: ink within cell bottom-right ({right}, {bottom})",
                        );
                        assert!(
                            entry.ascent > 0.0 && entry.descent > 0.0,
                            "{label}@{cell}: carries font ascent/descent",
                        );
                        let (expected_kind, expected_mode) = if coverage == Coverage::Color {
                            (
                                GlyphEntryKind::PremultipliedColorRgba,
                                FragmentGlyphMode::NativeColor,
                            )
                        } else {
                            (GlyphEntryKind::Mask, FragmentGlyphMode::Mask)
                        };
                        assert_eq!(entry.kind, expected_kind, "{label}@{cell}: kind");
                        assert_eq!(entry.fragment_mode(), expected_mode, "{label}@{cell}: mode");
                    }
                }
            }
        }
    }

    #[test]
    fn baseline_places_x_height_on_it_and_descenders_below_it() {
        for &(cell, padding, point) in SCALES {
            let x = rasterize("x", false, cell, padding, point);
            let g = rasterize("g", false, cell, padding, point);
            let ink_bottom = |entry: &GlyphAtlasEntry| {
                entry.ink_origin[1] + entry.safe_padding + entry.ink_size[1]
            };
            let x_bottom = ink_bottom(&x);
            let g_bottom = ink_bottom(&g);
            assert!(
                (x_bottom - x.baseline).abs() <= BASELINE_TOL,
                "x-height rests on the baseline @{cell}: bottom {x_bottom} vs baseline {}",
                x.baseline,
            );
            assert!(
                g_bottom > g.baseline + BASELINE_TOL,
                "descender drops below the baseline @{cell}: bottom {g_bottom} vs baseline {}",
                g.baseline,
            );
        }
    }

    #[test]
    fn composed_mark_stays_one_key_and_rises_above_the_base() {
        // A base plus a combining acute is one whole authored sequence.
        assert_eq!(
            GlyphKey::new("e\u{0301}", false).sequence.as_str(),
            "e\u{0301}"
        );
        let composed = rasterize("e\u{0301}", false, 80, 6, 48.0);
        let plain = rasterize("e", false, 80, 6, 48.0);
        assert!(composed.visible_uv.is_some());
        assert!(
            composed.ink_origin[1] < plain.ink_origin[1],
            "the combining mark rises above the base ink: {} < {}",
            composed.ink_origin[1],
            plain.ink_origin[1],
        );
    }
}

#[cfg(test)]
mod glyph_tests {
    use super::{FragmentGlyphMode, GlyphAtlasEntry, GlyphEntryKind, GlyphKey};

    #[test]
    fn scalar_sequence_is_one_atlas_key() {
        let key = GlyphKey::new("ö", false);
        assert_eq!(key.sequence.as_str(), "ö");
    }

    #[test]
    fn color_entry_bypasses_foreground_tint() {
        let entry = GlyphAtlasEntry::fixture(GlyphEntryKind::PremultipliedColorRgba);
        assert_eq!(entry.fragment_mode(), FragmentGlyphMode::NativeColor);
    }

    #[test]
    fn whitespace_keeps_advance_without_visible_uv() {
        let entry = GlyphAtlasEntry::whitespace(24.0, 52.0);
        assert_eq!(entry.advance, 24.0);
        assert_eq!(entry.visible_uv, None);
    }
}
