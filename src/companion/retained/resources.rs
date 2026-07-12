//! Glyph atlas resources for the retained companion renderer: complete glyph
//! metrics, Unicode scalar-sequence atlas keys, a resolved font policy, and
//! native-color (emoji) atlas entries.
//!
//! The rasterizer draws through `NSBitmapImageRep::initWithBitmapDataPlanes...`,
//! a different selector from the AppKit view-cache selectors the retained
//! capture boundary forbids, so glyph rasterization lives beside the retained
//! renderer here rather than in `capture.rs`.

use std::collections::BTreeMap;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::ClassType;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
    NSFontAttributeName, NSFontWeightBold, NSForegroundColorAttributeName, NSGraphicsContext,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSString};

use super::RetainedFailureCategory;
use crate::round::smooth::{
    collect_companion_glyph_repertoire, CompanionContentIdentity, RepertoireGlyph,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RasterItemPhases {
    pub(super) scratch_setup: Duration,
    pub(super) text_setup_measure: Duration,
    pub(super) draw_flush: Duration,
    pub(super) pixel_copy_classify: Duration,
    pub(super) mask_normalize_finalize: Duration,
}

impl RasterItemPhases {
    #[cfg(test)]
    pub(super) fn total(self) -> Duration {
        self.scratch_setup
            .saturating_add(self.text_setup_measure)
            .saturating_add(self.draw_flush)
            .saturating_add(self.pixel_copy_classify)
            .saturating_add(self.mask_normalize_finalize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RasterItemProgress {
    elapsed: Duration,
    phases: RasterItemPhases,
}

impl RasterItemProgress {
    #[cfg(test)]
    fn unclassified(elapsed: Duration) -> Self {
        Self {
            elapsed,
            phases: RasterItemPhases::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RasterSliceProgress {
    pub(super) start_cursor: usize,
    pub(super) end_cursor: usize,
    pub(super) completed_items: usize,
    pub(super) complete: bool,
    pub(super) deadline_missed: bool,
    pub(super) elapsed: Duration,
    pub(super) max_item_elapsed: Duration,
    pub(super) max_item_index: Option<usize>,
    pub(super) max_item_phases: RasterItemPhases,
}

fn run_resumable_slice<E>(
    cursor: &mut usize,
    item_count: usize,
    work_start_budget: Duration,
    hard_deadline: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut work: impl FnMut(usize) -> Result<RasterItemProgress, E>,
) -> Result<RasterSliceProgress, E> {
    let start_cursor = *cursor;
    let mut completed_items = 0;
    let mut max_item_elapsed = Duration::ZERO;
    let mut max_item_index = None;
    let mut max_item_phases = RasterItemPhases::default();
    let mut observed_elapsed = elapsed();
    while *cursor < item_count && observed_elapsed < work_start_budget {
        let index = *cursor;
        let item = work(index)?;
        *cursor += 1;
        completed_items += 1;
        observed_elapsed = elapsed();
        if max_item_index.is_none() || item.elapsed > max_item_elapsed {
            max_item_elapsed = item.elapsed;
            max_item_index = Some(index);
            max_item_phases = item.phases;
        }
        if observed_elapsed >= work_start_budget {
            break;
        }
    }
    Ok(RasterSliceProgress {
        start_cursor,
        end_cursor: *cursor,
        completed_items,
        complete: *cursor == item_count,
        deadline_missed: completed_items > 0 && observed_elapsed > hard_deadline,
        elapsed: observed_elapsed,
        max_item_elapsed,
        max_item_index,
        max_item_phases,
    })
}

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

    /// A deterministic visible entry for capacity inventory and contract tests.
    /// Primitive-count inventory needs key occupancy, not native font geometry;
    /// using this avoids hidden AppKit raster work in the evidence callback.
    pub(super) fn synthetic_visible(kind: GlyphEntryKind) -> Self {
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
/// path; the mask path is unchanged. Version 2 replaces the hard aperture/ellipse/
/// arc/round-cap edge discards with analytic `fwidth`/smoothstep coverage. Version
/// 3 switches the fragment output to gamma-space (sRGB) premultiplied color for a
/// linear-format render target and native-color atlas, so translucency composites
/// in gamma to match Smooth. A shader change bumps this so entries from an older
/// shader can be invalidated.
pub(super) const SHADER_RESOURCE_VERSION: u32 = 3;

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

struct ProfiledGlyphEntry {
    entry: GlyphAtlasEntry,
    phases: RasterItemPhases,
}

/// Rasterizes one glyph into `atlas` at `(x, y)` and returns its complete entry.
///
/// The glyph is drawn white so mask glyphs carry pure coverage. If the
/// non-transparent pixels contain native chroma (an emoji), the entry keeps the
/// premultiplied RGBA the bitmap produced and is classified
/// [`GlyphEntryKind::PremultipliedColorRgba`]; otherwise it is stored as white
/// RGB plus coverage alpha ([`GlyphEntryKind::Mask`]).
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
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
    rasterize_glyph_entry_profiled(
        key,
        target,
        font_policy_id,
        atlas,
        atlas_width,
        atlas_height,
        x,
        y,
    )
    .map(|profiled| profiled.entry)
}

#[allow(clippy::too_many_arguments)]
fn rasterize_glyph_entry_profiled(
    key: &GlyphKey,
    target: &GlyphRasterTarget,
    font_policy_id: u64,
    atlas: &mut [u8],
    atlas_width: u32,
    atlas_height: u32,
    x: u32,
    y: u32,
) -> std::result::Result<ProfiledGlyphEntry, RetainedFailureCategory> {
    let cell = target.cell;
    let padding = target.padding;
    unsafe {
        let phase_started_at = std::time::Instant::now();
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), cell as isize, cell as isize,
            8, 4, true, false, NSDeviceRGBColorSpace, (cell * 4) as isize, 32,
        ).ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
            .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let scratch_setup = phase_started_at.elapsed();

        let phase_started_at = std::time::Instant::now();
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
        let text_setup_measure = phase_started_at.elapsed();

        let phase_started_at = std::time::Instant::now();
        let draw_x = f64::from(padding);
        let draw_y = f64::from(padding);
        attributed.drawAtPoint(NSPoint::new(draw_x, draw_y));
        context.flushGraphics();
        NSGraphicsContext::setCurrentContext(previous.as_deref());
        let draw_flush = phase_started_at.elapsed();

        let phase_started_at = std::time::Instant::now();
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
        let pixel_copy_classify = phase_started_at.elapsed();

        let phase_started_at = std::time::Instant::now();

        // An inkless glyph (whitespace) still advances the pen but has no quad.
        if !has_ink {
            let entry = GlyphAtlasEntry::whitespace(size.width as f32, size.height as f32);
            let mask_normalize_finalize = phase_started_at.elapsed();
            return Ok(ProfiledGlyphEntry {
                entry,
                phases: RasterItemPhases {
                    scratch_setup,
                    text_setup_measure,
                    draw_flush,
                    pixel_copy_classify,
                    mask_normalize_finalize,
                },
            });
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

        let entry = GlyphAtlasEntry {
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
        };
        let mask_normalize_finalize = phase_started_at.elapsed();
        Ok(ProfiledGlyphEntry {
            entry,
            phases: RasterItemPhases {
                scratch_setup,
                text_setup_measure,
                draw_flush,
                pixel_copy_classify,
                mask_normalize_finalize,
            },
        })
    }
}

/// Inner drawable box as a multiple of the point size, inside the safe padding.
/// 1.5 comfortably clears the widest glyph (the bubble emoji measures ~1.42× the
/// point size) at every size, so the full repertoire fits even the smallest
/// matrix cell.
const ATLAS_INNER_RATIO: f64 = 1.5;
/// Safe padding as a multiple of the point size. At the 48pt device font this is
/// a 4px margin inside an 80px cell — the historical cell edge.
const ATLAS_PADDING_RATIO: f64 = 0.08;
/// Glyphs per atlas row.
const ATLAS_COLUMNS: u32 = 16;
/// Row ceiling for the packed atlas. Raised above the pre-preflight 16 rows so
/// the full declared repertoire (pet × room × props × tank life × ambient × HUD
/// × chrome, both weights) fits; a repertoire that overflows this fails the
/// preflight rather than silently rebuilding per frame.
const MAX_ATLAS_ROWS: u32 = 40;

/// The effect and chrome glyphs the retained renderer guarantees regardless of
/// pet content: the Unicode replacement glyph a corrupt/unmappable scalar falls
/// back to, the chest bubble emoji, and a composed-mark representative (kept in
/// both precomposed and combining forms so a query in either normalization
/// resolves).
const CHROME_GLYPHS: &[&str] = &["\u{fffd}", "\u{1fae7}", "\u{00f6}", "o\u{308}"];

/// The point size a production companion atlas rasterizes at — the historical
/// device font size. The renderer's glyph-quad scale divides the display font
/// size by this, so keeping it fixed preserves the on-screen glyph geometry
/// while the atlas now holds the full preflighted repertoire.
pub(super) const RETAINED_ATLAS_POINT_SIZE: f64 = 48.0;

/// The logical point size the atlas rasterizes at for a companion window of
/// `logical_size` points. The shipping 960pt device resolves to the historical
/// 48pt font; smaller and larger windows scale proportionally. Used by the
/// size/scale test matrix; production uses the fixed [`RETAINED_ATLAS_POINT_SIZE`].
#[cfg(test)]
fn atlas_point_size_for_logical(logical_size: u32) -> f64 {
    (f64::from(logical_size) * 48.0 / 960.0).max(10.0)
}

/// A rasterized glyph atlas for one resource generation: the packed RGBA pixels
/// and the per-key metric entries. Rasterized at the manifest's fixed production
/// point size, which the renderer's glyph scale divides the display font size by.
pub(super) struct CompiledGlyphAtlas {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Vec<u8>,
    pub(super) entries: BTreeMap<GlyphKey, GlyphAtlasEntry>,
}

struct GlyphAtlasPreparation {
    glyphs: Vec<GlyphKey>,
    target: GlyphRasterTarget,
    regular_font_policy_id: u64,
    bold_font_policy_id: u64,
    next_index: usize,
    atlas: CompiledGlyphAtlas,
}

impl GlyphAtlasPreparation {
    fn new(
        glyphs: &[GlyphKey],
        point_size: f64,
        backing_scale: f64,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let padding = (point_size * ATLAS_PADDING_RATIO).round().max(1.0) as u32;
        let inner = (point_size * ATLAS_INNER_RATIO).round().max(1.0) as u32;
        let cell = inner + 2 * padding;
        let count = glyphs.len().max(1) as u32;
        let rows = count.div_ceil(ATLAS_COLUMNS);
        if rows > MAX_ATLAS_ROWS {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        let width = ATLAS_COLUMNS * cell;
        let height = rows * cell;
        let target = GlyphRasterTarget { cell, padding, point_size };
        let regular_font_policy_id =
            ResolvedFontPolicy::resolve(point_size, backing_scale, FontWeightPolicy::Regular).id();
        let bold_font_policy_id =
            ResolvedFontPolicy::resolve(point_size, backing_scale, FontWeightPolicy::Bold).id();
        Ok(Self {
            glyphs: glyphs.to_vec(),
            target,
            regular_font_policy_id,
            bold_font_policy_id,
            next_index: 0,
            atlas: CompiledGlyphAtlas {
                width,
                height,
                rgba: vec![0_u8; (width * height * 4) as usize],
                entries: BTreeMap::new(),
            },
        })
    }

    fn advance(
        &mut self,
        work_start_budget: Duration,
        hard_deadline: Duration,
    ) -> std::result::Result<RasterSliceProgress, RetainedFailureCategory> {
        let started_at = std::time::Instant::now();
        let glyphs = &self.glyphs;
        let target = &self.target;
        let regular_font_policy_id = self.regular_font_policy_id;
        let bold_font_policy_id = self.bold_font_policy_id;
        let atlas = &mut self.atlas;
        run_resumable_slice(
            &mut self.next_index,
            glyphs.len(),
            work_start_budget,
            hard_deadline,
            || started_at.elapsed(),
            |index| {
                let item_started_at = std::time::Instant::now();
                let key = &glyphs[index];
                let slot_x = index as u32 % ATLAS_COLUMNS;
                let slot_y = index as u32 / ATLAS_COLUMNS;
                let font_policy_id = if key.bold {
                    bold_font_policy_id
                } else {
                    regular_font_policy_id
                };
                let profiled = rasterize_glyph_entry_profiled(
                    key,
                    target,
                    font_policy_id,
                    &mut atlas.rgba,
                    atlas.width,
                    atlas.height,
                    slot_x * target.cell,
                    slot_y * target.cell,
                )?;
                atlas.entries.insert(key.clone(), profiled.entry);
                let item_elapsed = item_started_at.elapsed();
                Ok(RasterItemProgress {
                    elapsed: item_elapsed,
                    phases: profiled.phases,
                })
            },
        )
    }

    fn finish(self) -> std::result::Result<CompiledGlyphAtlas, RetainedFailureCategory> {
        if self.next_index != self.glyphs.len() {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        Ok(self.atlas)
    }
}

/// A deterministic hash of a companion's declared-content identity, its full
/// glyph repertoire, and the resolved font policies (which fold in point size,
/// backing scale, atlas packing version, and shader resource version). The
/// retained atlas rebuilds only when this changes — never on a per-frame glyph
/// set change, and never on the per-minute room reshuffle (which changes only
/// which repertoire glyph is painted, not the identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResourceGenerationKey(u64);

impl ResourceGenerationKey {
    fn compute(manifest: &GlyphRepertoireManifest) -> Self {
        let mut hasher = Fnv1a::new();
        hasher.write(&manifest.identity.identity_bytes());
        for key in &manifest.glyphs {
            hasher.write(key.sequence.as_str().as_bytes());
            hasher.write_u8(u8::from(key.bold));
            hasher.write_u8(0xff);
        }
        let regular = ResolvedFontPolicy::resolve(
            manifest.atlas_point_size,
            manifest.backing_scale,
            FontWeightPolicy::Regular,
        )
        .id();
        let bold = ResolvedFontPolicy::resolve(
            manifest.atlas_point_size,
            manifest.backing_scale,
            FontWeightPolicy::Bold,
        )
        .id();
        hasher.write_u64(regular);
        hasher.write_u64(bold);
        Self(hasher.finish())
    }

    pub(super) fn value(self) -> u64 {
        self.0
    }
}

/// The full glyph repertoire a companion could ever paint, plus the atlas
/// geometry to rasterize it at. Built from the backend-neutral declared-content
/// collector and the retained chrome glyphs, then sorted and deduplicated into a
/// stable key list.
pub(super) struct GlyphRepertoireManifest {
    identity: CompanionContentIdentity,
    glyphs: Vec<GlyphKey>,
    atlas_point_size: f64,
    backing_scale: f64,
}

impl GlyphRepertoireManifest {
    fn from_repertoire(
        identity: CompanionContentIdentity,
        repertoire: Vec<RepertoireGlyph>,
        atlas_point_size: f64,
        backing_scale: f64,
    ) -> Self {
        let mut glyphs: Vec<GlyphKey> = repertoire
            .into_iter()
            .map(|glyph| GlyphKey::new(glyph.sequence, glyph.bold))
            .collect();
        // Chrome/effect glyphs, both weights — a fallback replacement can land in
        // a bold cell just like any other glyph.
        for chrome in CHROME_GLYPHS {
            glyphs.push(GlyphKey::new(*chrome, false));
            glyphs.push(GlyphKey::new(*chrome, true));
        }
        glyphs.sort();
        glyphs.dedup();
        Self {
            identity,
            glyphs,
            atlas_point_size,
            backing_scale,
        }
    }

    /// The manifest for the active pet: its one species across every stage/state,
    /// its room dialect, and every earnable prop/tank-life glyph. Production uses
    /// the fixed [`RETAINED_ATLAS_POINT_SIZE`] so on-screen glyph geometry is
    /// unchanged; the backing scale still feeds the generation key.
    pub(super) fn for_active_pet(identity: CompanionContentIdentity, backing_scale: f64) -> Self {
        let repertoire = collect_companion_glyph_repertoire(&identity);
        Self::from_repertoire(
            identity,
            repertoire,
            RETAINED_ATLAS_POINT_SIZE,
            backing_scale,
        )
    }

    /// The full-cast fixture manifest (every species) at a device-typical size —
    /// the strongest repertoire, a superset of any single pet's.
    #[cfg(test)]
    pub(super) fn for_fixture_pet() -> Self {
        Self::for_fixture_pet_at(960, 2.0)
    }

    /// The full-cast fixture manifest at an explicit logical size and backing
    /// scale, so the size/scale matrix can exercise distinct generations.
    #[cfg(test)]
    pub(super) fn for_fixture_pet_at(logical_size: u32, backing_scale: f64) -> Self {
        let identity = CompanionContentIdentity::all_species();
        let repertoire = collect_companion_glyph_repertoire(&identity);
        Self::from_repertoire(
            identity,
            repertoire,
            atlas_point_size_for_logical(logical_size),
            backing_scale,
        )
    }

    /// Whether the repertoire contains a glyph with exactly this scalar sequence
    /// (in either weight).
    #[cfg(test)]
    pub(super) fn contains_sequence(&self, sequence: &str) -> bool {
        self.glyphs
            .iter()
            .any(|key| key.sequence.as_str() == sequence)
    }

    pub(super) fn glyphs(&self) -> &[GlyphKey] {
        &self.glyphs
    }

    pub(super) fn generation_key(&self) -> ResourceGenerationKey {
        ResourceGenerationKey::compute(self)
    }
}

/// Every atlas resource for one resource generation: the compiled glyph atlas and
/// the generation key it was built for. Compiled in full before it replaces an
/// active generation, so a failed compile can leave the previous generation in
/// place.
pub(super) struct CompiledRetainedResources {
    generation: ResourceGenerationKey,
    atlas: CompiledGlyphAtlas,
}

pub(super) struct CompiledRetainedResourcesPreparation {
    generation: ResourceGenerationKey,
    atlas: GlyphAtlasPreparation,
}

impl CompiledRetainedResourcesPreparation {
    pub(super) fn new(
        manifest: &GlyphRepertoireManifest,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        Ok(Self {
            generation: manifest.generation_key(),
            atlas: GlyphAtlasPreparation::new(
                manifest.glyphs(),
                manifest.atlas_point_size,
                manifest.backing_scale,
            )?,
        })
    }

    pub(super) fn advance(
        &mut self,
        work_start_budget: Duration,
        hard_deadline: Duration,
    ) -> std::result::Result<RasterSliceProgress, RetainedFailureCategory> {
        self.atlas.advance(work_start_budget, hard_deadline)
    }

    pub(super) fn finish(
        self,
    ) -> std::result::Result<CompiledRetainedResources, RetainedFailureCategory> {
        Ok(CompiledRetainedResources {
            generation: self.generation,
            atlas: self.atlas.finish()?,
        })
    }
}

impl CompiledRetainedResources {
    /// Compiles the manifest's full repertoire into an atlas. On overflow or a
    /// glyph that does not fit, returns the failure category so the caller can
    /// retain the previous generation and fall back.
    #[cfg(test)]
    pub(super) fn compile(
        manifest: &GlyphRepertoireManifest,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let mut preparation = CompiledRetainedResourcesPreparation::new(manifest)?;
        let progress = preparation.advance(Duration::MAX, Duration::MAX)?;
        if !progress.complete {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        preparation.finish()
    }

    pub(super) fn for_capacity_inventory(manifest: &GlyphRepertoireManifest) -> Self {
        let entries = manifest
            .glyphs()
            .iter()
            .cloned()
            .map(|key| {
                let entry = if key.sequence.as_str().chars().all(char::is_whitespace) {
                    GlyphAtlasEntry::whitespace(29.0, 52.0)
                } else {
                    GlyphAtlasEntry::synthetic_visible(GlyphEntryKind::Mask)
                };
                (key, entry)
            })
            .collect();
        Self {
            generation: ResourceGenerationKey(0),
            atlas: CompiledGlyphAtlas {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
                entries,
            },
        }
    }

    pub(super) fn generation(&self) -> ResourceGenerationKey {
        self.generation
    }

    pub(super) fn atlas(&self) -> &CompiledGlyphAtlas {
        &self.atlas
    }

    /// Whether the compiled atlas holds this glyph. A frame that asks for a glyph
    /// not present is an atlas miss — which the churn contract forbids after
    /// activation from the full repertoire.
    #[cfg(test)]
    pub(super) fn contains(&self, key: &GlyphKey) -> bool {
        self.atlas.entries.contains_key(key)
    }
}

/// GPU resource-lifecycle counters for the retained host. Every field records
/// how many times the host actually created a GPU object or wrote instance data,
/// so a caller can snapshot the counters, run a stretch of frames, and prove the
/// steady state created nothing: `host.counters() - before` yields the delta.
///
/// The atlas fields carry the post-activation churn contract: a companion
/// activated from its full declared repertoire runs any animation with
/// `atlas_builds_after_activation`, `atlas_uploads_after_activation`, and
/// `atlas_misses` all zero — the atlas is compiled once at the generation change
/// and never rebuilt, re-uploaded, or missed per frame. The creation counters
/// carry the bounded-buffer contract: after warmup, ordinary motion writes
/// instances but creates no buffers, textures, samplers, bind groups, or
/// pipelines, and performs no static uploads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RetainedResourceCounters {
    pub(super) atlas_builds_after_activation: u32,
    pub(super) atlas_uploads_after_activation: u32,
    pub(super) atlas_misses: u32,
    /// GPU buffers created (persistent instance-ring growth and the capture
    /// staging buffer).
    pub(super) buffer_creations: u32,
    /// GPU textures created (the glyph atlas and the capture intermediate).
    pub(super) texture_creations: u32,
    /// GPU samplers created (the glyph atlas sampler).
    pub(super) sampler_creations: u32,
    /// GPU bind groups created (the glyph atlas bind group).
    pub(super) bind_group_creations: u32,
    /// GPU render pipelines created (one per blend mode at host construction).
    pub(super) pipeline_creations: u32,
    /// Static texture uploads performed (the glyph atlas upload at generation
    /// activation), never per frame.
    pub(super) static_uploads: u32,
    /// Instance-buffer writes performed — one per prepared frame in steady
    /// state.
    pub(super) instance_writes: u32,
    /// Total bytes written into instance buffers across every `instance_writes`.
    pub(super) instance_write_bytes: u64,
}

impl std::ops::Sub for RetainedResourceCounters {
    type Output = Self;

    /// Field-wise delta of two snapshots. Saturating so a delta never wraps if a
    /// caller subtracts snapshots out of order.
    fn sub(self, earlier: Self) -> Self {
        Self {
            atlas_builds_after_activation: self
                .atlas_builds_after_activation
                .saturating_sub(earlier.atlas_builds_after_activation),
            atlas_uploads_after_activation: self
                .atlas_uploads_after_activation
                .saturating_sub(earlier.atlas_uploads_after_activation),
            atlas_misses: self.atlas_misses.saturating_sub(earlier.atlas_misses),
            buffer_creations: self
                .buffer_creations
                .saturating_sub(earlier.buffer_creations),
            texture_creations: self
                .texture_creations
                .saturating_sub(earlier.texture_creations),
            sampler_creations: self
                .sampler_creations
                .saturating_sub(earlier.sampler_creations),
            bind_group_creations: self
                .bind_group_creations
                .saturating_sub(earlier.bind_group_creations),
            pipeline_creations: self
                .pipeline_creations
                .saturating_sub(earlier.pipeline_creations),
            static_uploads: self.static_uploads.saturating_sub(earlier.static_uploads),
            instance_writes: self.instance_writes.saturating_sub(earlier.instance_writes),
            instance_write_bytes: self
                .instance_write_bytes
                .saturating_sub(earlier.instance_write_bytes),
        }
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
    use super::*;

    #[test]
    fn raster_slice_stops_at_soft_cutoff_and_resumes_at_exact_next_item() {
        let mut cursor = 0;
        let mut visited = Vec::new();
        let mut times = [
            std::time::Duration::ZERO,
            std::time::Duration::from_micros(1_000),
            std::time::Duration::from_micros(1_500),
        ]
        .into_iter();
        let first = run_resumable_slice(
            &mut cursor,
            4,
            std::time::Duration::from_micros(1_500),
            std::time::Duration::from_micros(4_000),
            || times.next().expect("clock sample"),
            |index| {
                visited.push(index);
                Ok::<_, ()>(RasterItemProgress::unclassified(std::time::Duration::ZERO))
            },
        )
        .unwrap();
        assert_eq!(visited, [0, 1]);
        assert_eq!(cursor, 2);
        assert_eq!(first.completed_items, 2);
        assert!(!first.complete);
        assert!(!first.deadline_missed);

        let mut times = [
            std::time::Duration::ZERO,
            std::time::Duration::from_micros(1_000),
            std::time::Duration::from_micros(2_000),
        ]
        .into_iter();
        let second = run_resumable_slice(
            &mut cursor,
            4,
            std::time::Duration::from_micros(1_500),
            std::time::Duration::from_micros(4_000),
            || times.next().expect("clock sample"),
            |index| {
                visited.push(index);
                Ok::<_, ()>(RasterItemProgress::unclassified(std::time::Duration::ZERO))
            },
        )
        .unwrap();
        assert_eq!(visited, [0, 1, 2, 3]);
        assert_eq!(cursor, 4);
        assert_eq!(second.completed_items, 2);
        assert!(second.complete);
        assert!(!second.deadline_missed);
    }

    #[test]
    fn crossing_soft_cutoff_below_hard_deadline_is_not_a_deadline_miss() {
        let mut cursor = 0;
        let mut visited = Vec::new();
        let mut times = [
            std::time::Duration::ZERO,
            std::time::Duration::from_micros(1_400),
            std::time::Duration::from_micros(1_800),
        ]
        .into_iter();
        let slice = run_resumable_slice(
            &mut cursor,
            3,
            std::time::Duration::from_micros(1_500),
            std::time::Duration::from_micros(4_000),
            || times.next().expect("clock sample"),
            |index| {
                visited.push(index);
                Ok::<_, ()>(RasterItemProgress::unclassified(
                    std::time::Duration::from_micros(400),
                ))
            },
        )
        .unwrap();
        assert_eq!(visited, [0, 1]);
        assert_eq!(cursor, 2);
        assert_eq!(slice.completed_items, 2);
        assert!(!slice.complete);
        assert!(!slice.deadline_missed);
    }

    #[test]
    fn one_nonpreemptible_item_overrun_records_miss_and_yields_before_next_item() {
        let mut cursor = 0;
        let mut visited = Vec::new();
        let mut times = [
            std::time::Duration::ZERO,
            std::time::Duration::from_micros(4_001),
        ]
        .into_iter();
        let slice = run_resumable_slice(
            &mut cursor,
            3,
            std::time::Duration::from_micros(1_500),
            std::time::Duration::from_micros(4_000),
            || times.next().expect("clock sample"),
            |index| {
                visited.push(index);
                Ok::<_, ()>(RasterItemProgress::unclassified(
                    std::time::Duration::from_micros(4_001),
                ))
            },
        )
        .unwrap();
        assert_eq!(visited, [0]);
        assert_eq!(cursor, 1);
        assert_eq!(slice.completed_items, 1);
        assert!(!slice.complete);
        assert!(slice.deadline_missed);
    }

    #[test]
    fn raster_slice_reports_cursor_range_and_actual_item_time() {
        let mut cursor = 0;
        let mut lane_times = [
            std::time::Duration::from_micros(100),
            std::time::Duration::from_micros(1_200),
            std::time::Duration::from_micros(2_000),
        ]
        .into_iter();
        let item_progress = [
            RasterItemProgress {
                elapsed: std::time::Duration::from_micros(900),
                phases: RasterItemPhases {
                    scratch_setup: std::time::Duration::from_micros(100),
                    text_setup_measure: std::time::Duration::from_micros(200),
                    draw_flush: std::time::Duration::from_micros(300),
                    pixel_copy_classify: std::time::Duration::from_micros(150),
                    mask_normalize_finalize: std::time::Duration::from_micros(100),
                },
            },
            RasterItemProgress {
                elapsed: std::time::Duration::from_micros(500),
                phases: RasterItemPhases {
                    scratch_setup: std::time::Duration::from_micros(50),
                    text_setup_measure: std::time::Duration::from_micros(100),
                    draw_flush: std::time::Duration::from_micros(150),
                    pixel_copy_classify: std::time::Duration::from_micros(100),
                    mask_normalize_finalize: std::time::Duration::from_micros(50),
                },
            },
        ];

        let slice = run_resumable_slice(
            &mut cursor,
            2,
            std::time::Duration::from_micros(1_500),
            std::time::Duration::from_micros(4_000),
            || lane_times.next().expect("lane clock sample"),
            |index| Ok::<_, ()>(item_progress[index]),
        )
        .unwrap();

        assert_eq!(slice.start_cursor, 0);
        assert_eq!(slice.end_cursor, 2);
        assert_eq!(
            slice.max_item_elapsed,
            std::time::Duration::from_micros(900)
        );
        assert_eq!(slice.max_item_index, Some(0));
        assert_eq!(slice.max_item_phases, item_progress[0].phases);
        assert!(slice.max_item_phases.total() <= slice.max_item_elapsed);
        assert_eq!(slice.elapsed, std::time::Duration::from_micros(2_000));
    }

    #[test]
    fn resumable_atlas_keeps_fixed_storage_and_publishes_only_after_all_slots() {
        let glyphs = [GlyphKey::new("x", false), GlyphKey::new("g", true)];
        let mut preparation = GlyphAtlasPreparation::new(&glyphs, 48.0, 2.0).unwrap();
        let rgba_pointer = preparation.atlas.rgba.as_ptr();
        let rgba_capacity = preparation.atlas.rgba.capacity();

        let paused = preparation
            .advance(std::time::Duration::ZERO, std::time::Duration::MAX)
            .unwrap();
        assert_eq!(paused.completed_items, 0);
        assert!(!paused.complete);
        assert_eq!(preparation.next_index, 0);
        assert!(preparation.atlas.entries.is_empty());
        assert_eq!(preparation.atlas.rgba.as_ptr(), rgba_pointer);
        assert_eq!(preparation.atlas.rgba.capacity(), rgba_capacity);

        let completed = preparation
            .advance(std::time::Duration::MAX, std::time::Duration::MAX)
            .unwrap();
        assert!(completed.complete);
        assert_eq!(preparation.next_index, glyphs.len());
        assert_eq!(preparation.atlas.entries.len(), glyphs.len());
        assert_eq!(preparation.atlas.rgba.as_ptr(), rgba_pointer);
        assert_eq!(preparation.atlas.rgba.capacity(), rgba_capacity);
        let atlas = preparation.finish().unwrap();
        assert_eq!(atlas.entries.len(), glyphs.len());
    }

    #[test]
    fn scalar_sequence_is_one_atlas_key() {
        let key = GlyphKey::new("ö", false);
        assert_eq!(key.sequence.as_str(), "ö");
    }

    #[test]
    fn color_entry_bypasses_foreground_tint() {
        let entry = GlyphAtlasEntry::synthetic_visible(GlyphEntryKind::PremultipliedColorRgba);
        assert_eq!(entry.fragment_mode(), FragmentGlyphMode::NativeColor);
    }

    #[test]
    fn whitespace_keeps_advance_without_visible_uv() {
        let entry = GlyphAtlasEntry::whitespace(24.0, 52.0);
        assert_eq!(entry.advance, 24.0);
        assert_eq!(entry.visible_uv, None);
    }
}

#[cfg(test)]
mod repertoire_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::Species;
    use crate::round::hud::{companion_hud_text, CompanionHudText};
    use crate::round::scene::CompanionMotion;
    use crate::round::smooth::{build_round_smooth_scene_plan, frame_glyph_sequences};
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    const GRID_COLS: u16 = 44;
    const GRID_ROWS: u16 = 18;

    /// One rendered companion frame: the scene plan plus its HUD text. The glyph
    /// keys are extracted with the same backend-neutral collector the production
    /// renderer uses.
    struct StripFrame {
        plan: crate::presentation::smooth::SmoothCompanionScenePlan,
        hud: CompanionHudText,
    }

    impl StripFrame {
        fn glyph_keys(&self) -> Vec<GlyphKey> {
            frame_glyph_sequences(&self.plan, &self.hud)
                .into_iter()
                .map(|glyph| GlyphKey::new(glyph.sequence, glyph.bold))
                .collect()
        }
    }

    /// A resource cache that compiles a manifest's full repertoire once at
    /// activation, then serves frames from it. It rebuilds only on a resource
    /// generation change — never per frame — so `prepare` only ever counts atlas
    /// misses (a frame glyph the compiled atlas lacks), never builds or uploads.
    struct TestResourceCache {
        compiled: CompiledRetainedResources,
        counters: RetainedResourceCounters,
    }

    impl TestResourceCache {
        fn activate(manifest: GlyphRepertoireManifest) -> Self {
            let compiled = CompiledRetainedResources::compile(&manifest)
                .expect("the full declared repertoire fits the preflighted atlas");
            Self {
                compiled,
                counters: RetainedResourceCounters::default(),
            }
        }

        fn prepare(
            &mut self,
            frame: &StripFrame,
        ) -> std::result::Result<(), RetainedFailureCategory> {
            // The generation is fixed at activation: a frame never rebuilds or
            // re-uploads the atlas. A missing glyph is the only churn a frame can
            // cause, and the full-repertoire preflight forbids it.
            for key in frame.glyph_keys() {
                if !self.compiled.contains(&key) {
                    self.counters.atlas_misses += 1;
                }
            }
            Ok(())
        }

        fn counters(&self) -> RetainedResourceCounters {
            self.counters
        }
    }

    /// One rendered frame for a single species at `now`, used to prove the
    /// per-minute room reshuffle never misses the compiled atlas.
    fn single_species_frame(species: Species, now: time::OffsetDateTime) -> StripFrame {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_render.generated_species = species;
        let tick = now.unix_timestamp().max(0) as u64;
        crate::commands::watch::rerender_pet_for_view_model(&mut vm, tick, false, now)
            .expect("pet rerenders");
        let plan = build_round_smooth_scene_plan(
            &vm,
            now,
            GRID_COLS,
            GRID_ROWS,
            &CompanionMotion::default(),
            tick * 250,
        );
        StripFrame {
            plan,
            hud: companion_hud_text(1_234_567.0, Some(0.5), 8_900.0),
        }
    }

    /// A deterministic animation strip covering every species (→ pet body, room
    /// dialect, and props), a spread of stages and moods, tick-driven blink and
    /// particles, changing HUD digits, and a minute boundary so the room reseeds
    /// its random glyph pick.
    fn deterministic_full_strip_at(_logical_size: u32, _backing_scale: f64) -> Vec<StripFrame> {
        let base = datetime!(2026-06-13 18:00:30 UTC);
        let motion = CompanionMotion::default();
        let profiles = [
            (Stage::S0, Mood::Content, false),
            (Stage::S3, Mood::Happy, false),
            (Stage::S6, Mood::Sleepy, true),
        ];
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        let mut frames = Vec::new();
        for species in Species::all() {
            vm.pet_render.generated_species = species;
            for &(stage, mood, asleep) in &profiles {
                vm.pet_render.stage = stage;
                vm.pet_render.mood = mood;
                for minute in [0_i64, 1] {
                    for tick_step in 0..2_u64 {
                        let now = base
                            + time::Duration::minutes(minute)
                            + time::Duration::seconds((tick_step * 11) as i64);
                        let tick = now.unix_timestamp().max(0) as u64;
                        crate::commands::watch::rerender_pet_for_view_model(
                            &mut vm, tick, asleep, now,
                        )
                        .expect("pet rerenders");
                        let plan = build_round_smooth_scene_plan(
                            &vm,
                            now,
                            GRID_COLS,
                            GRID_ROWS,
                            &motion,
                            tick * 250,
                        );
                        let hud = companion_hud_text(
                            (tick % 9_999_999) as f64,
                            Some((tick % 200) as f64 / 100.0),
                            (tick % 88_000) as f64,
                        );
                        frames.push(StripFrame { plan, hud });
                    }
                }
            }
        }
        frames
    }

    fn deterministic_full_strip() -> Vec<StripFrame> {
        deterministic_full_strip_at(960, 2.0)
    }

    #[test]
    fn manifest_contains_dynamic_and_chrome_repertoire() {
        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        for required in ["-", ".", "0", "9", "\u{fffd}", "\u{f6}", "\u{1fae7}"] {
            assert!(manifest.contains_sequence(required), "missing {required}");
        }
    }

    #[test]
    fn full_animation_strip_has_no_post_activation_atlas_churn() {
        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        let mut cache = TestResourceCache::activate(manifest);
        for frame in deterministic_full_strip() {
            cache.prepare(&frame).unwrap();
        }
        assert_eq!(cache.counters().atlas_builds_after_activation, 0);
        assert_eq!(cache.counters().atlas_uploads_after_activation, 0);
        assert_eq!(cache.counters().atlas_misses, 0);
    }

    #[test]
    fn generation_key_is_stable_and_churn_free_across_a_minute_boundary_reshuffle() {
        let species = Species::Crystal;
        let identity = crate::round::smooth::CompanionContentIdentity::for_pet(species);
        let manifest = GlyphRepertoireManifest::for_active_pet(identity, 2.0);
        let key = manifest.generation_key();
        let mut cache = TestResourceCache::activate(manifest);
        // The room reseeds its random glyph pick every minute; step across
        // several minute boundaries and confirm the atlas serves them all with no
        // rebuild, upload, or miss.
        let base = datetime!(2026-06-13 18:00:30 UTC);
        for minute in [0_i64, 1, 2, 5, 60] {
            let now = base + time::Duration::minutes(minute);
            cache.prepare(&single_species_frame(species, now)).unwrap();
        }
        assert_eq!(cache.counters().atlas_builds_after_activation, 0);
        assert_eq!(cache.counters().atlas_uploads_after_activation, 0);
        assert_eq!(cache.counters().atlas_misses, 0);
        // The declared-content generation key is unchanged by the reshuffle.
        assert_eq!(
            GlyphRepertoireManifest::for_active_pet(
                crate::round::smooth::CompanionContentIdentity::for_pet(species),
                2.0,
            )
            .generation_key(),
            key,
        );
        // A backing-scale change is a real generation change (Task-8 finding #3).
        assert_ne!(
            GlyphRepertoireManifest::for_active_pet(
                crate::round::smooth::CompanionContentIdentity::for_pet(species),
                1.0,
            )
            .generation_key(),
            key,
        );
    }

    #[test]
    fn full_strip_has_no_churn_across_the_size_and_scale_matrix() {
        for logical_size in [260_u32, 360, 480, 720] {
            for backing_scale in [1.0_f64, 2.0] {
                let manifest =
                    GlyphRepertoireManifest::for_fixture_pet_at(logical_size, backing_scale);
                let mut cache = TestResourceCache::activate(manifest);
                for frame in deterministic_full_strip_at(logical_size, backing_scale) {
                    cache.prepare(&frame).unwrap();
                }
                let counters = cache.counters();
                assert_eq!(
                    counters.atlas_builds_after_activation, 0,
                    "builds at {logical_size}@{backing_scale}",
                );
                assert_eq!(
                    counters.atlas_uploads_after_activation, 0,
                    "uploads at {logical_size}@{backing_scale}",
                );
                assert_eq!(
                    counters.atlas_misses, 0,
                    "misses at {logical_size}@{backing_scale}",
                );
            }
        }
    }

    /// The render path looks up `GlyphKey{sequence, bold}` for every scene cell,
    /// where the weight is the cell's real BOLD flag — and tank-life foreground
    /// (and other roles) render bold, not just the eye role. This walks a
    /// live-like scene that renders bold tank-life foreground cells and asserts
    /// every rendered `(glyph, weight)` pair is present in the preflighted
    /// repertoire, guarding the invariant `repertoire ⊇ rendered-(glyph,bold)-set`
    /// against any collector/render weight-or-glyph drift.
    #[test]
    fn preflighted_repertoire_covers_every_rendered_glyph_and_weight() {
        use crate::presentation::smooth::{SmoothLayerItem, SmoothLayerRole};

        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        let base = datetime!(2026-06-13 18:00:30 UTC);
        let motion = CompanionMotion::default();
        // A habitat with the full tank-life cast (foreground sprites render bold)
        // and earned props — the content the live `--state normal` companion draws.
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(120, base.date());
        vm.habitat.earned_props = WatchViewModel::fixture_with_habitat_props()
            .habitat
            .earned_props;

        let present = |glyph: &str, bold: bool| {
            manifest
                .glyphs()
                .iter()
                .any(|key| key.sequence.as_str() == glyph && key.bold == bold)
        };

        // The review states the parity gate captures: Normal, ActivePulse,
        // AsleepCalm, HelperTrouble, approximated by their mood + asleep shape.
        let review_states = [
            (Mood::Content, false),
            (Mood::Ecstatic, false),
            (Mood::Sleepy, true),
            (Mood::Sad, false),
        ];
        let mut saw_bold_tank_foreground = false;
        let mut missing: Vec<(String, bool)> = Vec::new();
        for species in Species::all() {
            vm.pet_render.generated_species = species;
            for stage in [Stage::S3, Stage::S6] {
                vm.pet_render.stage = stage;
                for &(mood, asleep) in &review_states {
                    vm.pet_render.mood = mood;
                    for minute in [0_i64, 1] {
                        let now = base + time::Duration::minutes(minute);
                        let tick = now.unix_timestamp().max(0) as u64;
                        crate::commands::watch::rerender_pet_for_view_model(
                            &mut vm, tick, asleep, now,
                        )
                        .expect("pet rerenders");
                        // Device-like grid (COMPANION_TARGET_COLS square) so the
                        // tank bed is large enough to place the foreground cast.
                        let plan =
                            build_round_smooth_scene_plan(&vm, now, 36, 36, &motion, tick * 250);
                        for layer in &plan.layers {
                            for item in &layer.items {
                                if let SmoothLayerItem::LocalCell(cell) = item {
                                    if let Some(glyph) = cell.glyph.as_ref() {
                                        if !present(glyph, cell.bold) {
                                            missing.push((glyph.clone(), cell.bold));
                                        }
                                        if cell.bold
                                            && layer.role == SmoothLayerRole::TankLifeForeground
                                        {
                                            saw_bold_tank_foreground = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // The render path also looks up every HUD scalar (always regular weight).
        // A review-pair capture renders the REDACTED HUD by default, not the live
        // one, so its letters must be covered too.
        let huds = [
            companion_hud_text(1_234_567.0, Some(0.5), 8_900.0),
            crate::round::hud::review_capture_hud_text(),
        ];
        for hud in &huds {
            for line in [&hud.today_total, &hud.daily_percent, &hud.pace] {
                for scalar in line.chars() {
                    if !present(&scalar.to_string(), false) {
                        missing.push((scalar.to_string(), false));
                    }
                }
            }
        }

        assert!(
            saw_bold_tank_foreground,
            "the test must exercise a bold tank-life foreground cell to guard the regression",
        );
        // The redacted review-capture HUD letters specifically ("review" /
        // "privacy" / "redacted").
        for required in ['c', 'e', 'i', 'p', 'r', 't', 'w'] {
            assert!(
                present(&required.to_string(), false),
                "redacted review-capture HUD letter {required:?} is not in the repertoire",
            );
        }
        assert!(
            missing.is_empty(),
            "rendered (glyph, bold) pairs absent from the preflighted repertoire: {missing:?}",
        );
    }
}
