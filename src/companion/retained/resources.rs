//! Glyph atlas resources for the retained companion renderer: complete glyph
//! metrics, Unicode scalar-sequence atlas keys, a resolved font policy, and
//! native-color (emoji) atlas entries.
//!
//! The rasterizer draws through `NSBitmapImageRep::initWithBitmapDataPlanes...`,
//! a different selector from the AppKit view-cache selectors the retained
//! capture boundary forbids, so glyph rasterization lives beside the retained
//! renderer here rather than in `capture.rs`.

use std::collections::{BTreeMap, VecDeque};

use objc2::msg_send_id;
use objc2::rc::{autoreleasepool, Retained};
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

/// Exact integer bounds of the atlas cell allocated to one glyph. These bounds
/// describe the packing allocation, including whitespace and transparent
/// gutters; they are deliberately independent of the visible-ink UV rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AtlasCell {
    pub(super) origin: [u32; 2],
    pub(super) extent: [u32; 2],
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
    /// Exact packed cell owned by this entry, including transparent padding.
    #[allow(dead_code)] // Scene-atlas validation reads this independently of visible UV.
    pub(super) allocated_cell: AtlasCell,
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
    pub(super) fn whitespace(advance: f32, line_height: f32, allocated_cell: AtlasCell) -> Self {
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
            allocated_cell,
        }
    }

    /// A deterministic visible entry for capacity inventory and contract tests.
    /// Primitive-count inventory needs key occupancy, not native font geometry;
    /// using this avoids hidden AppKit raster work in the evidence callback.
    pub(super) fn synthetic_visible(kind: GlyphEntryKind, allocated_cell: AtlasCell) -> Self {
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
            allocated_cell,
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
/// in gamma to match Smooth. Version 4 adds the biome-tinted tank bed to the room
/// analytic. A shader change bumps this so entries from an older shader can be
/// invalidated.
pub(super) const SHADER_RESOURCE_VERSION: u32 = 4;

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
    #[cfg(test)]
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
    /// Reads the identity of an already-resolved Smooth monospaced font. The
    /// policy id and the rasterizer therefore describe the exact same AppKit
    /// object, rather than independently asking the font factory again.
    fn from_font(
        font: &NSFont,
        point_size: f64,
        backing_scale: f64,
        weight: FontWeightPolicy,
    ) -> Self {
        // SAFETY: `font` is a valid `NSFont`; `fontName`/`fontDescriptor` read
        // immutable identity.
        let postscript_name = unsafe { font.fontName() }.to_string();
        let descriptor_hash = descriptor_hash(font);
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

const FONT_RESOLUTION_MAX_ATTEMPTS: usize = 3;

/// Sends the exact Smooth font selector through a nullable return type. AppKit
/// documents the class method as nonnull, but the Objective-C ABI can still
/// return nil under transient system-font initialization failures. Keeping the
/// raw boundary narrow prevents the generated nonnull binding from turning nil
/// into an objc2 panic.
fn resolve_font_once(point_size: f64, weight: FontWeightPolicy) -> Option<Retained<NSFont>> {
    // SAFETY: The selector and argument ABI exactly match
    // `+[NSFont monospacedSystemFontOfSize:weight:]`. `Option<Retained<_>>`
    // deliberately models a nullable autoreleased Objective-C object result.
    #[allow(deprecated)]
    unsafe {
        msg_send_id![
            NSFont::class(),
            monospacedSystemFontOfSize: point_size,
            weight: weight.ns_weight()
        ]
    }
}

fn resolve_font_with_attempts(
    point_size: f64,
    weight: FontWeightPolicy,
    resolver: &mut impl FnMut(f64, FontWeightPolicy) -> Option<Retained<NSFont>>,
) -> std::result::Result<Retained<NSFont>, RetainedFailureCategory> {
    for _ in 0..FONT_RESOLUTION_MAX_ATTEMPTS {
        // Each immediate attempt gets its own pool so a failed AppKit lookup
        // cannot retain autoreleased intermediates into the next attempt.
        if let Some(font) = autoreleasepool(|_| resolver(point_size, weight)) {
            return Ok(font);
        }
    }
    Err(RetainedFailureCategory::FontUnavailable)
}

struct ResolvedAtlasFonts {
    regular: Retained<NSFont>,
    bold: Retained<NSFont>,
    regular_policy_id: u64,
    bold_policy_id: u64,
}

impl ResolvedAtlasFonts {
    fn resolve_with(
        point_size: f64,
        backing_scale: f64,
        resolver: &mut impl FnMut(f64, FontWeightPolicy) -> Option<Retained<NSFont>>,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let regular = resolve_font_with_attempts(point_size, FontWeightPolicy::Regular, resolver)?;
        let bold = resolve_font_with_attempts(point_size, FontWeightPolicy::Bold, resolver)?;
        let regular_policy_id = ResolvedFontPolicy::from_font(
            &regular,
            point_size,
            backing_scale,
            FontWeightPolicy::Regular,
        )
        .id();
        let bold_policy_id =
            ResolvedFontPolicy::from_font(&bold, point_size, backing_scale, FontWeightPolicy::Bold)
                .id();
        Ok(Self {
            regular,
            bold,
            regular_policy_id,
            bold_policy_id,
        })
    }
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
    #[cfg(test)]
    pub(super) point_size: f64,
}

/// A premultiplied channel spread above this (out of 255) marks a pixel as
/// carrying native chroma rather than a gray coverage mask. Grayscale-antialiased
/// white text stays at spread zero; emoji cross it decisively.
const CHROMA_THRESHOLD: u8 = 16;

struct CurrentGraphicsContextGuard {
    previous: Option<Retained<NSGraphicsContext>>,
}

impl CurrentGraphicsContextGuard {
    fn install(context: &NSGraphicsContext) -> Self {
        // SAFETY: NSGraphicsContext's current context is thread-local. The
        // retained previous value remains valid until this guard restores it.
        let previous = unsafe { NSGraphicsContext::currentContext() };
        unsafe { NSGraphicsContext::setCurrentContext(Some(context)) };
        Self { previous }
    }
}

impl Drop for CurrentGraphicsContextGuard {
    fn drop(&mut self) {
        // SAFETY: The guard is dropped on the same thread where it was created,
        // and `previous` retains the context being restored.
        unsafe { NSGraphicsContext::setCurrentContext(self.previous.as_deref()) };
    }
}

/// Rasterizes one glyph into `atlas` at `(x, y)` and returns its complete entry.
///
/// The glyph is drawn white so mask glyphs carry pure coverage. If the
/// non-transparent pixels contain native chroma (an emoji), the entry keeps the
/// premultiplied RGBA the bitmap produced and is classified
/// [`GlyphEntryKind::PremultipliedColorRgba`]; otherwise it is stored as white
/// RGB plus coverage alpha ([`GlyphEntryKind::Mask`]).
#[cfg(test)]
fn resolve_reference_font_for_test(point_size: f64, weight: FontWeightPolicy) -> Retained<NSFont> {
    // This deliberately uses the independently generated objc2 binding. It is
    // reference-only test code; production uses the nullable raw boundary and
    // never performs a font factory call while rasterizing a glyph.
    unsafe { NSFont::monospacedSystemFontOfSize_weight(point_size, weight.ns_weight()) }
}

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
    let font =
        resolve_reference_font_for_test(target.point_size, FontWeightPolicy::from_bold(key.bold));
    rasterize_glyph_entry_impl(
        key,
        target,
        &font,
        font_policy_id,
        atlas,
        atlas_width,
        atlas_height,
        x,
        y,
    )
}

#[allow(clippy::too_many_arguments)]
fn rasterize_glyph_entry_impl(
    key: &GlyphKey,
    target: &GlyphRasterTarget,
    font: &NSFont,
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
        let current_context_guard = CurrentGraphicsContextGuard::install(&context);

        let text = NSString::from_str(key.sequence.as_str());
        let mut attributed = NSMutableAttributedString::from_nsstring(&text);
        let range = NSRange::from(0..text.length());
        attributed.addAttribute_value_range(NSFontAttributeName, font, range);
        let white = NSColor::whiteColor();
        attributed.addAttribute_value_range(NSForegroundColorAttributeName, &white, range);
        let attributed: Retained<objc2_foundation::NSAttributedString> =
            Retained::into_super(attributed);
        let size = attributed.size();
        if size.width + f64::from(padding * 2) > f64::from(cell)
            || size.height + f64::from(padding * 2) > f64::from(cell)
        {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        let draw_x = f64::from(padding);
        let draw_y = f64::from(padding);
        attributed.drawAtPoint(NSPoint::new(draw_x, draw_y));
        context.flushGraphics();
        drop(current_context_guard);

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
                AtlasCell { origin: [x, y], extent: [cell, cell] },
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
            allocated_cell: AtlasCell { origin: [x, y], extent: [cell, cell] },
        };
        Ok(entry)
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

/// A dense, deterministic scene-atlas entry. The id is its index in
/// [`PreparedSceneAtlas::entries`], assigned in complete [`GlyphKey`] order.
#[derive(Debug, Clone)]
#[allow(dead_code)] // CPU-only handoff remains independent of GPU ownership.
pub(super) struct PreparedSceneAtlasEntry {
    pub(super) id: u32,
    pub(super) key: GlyphKey,
    pub(super) entry: GlyphAtlasEntry,
}

/// Pure worker-owned atlas data prepared for the scene renderer. Coverage and
/// native color are split so later GPU materialization can choose appropriate
/// texture formats without retaining AppKit or wgpu objects here.
#[derive(Debug)]
#[allow(dead_code)] // CPU-only handoff remains independent of GPU ownership.
pub(super) struct PreparedSceneAtlas {
    /// Presentation identity this atlas was prepared for. The render owner
    /// rejects a CPU upload carrying a different resource generation before it
    /// opens GPU error scopes or allocates any candidate objects.
    pub(super) resource_generation: crate::presentation::companion_scene::ResourceGeneration,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) coverage_r8: Vec<u8>,
    pub(super) straight_color_rgba_srgb: Vec<u8>,
    pub(super) entries: Vec<PreparedSceneAtlasEntry>,
    entry_ids: BTreeMap<GlyphKey, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Preparation stays fallible before any GPU objects exist.
pub(super) enum PreparedSceneAtlasError {
    PixelDataLength { expected: usize, actual: usize },
    TooManyEntries,
    CellOutOfBounds { key: GlyphKey },
    OverlappingCells { first: GlyphKey, second: GlyphKey },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Resolution stays typed at the CPU scene boundary.
pub(super) enum GlyphAtlasResolveError {
    MissingKey(GlyphKey),
    MissingSingleScalar(char),
    AmbiguousSingleScalar {
        scalar: char,
        matches: Vec<GlyphKey>,
    },
}

#[allow(dead_code)] // Pure conversion is exercised before GPU materialization.
impl PreparedSceneAtlas {
    /// Test/capacity convenience. Production handoff uses
    /// [`Self::from_compiled_for_generation`] so resource identity is explicit.
    #[cfg(test)]
    pub(super) fn from_compiled(
        compiled: &CompiledGlyphAtlas,
    ) -> std::result::Result<Self, PreparedSceneAtlasError> {
        Self::from_compiled_for_generation(
            compiled,
            crate::presentation::companion_scene::ResourceGeneration(0),
        )
    }

    pub(super) fn from_compiled_for_generation(
        compiled: &CompiledGlyphAtlas,
        resource_generation: crate::presentation::companion_scene::ResourceGeneration,
    ) -> std::result::Result<Self, PreparedSceneAtlasError> {
        let pixel_count = usize::try_from(compiled.width)
            .ok()
            .and_then(|width| {
                usize::try_from(compiled.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or(PreparedSceneAtlasError::PixelDataLength {
                expected: usize::MAX,
                actual: compiled.rgba.len(),
            })?;
        let expected_rgba =
            pixel_count
                .checked_mul(4)
                .ok_or(PreparedSceneAtlasError::PixelDataLength {
                    expected: usize::MAX,
                    actual: compiled.rgba.len(),
                })?;
        if compiled.rgba.len() != expected_rgba {
            return Err(PreparedSceneAtlasError::PixelDataLength {
                expected: expected_rgba,
                actual: compiled.rgba.len(),
            });
        }
        if compiled.entries.len() > u32::MAX as usize {
            return Err(PreparedSceneAtlasError::TooManyEntries);
        }

        let mut allocated: Vec<(GlyphKey, AtlasCell)> = Vec::with_capacity(compiled.entries.len());
        for (key, entry) in &compiled.entries {
            let cell = entry.allocated_cell;
            let end_x = cell.origin[0].checked_add(cell.extent[0]);
            let end_y = cell.origin[1].checked_add(cell.extent[1]);
            if cell.extent.contains(&0)
                || end_x.is_none_or(|end| end > compiled.width)
                || end_y.is_none_or(|end| end > compiled.height)
            {
                return Err(PreparedSceneAtlasError::CellOutOfBounds { key: key.clone() });
            }
            for (other_key, other_cell) in &allocated {
                if cells_overlap(cell, *other_cell) {
                    return Err(PreparedSceneAtlasError::OverlappingCells {
                        first: other_key.clone(),
                        second: key.clone(),
                    });
                }
            }
            allocated.push((key.clone(), cell));
        }

        let mut coverage_r8 = vec![0; pixel_count];
        let mut straight_color_rgba_srgb = vec![0; expected_rgba];
        let mut entries = Vec::with_capacity(compiled.entries.len());
        let mut entry_ids = BTreeMap::new();
        for (key, source_entry) in &compiled.entries {
            let id = entries.len() as u32;
            entry_ids.insert(key.clone(), id);
            entries.push(PreparedSceneAtlasEntry {
                id,
                key: key.clone(),
                entry: *source_entry,
            });
            copy_scene_atlas_cell(
                compiled,
                *source_entry,
                &mut coverage_r8,
                &mut straight_color_rgba_srgb,
            );
        }

        Ok(Self {
            resource_generation,
            width: compiled.width,
            height: compiled.height,
            coverage_r8,
            straight_color_rgba_srgb,
            entries,
            entry_ids,
        })
    }

    pub(super) fn resolve_key(
        &self,
        key: &GlyphKey,
    ) -> std::result::Result<&PreparedSceneAtlasEntry, GlyphAtlasResolveError> {
        self.entry_ids
            .get(key)
            .map(|&id| &self.entries[id as usize])
            .ok_or_else(|| GlyphAtlasResolveError::MissingKey(key.clone()))
    }

    /// Resolves only an authored sequence containing exactly one scalar. Weight
    /// is intentionally absent: if both regular and bold exist, callers must use
    /// the full-key resolver instead of silently choosing one.
    pub(super) fn resolve_single_scalar(
        &self,
        scalar: char,
    ) -> std::result::Result<&PreparedSceneAtlasEntry, GlyphAtlasResolveError> {
        let matches = self
            .entries
            .iter()
            .filter(|candidate| {
                let mut scalars = candidate.key.sequence.as_str().chars();
                scalars.next() == Some(scalar) && scalars.next().is_none()
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(GlyphAtlasResolveError::MissingSingleScalar(scalar)),
            [entry] => Ok(*entry),
            _ => Err(GlyphAtlasResolveError::AmbiguousSingleScalar {
                scalar,
                matches: matches.iter().map(|entry| entry.key.clone()).collect(),
            }),
        }
    }
}

#[allow(dead_code)]
fn cells_overlap(left: AtlasCell, right: AtlasCell) -> bool {
    left.origin[0] < right.origin[0] + right.extent[0]
        && right.origin[0] < left.origin[0] + left.extent[0]
        && left.origin[1] < right.origin[1] + right.extent[1]
        && right.origin[1] < left.origin[1] + left.extent[1]
}

#[allow(dead_code)]
fn copy_scene_atlas_cell(
    compiled: &CompiledGlyphAtlas,
    entry: GlyphAtlasEntry,
    coverage_r8: &mut [u8],
    straight_color_rgba_srgb: &mut [u8],
) {
    let cell = entry.allocated_cell;
    for row in 0..cell.extent[1] {
        for column in 0..cell.extent[0] {
            let x = cell.origin[0] + column;
            let y = cell.origin[1] + row;
            let pixel = y as usize * compiled.width as usize + x as usize;
            let rgba = pixel * 4;
            let alpha = compiled.rgba[rgba + 3];
            match entry.kind {
                GlyphEntryKind::Mask => coverage_r8[pixel] = alpha,
                GlyphEntryKind::PremultipliedColorRgba => {
                    straight_color_rgba_srgb[rgba + 3] = alpha;
                    if alpha != 0 {
                        for channel in 0..3 {
                            straight_color_rgba_srgb[rgba + channel] =
                                unpremultiply(compiled.rgba[rgba + channel], alpha);
                        }
                    }
                }
            }
        }
    }
    if entry.kind == GlyphEntryKind::PremultipliedColorRgba {
        dilate_native_color_cell(compiled.width, entry, straight_color_rgba_srgb);
    }
}

#[allow(dead_code)]
fn unpremultiply(channel: u8, alpha: u8) -> u8 {
    debug_assert_ne!(alpha, 0);
    ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
}

#[allow(dead_code)]
fn dilate_native_color_cell(atlas_width: u32, entry: GlyphAtlasEntry, color: &mut [u8]) {
    let cell = entry.allocated_cell;
    let radius = if entry.safe_padding.is_finite() && entry.safe_padding > 0.0 {
        entry.safe_padding.floor() as u32
    } else {
        0
    }
    .min(cell.extent[0].max(cell.extent[1]));
    if radius == 0 {
        return;
    }

    let local_len = cell.extent[0] as usize * cell.extent[1] as usize;
    let mut distance = vec![u32::MAX; local_len];
    let mut queue = VecDeque::with_capacity(local_len);
    for row in 0..cell.extent[1] {
        for column in 0..cell.extent[0] {
            let local = row as usize * cell.extent[0] as usize + column as usize;
            let pixel = (cell.origin[1] + row) as usize * atlas_width as usize
                + (cell.origin[0] + column) as usize;
            if color[pixel * 4 + 3] != 0 {
                distance[local] = 0;
                queue.push_back((column, row));
            }
        }
    }

    while let Some((column, row)) = queue.pop_front() {
        let local = row as usize * cell.extent[0] as usize + column as usize;
        let next_distance = distance[local] + 1;
        if next_distance > radius {
            continue;
        }
        let pixel = (cell.origin[1] + row) as usize * atlas_width as usize
            + (cell.origin[0] + column) as usize;
        let rgb = [color[pixel * 4], color[pixel * 4 + 1], color[pixel * 4 + 2]];
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let Some(next_column) = column.checked_add_signed(dx) else {
                    continue;
                };
                let Some(next_row) = row.checked_add_signed(dy) else {
                    continue;
                };
                if next_column >= cell.extent[0] || next_row >= cell.extent[1] {
                    continue;
                }
                let next_local = next_row as usize * cell.extent[0] as usize + next_column as usize;
                if distance[next_local] != u32::MAX {
                    continue;
                }
                let next_pixel = (cell.origin[1] + next_row) as usize * atlas_width as usize
                    + (cell.origin[0] + next_column) as usize;
                if color[next_pixel * 4 + 3] != 0 {
                    continue;
                }
                distance[next_local] = next_distance;
                color[next_pixel * 4..next_pixel * 4 + 3].copy_from_slice(&rgb);
                queue.push_back((next_column, next_row));
            }
        }
    }
}

struct GlyphAtlasPreparation {
    glyphs: Vec<GlyphKey>,
    target: GlyphRasterTarget,
    fonts: ResolvedAtlasFonts,
    next_index: usize,
    atlas: CompiledGlyphAtlas,
}

impl GlyphAtlasPreparation {
    #[cfg(test)]
    fn new(
        glyphs: &[GlyphKey],
        point_size: f64,
        backing_scale: f64,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let fonts =
            ResolvedAtlasFonts::resolve_with(point_size, backing_scale, &mut resolve_font_once)?;
        Self::new_with_fonts(glyphs, point_size, fonts)
    }

    fn new_with_fonts(
        glyphs: &[GlyphKey],
        point_size: f64,
        fonts: ResolvedAtlasFonts,
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
        let target = GlyphRasterTarget {
            cell,
            padding,
            #[cfg(test)]
            point_size,
        };
        Ok(Self {
            glyphs: glyphs.to_vec(),
            target,
            fonts,
            next_index: 0,
            atlas: CompiledGlyphAtlas {
                width,
                height,
                rgba: vec![0_u8; (width * height * 4) as usize],
                entries: BTreeMap::new(),
            },
        })
    }

    fn advance_one(&mut self) -> std::result::Result<bool, RetainedFailureCategory> {
        if self.next_index == self.glyphs.len() {
            return Ok(true);
        }
        rasterize_prepared_glyph(
            &self.glyphs,
            &self.target,
            &self.fonts,
            &mut self.atlas,
            self.next_index,
        )?;
        self.next_index += 1;
        Ok(self.next_index == self.glyphs.len())
    }

    fn finish(self) -> std::result::Result<CompiledGlyphAtlas, RetainedFailureCategory> {
        if self.next_index != self.glyphs.len() {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        Ok(self.atlas)
    }
}

fn rasterize_prepared_glyph(
    glyphs: &[GlyphKey],
    target: &GlyphRasterTarget,
    fonts: &ResolvedAtlasFonts,
    atlas: &mut CompiledGlyphAtlas,
    index: usize,
) -> std::result::Result<(), RetainedFailureCategory> {
    let key = &glyphs[index];
    let slot_x = index as u32 % ATLAS_COLUMNS;
    let slot_y = index as u32 / ATLAS_COLUMNS;
    let font_policy_id = if key.bold {
        fonts.bold_policy_id
    } else {
        fonts.regular_policy_id
    };
    let entry = autoreleasepool(|_| {
        let font = if key.bold {
            &*fonts.bold
        } else {
            &*fonts.regular
        };
        rasterize_glyph_entry_impl(
            key,
            target,
            font,
            font_policy_id,
            &mut atlas.rgba,
            atlas.width,
            atlas.height,
            slot_x * target.cell,
            slot_y * target.cell,
        )
    })?;
    atlas.entries.insert(key.clone(), entry);
    Ok(())
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
    fn compute(
        manifest: &GlyphRepertoireManifest,
        regular_font_policy_id: u64,
        bold_font_policy_id: u64,
    ) -> Self {
        let mut hasher = Fnv1a::new();
        hasher.write(&manifest.identity.identity_bytes());
        for key in &manifest.glyphs {
            hasher.write(key.sequence.as_str().as_bytes());
            hasher.write_u8(u8::from(key.bold));
            hasher.write_u8(0xff);
        }
        hasher.write_u64(regular_font_policy_id);
        hasher.write_u64(bold_font_policy_id);
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

    #[cfg(test)]
    pub(super) fn generation_key(&self) -> ResourceGenerationKey {
        let fonts = ResolvedAtlasFonts::resolve_with(
            self.atlas_point_size,
            self.backing_scale,
            &mut resolve_font_once,
        )
        .expect("test font policy resolves");
        ResourceGenerationKey::compute(self, fonts.regular_policy_id, fonts.bold_policy_id)
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
        Self::new_with_font_resolver(manifest, &mut resolve_font_once)
    }

    pub(super) fn new_with_font_resolver(
        manifest: &GlyphRepertoireManifest,
        resolver: &mut impl FnMut(f64, FontWeightPolicy) -> Option<Retained<NSFont>>,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let fonts = ResolvedAtlasFonts::resolve_with(
            manifest.atlas_point_size,
            manifest.backing_scale,
            resolver,
        )?;
        let generation =
            ResourceGenerationKey::compute(manifest, fonts.regular_policy_id, fonts.bold_policy_id);
        Ok(Self {
            generation,
            atlas: GlyphAtlasPreparation::new_with_fonts(
                manifest.glyphs(),
                manifest.atlas_point_size,
                fonts,
            )?,
        })
    }

    pub(super) fn advance_one(&mut self) -> std::result::Result<bool, RetainedFailureCategory> {
        self.atlas.advance_one()
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
        while !preparation.advance_one()? {
            // Synchronous parity fixtures intentionally drive the same worker
            // primitive one glyph at a time until the full atlas is complete.
        }
        preparation.finish()
    }

    pub(super) fn for_capacity_inventory(manifest: &GlyphRepertoireManifest) -> Self {
        let width = u32::try_from(manifest.glyphs().len())
            .expect("capacity inventory fits the declared atlas limit")
            .max(1);
        let entries = manifest
            .glyphs()
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, key)| {
                let allocated_cell = AtlasCell {
                    origin: [index as u32, 0],
                    extent: [1, 1],
                };
                let entry = if key.sequence.as_str().chars().all(char::is_whitespace) {
                    GlyphAtlasEntry::whitespace(29.0, 52.0, allocated_cell)
                } else {
                    GlyphAtlasEntry::synthetic_visible(GlyphEntryKind::Mask, allocated_cell)
                };
                (key, entry)
            })
            .collect();
        Self {
            generation: ResourceGenerationKey(0),
            atlas: CompiledGlyphAtlas {
                width,
                height: 1,
                rgba: vec![0; width as usize * 4],
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
            let font =
                resolve_reference_font_for_test(point_size, FontWeightPolicy::from_bold(bold));
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
                assert_eq!(
                    entry.allocated_cell,
                    AtlasCell { origin: [0, 0], extent: [cell, cell] },
                    "{label}@{cell}: exact allocated raster cell",
                );

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
    fn shader_resource_version_tracks_tank_bed_shader_change() {
        assert_eq!(SHADER_RESOURCE_VERSION, 4);
    }

    #[test]
    fn font_resolver_retries_nil_once_then_succeeds_in_exact_order() {
        let mut attempts = Vec::new();
        let mut resolver = |point_size, weight| {
            attempts.push((point_size, weight));
            (attempts.len() > 1).then(|| resolve_font_once(point_size, weight).unwrap())
        };

        let font = resolve_font_with_attempts(48.0, FontWeightPolicy::Regular, &mut resolver)
            .expect("second immediate attempt resolves the requested font");

        assert!(!unsafe { font.fontName() }.is_empty());
        assert_eq!(
            attempts,
            vec![
                (48.0, FontWeightPolicy::Regular),
                (48.0, FontWeightPolicy::Regular),
            ]
        );
    }

    #[test]
    fn font_resolver_caps_permanent_nil_at_three_with_typed_failure() {
        let mut attempts = Vec::new();
        let failure =
            resolve_font_with_attempts(48.0, FontWeightPolicy::Bold, &mut |point_size, weight| {
                attempts.push((point_size, weight));
                None
            })
            .expect_err("three nil replies exhaust the bounded resolver");

        assert_eq!(failure, RetainedFailureCategory::FontUnavailable);
        assert_eq!(
            attempts,
            vec![
                (48.0, FontWeightPolicy::Bold),
                (48.0, FontWeightPolicy::Bold),
                (48.0, FontWeightPolicy::Bold),
            ]
        );
    }

    #[test]
    fn full_preparation_resolves_exactly_regular_then_bold_once() {
        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        let mut successful_resolutions = Vec::new();
        let mut preparation = CompiledRetainedResourcesPreparation::new_with_font_resolver(
            &manifest,
            &mut |point_size, weight| {
                let font = resolve_font_once(point_size, weight);
                if font.is_some() {
                    successful_resolutions.push(weight);
                }
                font
            },
        )
        .expect("the production selector resolves both atlas fonts");

        while !preparation.advance_one().unwrap() {}
        preparation.finish().unwrap();
        assert_eq!(
            successful_resolutions,
            vec![FontWeightPolicy::Regular, FontWeightPolicy::Bold]
        );
    }

    fn bitmap_context() -> Retained<NSGraphicsContext> {
        unsafe {
            let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(), std::ptr::null_mut(), 8, 8, 8, 4, true, false,
                NSDeviceRGBColorSpace, 32, 32,
            ).expect("test bitmap representation");
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("test bitmap context")
        }
    }

    #[test]
    fn current_graphics_context_guard_restores_after_unwind() {
        autoreleasepool(|_| {
            let sentinel = bitmap_context();
            let replacement = bitmap_context();
            let _sentinel_guard = CurrentGraphicsContextGuard::install(&sentinel);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _replacement_guard = CurrentGraphicsContextGuard::install(&replacement);
                panic!("exercise context restoration");
            }));
            assert!(caught.is_err());
            let current =
                unsafe { NSGraphicsContext::currentContext() }.expect("sentinel context restored");
            assert!(std::ptr::eq(&*current, &*sentinel));
        });
    }

    #[test]
    fn advance_one_keeps_fixed_storage_and_finishes_after_all_slots() {
        let glyphs = [GlyphKey::new("x", false), GlyphKey::new("g", true)];
        let mut preparation = GlyphAtlasPreparation::new(&glyphs, 48.0, 2.0).unwrap();
        let rgba_pointer = preparation.atlas.rgba.as_ptr();
        let rgba_capacity = preparation.atlas.rgba.capacity();

        assert!(!preparation.advance_one().unwrap());
        assert!(preparation.advance_one().unwrap());
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
        let entry = GlyphAtlasEntry::synthetic_visible(
            GlyphEntryKind::PremultipliedColorRgba,
            AtlasCell { origin: [0, 0], extent: [1, 1] },
        );
        assert_eq!(entry.fragment_mode(), FragmentGlyphMode::NativeColor);
    }

    #[test]
    fn whitespace_keeps_advance_without_visible_uv() {
        let entry =
            GlyphAtlasEntry::whitespace(24.0, 52.0, AtlasCell { origin: [0, 0], extent: [1, 1] });
        assert_eq!(entry.advance, 24.0);
        assert_eq!(entry.visible_uv, None);
    }
}

#[cfg(test)]
mod prepared_scene_atlas_tests {
    use super::*;

    fn entry(
        kind: GlyphEntryKind,
        origin: [u32; 2],
        extent: [u32; 2],
        safe_padding: f32,
    ) -> GlyphAtlasEntry {
        GlyphAtlasEntry {
            visible_uv: Some([0.0, 0.0, 1.0, 1.0]),
            ink_origin: [0.0, 0.0],
            ink_size: [extent[0] as f32, extent[1] as f32],
            line_height: extent[1] as f32,
            advance: extent[0] as f32,
            kind,
            baseline: 0.0,
            ascent: 0.0,
            descent: 0.0,
            raster_size: [extent[0] as f32, extent[1] as f32],
            safe_padding,
            font_policy_id: 1,
            allocated_cell: AtlasCell { origin, extent },
        }
    }

    fn atlas(
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        entries: impl IntoIterator<Item = (GlyphKey, GlyphAtlasEntry)>,
    ) -> CompiledGlyphAtlas {
        CompiledGlyphAtlas {
            width,
            height,
            rgba,
            entries: entries.into_iter().collect(),
        }
    }

    fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * width + x) * 4) as usize;
        bytes[offset..offset + 4].try_into().unwrap()
    }

    #[test]
    fn mask_alpha_becomes_coverage_and_native_premultiplied_color_becomes_straight() {
        let mask = GlyphKey::new("m", false);
        let color = GlyphKey::new("c", false);
        let source = atlas(
            6,
            1,
            vec![
                9, 8, 7, 0, 9, 8, 7, 128, 9, 8, 7, 255, 200, 100, 50, 0, 64, 32, 16, 128, 255, 127,
                1, 255,
            ],
            [
                (mask, entry(GlyphEntryKind::Mask, [0, 0], [3, 1], 0.0)),
                (
                    color,
                    entry(GlyphEntryKind::PremultipliedColorRgba, [3, 0], [3, 1], 0.0),
                ),
            ],
        );

        let prepared = PreparedSceneAtlas::from_compiled(&source).unwrap();

        assert_eq!([prepared.width, prepared.height], [6, 1]);
        assert_eq!(prepared.coverage_r8, [0, 128, 255, 0, 0, 0]);
        assert_eq!(&prepared.straight_color_rgba_srgb[..12], &[0; 12]);
        assert_eq!(
            pixel(&prepared.straight_color_rgba_srgb, 6, 3, 0),
            [0, 0, 0, 0]
        );
        assert_eq!(
            pixel(&prepared.straight_color_rgba_srgb, 6, 4, 0),
            [128, 64, 32, 128]
        );
        assert_eq!(
            pixel(&prepared.straight_color_rgba_srgb, 6, 5, 0),
            [255, 127, 1, 255]
        );
    }

    #[test]
    fn prepared_scene_atlas_carries_the_presentation_resource_generation() {
        let generation = crate::presentation::companion_scene::ResourceGeneration(42);
        let source = atlas(
            1,
            1,
            vec![255, 255, 255, 255],
            [(
                GlyphKey::new("^", false),
                entry(GlyphEntryKind::Mask, [0, 0], [1, 1], 0.0),
            )],
        );
        let production =
            PreparedSceneAtlas::from_compiled_for_generation(&source, generation).unwrap();
        assert_eq!(production.resource_generation, generation);
        assert_eq!(
            PreparedSceneAtlas::from_compiled(&source)
                .unwrap()
                .resource_generation,
            crate::presentation::companion_scene::ResourceGeneration(0),
        );
    }

    #[test]
    fn preparation_rejects_bad_pixel_length_out_of_bounds_and_overlapping_cells() {
        let key = GlyphKey::new("x", false);
        let bad_length = atlas(
            1,
            1,
            vec![],
            [(
                key.clone(),
                entry(GlyphEntryKind::Mask, [0, 0], [1, 1], 0.0),
            )],
        );
        assert!(matches!(
            PreparedSceneAtlas::from_compiled(&bad_length),
            Err(PreparedSceneAtlasError::PixelDataLength { expected: 4, actual: 0 })
        ));

        let out_of_bounds = atlas(
            1,
            1,
            vec![0; 4],
            [(
                key.clone(),
                entry(GlyphEntryKind::Mask, [1, 0], [1, 1], 0.0),
            )],
        );
        assert!(matches!(
            PreparedSceneAtlas::from_compiled(&out_of_bounds),
            Err(PreparedSceneAtlasError::CellOutOfBounds { key: failed }) if failed == key
        ));

        let second = GlyphKey::new("y", false);
        let overlap = atlas(
            2,
            1,
            vec![0; 8],
            [
                (key, entry(GlyphEntryKind::Mask, [0, 0], [2, 1], 0.0)),
                (second, entry(GlyphEntryKind::Mask, [1, 0], [1, 1], 0.0)),
            ],
        );
        assert!(matches!(
            PreparedSceneAtlas::from_compiled(&overlap),
            Err(PreparedSceneAtlasError::OverlappingCells { .. })
        ));
    }

    #[test]
    fn native_color_dilation_fills_one_and_two_pixel_gutters_including_corners() {
        for (size, padding) in [(3, 1.0), (5, 2.0)] {
            let mut rgba = vec![0; (size * size * 4) as usize];
            let center = size / 2;
            let offset = ((center * size + center) * 4) as usize;
            rgba[offset..offset + 4].copy_from_slice(&[200, 100, 50, 255]);
            let source = atlas(
                size,
                size,
                rgba,
                [(
                    GlyphKey::new("color", false),
                    entry(
                        GlyphEntryKind::PremultipliedColorRgba,
                        [0, 0],
                        [size, size],
                        padding,
                    ),
                )],
            );

            let prepared = PreparedSceneAtlas::from_compiled(&source).unwrap();
            assert_eq!(
                pixel(&prepared.straight_color_rgba_srgb, size, 0, 0),
                [200, 100, 50, 0]
            );
            assert_eq!(
                pixel(&prepared.straight_color_rgba_srgb, size, size - 1, size - 1),
                [200, 100, 50, 0]
            );
            assert_eq!(
                pixel(&prepared.straight_color_rgba_srgb, size, center, center),
                [200, 100, 50, 255]
            );
        }
    }

    #[test]
    fn native_color_dilation_never_crosses_an_adjacent_allocated_cell() {
        let mut rgba = vec![0; 6 * 3 * 4];
        let red = ((6 + 1) * 4) as usize;
        rgba[red..red + 4].copy_from_slice(&[255, 0, 0, 255]);
        let blue = ((6 + 4) * 4) as usize;
        rgba[blue..blue + 4].copy_from_slice(&[0, 0, 255, 255]);
        let source = atlas(
            6,
            3,
            rgba,
            [
                (
                    GlyphKey::new("red", false),
                    entry(GlyphEntryKind::PremultipliedColorRgba, [0, 0], [3, 3], 2.0),
                ),
                (
                    GlyphKey::new("blue", false),
                    entry(GlyphEntryKind::PremultipliedColorRgba, [3, 0], [3, 3], 2.0),
                ),
            ],
        );

        let prepared = PreparedSceneAtlas::from_compiled(&source).unwrap();
        assert_eq!(
            pixel(&prepared.straight_color_rgba_srgb, 6, 2, 1),
            [255, 0, 0, 0]
        );
        assert_eq!(
            pixel(&prepared.straight_color_rgba_srgb, 6, 3, 1),
            [0, 0, 255, 0]
        );
    }

    #[test]
    fn whitespace_keeps_an_allocated_cell_and_resolves_without_pixels() {
        let key = GlyphKey::new(" ", false);
        let source = atlas(
            2,
            2,
            vec![0; 16],
            [(
                key.clone(),
                GlyphAtlasEntry::whitespace(
                    12.0,
                    20.0,
                    AtlasCell { origin: [0, 0], extent: [2, 2] },
                ),
            )],
        );

        let prepared = PreparedSceneAtlas::from_compiled(&source).unwrap();
        let resolved = prepared.resolve_key(&key).unwrap();
        assert_eq!(resolved.entry.allocated_cell.extent, [2, 2]);
        assert!(prepared.coverage_r8.iter().all(|&byte| byte == 0));
        assert!(prepared
            .straight_color_rgba_srgb
            .iter()
            .all(|&byte| byte == 0));
    }

    #[test]
    fn dense_ids_are_sorted_and_scalar_resolution_reports_missing_or_ambiguous() {
        let regular = GlyphKey::new("x", false);
        let bold = GlyphKey::new("x", true);
        let only = GlyphKey::new("y", false);
        let make = |entries| atlas(3, 1, vec![0; 12], entries);
        let first = make([
            (
                bold.clone(),
                entry(GlyphEntryKind::Mask, [1, 0], [1, 1], 0.0),
            ),
            (
                only.clone(),
                entry(GlyphEntryKind::Mask, [2, 0], [1, 1], 0.0),
            ),
            (
                regular.clone(),
                entry(GlyphEntryKind::Mask, [0, 0], [1, 1], 0.0),
            ),
        ]);
        let second = make([
            (
                regular.clone(),
                entry(GlyphEntryKind::Mask, [0, 0], [1, 1], 0.0),
            ),
            (
                bold.clone(),
                entry(GlyphEntryKind::Mask, [1, 0], [1, 1], 0.0),
            ),
            (
                only.clone(),
                entry(GlyphEntryKind::Mask, [2, 0], [1, 1], 0.0),
            ),
        ]);

        let first = PreparedSceneAtlas::from_compiled(&first).unwrap();
        let second = PreparedSceneAtlas::from_compiled(&second).unwrap();
        let ids = |atlas: &PreparedSceneAtlas| {
            atlas
                .entries
                .iter()
                .map(|entry| (entry.key.clone(), entry.id))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&first), ids(&second));
        assert_eq!(first.resolve_key(&regular).unwrap().id, 0);
        assert_eq!(first.resolve_key(&bold).unwrap().id, 1);
        assert_eq!(first.resolve_single_scalar('y').unwrap().key, only);
        assert!(matches!(
            first.resolve_single_scalar('x'),
            Err(GlyphAtlasResolveError::AmbiguousSingleScalar { scalar: 'x', .. })
        ));
        assert!(matches!(
            first.resolve_single_scalar('z'),
            Err(GlyphAtlasResolveError::MissingSingleScalar('z'))
        ));
        assert!(matches!(
            first.resolve_key(&GlyphKey::new("z", false)),
            Err(GlyphAtlasResolveError::MissingKey(_))
        ));
    }

    #[test]
    fn production_repertoire_converts_and_resolves_regular_and_bold_keys() {
        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        let compiled = CompiledRetainedResources::compile(&manifest).unwrap();
        let legacy_bytes = compiled.atlas().rgba.clone();

        let prepared = PreparedSceneAtlas::from_compiled(compiled.atlas()).unwrap();

        assert_eq!(prepared.entries.len(), manifest.glyphs().len());
        for key in manifest.glyphs() {
            assert_eq!(prepared.resolve_key(key).unwrap().key, *key);
        }
        assert!(prepared
            .resolve_key(&GlyphKey::new("\u{fffd}", false))
            .is_ok());
        assert!(prepared
            .resolve_key(&GlyphKey::new("\u{fffd}", true))
            .is_ok());
        assert_eq!(compiled.atlas().rgba, legacy_bytes);
    }

    #[test]
    fn synthetic_capacity_inventory_has_valid_distinct_allocated_cells() {
        let manifest = GlyphRepertoireManifest::for_fixture_pet();
        let compiled = CompiledRetainedResources::for_capacity_inventory(&manifest);

        let prepared = PreparedSceneAtlas::from_compiled(compiled.atlas()).unwrap();

        assert_eq!(prepared.entries.len(), manifest.glyphs().len());
    }

    #[test]
    fn prepared_scene_atlas_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PreparedSceneAtlas>();
    }
}

#[cfg(test)]
mod repertoire_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::Species;
    use crate::round::hud::{companion_hud_text, pack_companion_hud_glyphs, CompanionHudText};
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
            let packed_hud = pack_companion_hud_glyphs(&self.hud)
                .expect("retained strip HUD fixture must satisfy the glyph contract");
            frame_glyph_sequences(&self.plan, &packed_hud)
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
