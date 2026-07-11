//! Frozen, checksummable paired review frames and capture-path privacy policy.
//!
//! A [`PairedReviewFrame`] freezes a prepared companion frame together with a
//! serializable [`PairedReviewIdentity`] and a precomputed content checksum. The
//! identity captures every chrome/geometry input that a reviewer compares across
//! two renderers, projected into a stable serde form; the checksum is a
//! lowercase SHA-256 over ONLY that identity, so it changes whenever the frozen
//! frame's chrome or geometry changes and stays byte-stable otherwise.
//!
//! [`validate_review_output`] enforces where a capture may be written. The
//! privacy policy keeps redacted-HUD captures under `target/glorp-review` and
//! confines sensitive live-value captures to `target/glorp-review-sensitive`,
//! rejecting absolute destinations, parent-directory escapes, symlink traversal,
//! and any destination that resolves outside the review root.

use std::fmt::Debug;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::commands::companion_mode::{
    CompanionRendererRequest, EffectiveCompanionRenderer, RendererRuntimeState,
};
use crate::companion::app::{CompanionGridMetrics, PreparedCompanionFrame, PreparedGaugeFrame};
use crate::companion::retained::{ActiveRetainedHost, FrameDisposition, FrameMilestone};
use crate::presentation::draw_list::SceneDrawList;
use crate::presentation::pixel::PixelFrame;
use crate::presentation::smooth::SmoothCompanionScenePlan;
use crate::round::draw::{RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::round::hud::CompanionHudText;
use crate::round::layout::RoundAperture;

/// A frozen companion frame: a cloned [`PreparedCompanionFrame`], the
/// serializable identity projected from it plus its sampling context, and a
/// precomputed content checksum over that identity.
#[derive(Debug, Clone)]
pub struct PairedReviewFrame {
    /// The immutable prepared frame this review row was frozen from. Retained so
    /// the GPU readback can repaint the exact same scene into its capture
    /// intermediate; exposed through [`PairedReviewFrame::prepared`].
    frame: PreparedCompanionFrame,
    pub identity: PairedReviewIdentity,
    pub checksum: String,
}

impl PairedReviewFrame {
    /// Freezes a prepared frame with its sampling context into a checksummed
    /// review row. Exposes only the minimum prepared-frame accessors it needs;
    /// no live `AppState` or `WatchViewModel` state is projected.
    #[allow(clippy::too_many_arguments)] // Explicit sampling context stays flat.
    pub(super) fn from_prepared(
        frame: PreparedCompanionFrame,
        backing_scale: f64,
        logical: ReviewFrameDimensions,
        physical: ReviewFrameDimensions,
        semantic_tick: u64,
        elapsed_ms: u64,
        frame_id: u64,
        resource_generation: u64,
    ) -> Self {
        let identity = PairedReviewIdentity {
            renderer: RendererIdentity::from_source(frame.renderer_source()),
            aperture: ApertureIdentity::from_aperture(frame.review_aperture()),
            background: color_channels(frame.review_background()),
            mood_aura: color_channels(frame.review_mood_aura()),
            dim_overlay: frame.review_dim_overlay(),
            gauges: GaugeIdentity::from_gauges(frame.review_gauges()),
            hud: HudIdentity::from_hud(frame.review_hud()),
            hud_font_size: frame.review_hud_font_size(),
            overlays: frame
                .review_overlays()
                .iter()
                .map(OverlayIdentity::from_command)
                .collect(),
            logical,
            physical,
            backing_scale,
            semantic_tick,
            elapsed_ms,
            frame_id,
            resource_generation,
        };
        let checksum = canonical_frame_checksum(&identity);
        Self { frame, identity, checksum }
    }

    /// Recomputes the checksum from the current identity. Freezing sets
    /// [`Self::checksum`]; this recomputes it so a caller can detect that the
    /// identity was mutated away from the frozen value.
    pub fn recompute_checksum(&self) -> String {
        canonical_frame_checksum(&self.identity)
    }

    /// The frozen prepared frame, handed to the GPU readback so it can repaint
    /// the exact scene this review row was frozen from. The frame/generation
    /// IDs the readback stamps onto its artifact come from
    /// [`Self::identity`]`.frame_id` / `.resource_generation`.
    #[allow(dead_code)] // Consumed by the Task 7 paired-capture coordinator.
    pub(super) fn prepared(&self) -> &PreparedCompanionFrame {
        &self.frame
    }

    /// A deterministic paired review frame for tests and design review. Built
    /// entirely from constants, never from live pet state.
    pub fn fixture() -> Self {
        Self::from_prepared(
            PreparedCompanionFrame::fixture(),
            2.0,
            ReviewFrameDimensions { width: 360.0, height: 360.0 },
            ReviewFrameDimensions { width: 720.0, height: 720.0 },
            7,
            250,
            3,
            1,
        )
    }
}

/// A serializable projection of every frozen chrome and geometry input a
/// reviewer compares between two renderers. Field order is the declaration order
/// (a derived `Serialize`), so the serialization is deterministic.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PairedReviewIdentity {
    pub renderer: RendererIdentity,
    pub aperture: ApertureIdentity,
    pub background: [f32; 4],
    pub mood_aura: [f32; 4],
    pub dim_overlay: bool,
    pub gauges: GaugeIdentity,
    pub hud: HudIdentity,
    pub hud_font_size: f64,
    pub overlays: Vec<OverlayIdentity>,
    pub logical: ReviewFrameDimensions,
    pub physical: ReviewFrameDimensions,
    pub backing_scale: f64,
    pub semantic_tick: u64,
    pub elapsed_ms: u64,
    pub frame_id: u64,
    pub resource_generation: u64,
}

/// Logical (points) or physical (pixels) frame dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ReviewFrameDimensions {
    pub width: f64,
    pub height: f64,
}

/// The four normalized gauge fractions, mirrored from `PreparedGaugeFrame` into
/// a public serde form so the identity never leaks the internal companion type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GaugeIdentity {
    pub xp_fraction: f64,
    pub daily_fraction: f64,
    pub daily_overage_fraction: f64,
    pub pace_fraction: f64,
}

impl GaugeIdentity {
    fn from_gauges(gauges: PreparedGaugeFrame) -> Self {
        Self {
            xp_fraction: gauges.xp_fraction,
            daily_fraction: gauges.daily_fraction,
            daily_overage_fraction: gauges.daily_overage_fraction,
            pace_fraction: gauges.pace_fraction,
        }
    }
}

/// The round aperture projected into its numeric fields.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ApertureIdentity {
    pub width: u16,
    pub height: u16,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

impl ApertureIdentity {
    fn from_aperture(aperture: RoundAperture) -> Self {
        Self {
            width: aperture.width,
            height: aperture.height,
            center_x: aperture.center_x,
            center_y: aperture.center_y,
            radius: aperture.radius,
        }
    }
}

/// The companion grid metrics projected into their numeric fields.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MetricsIdentity {
    pub font_size: f64,
    pub cell_w: f64,
    pub cell_h: f64,
    pub grid_cols: u16,
    pub grid_rows: u16,
    pub origin_x: f64,
    pub origin_y: f64,
}

impl MetricsIdentity {
    fn from_metrics(metrics: CompanionGridMetrics) -> Self {
        Self {
            font_size: metrics.font_size,
            cell_w: metrics.cell_w,
            cell_h: metrics.cell_h,
            grid_cols: metrics.grid_cols,
            grid_rows: metrics.grid_rows,
            origin_x: metrics.origin_x,
            origin_y: metrics.origin_y,
        }
    }
}

/// The HUD text lines carried by the frozen frame. These are already whatever
/// the frame preparation chose (redacted or live); the identity only records
/// them for checksumming and never writes them to a capture artifact itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HudIdentity {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

impl HudIdentity {
    fn from_hud(hud: &CompanionHudText) -> Self {
        Self {
            today_total: hud.today_total.clone(),
            daily_percent: hud.daily_percent.clone(),
            pace: hud.pace.clone(),
        }
    }
}

/// A stable serde projection of one overlay draw command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OverlayIdentity {
    pub kind: &'static str,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<char>,
    pub text: Option<String>,
    pub span_count: usize,
    pub color: [f32; 4],
}

impl OverlayIdentity {
    fn from_command(command: &RoundDrawCommand) -> Self {
        Self {
            kind: round_draw_kind_tag(command.kind),
            x: command.x,
            y: command.y,
            radius: command.radius,
            label: command.label,
            text: command.text.clone(),
            span_count: command.spans.len(),
            color: color_channels(command.color),
        }
    }
}

/// The renderer-specific identity: plan checksum, draw order, and metrics live
/// here because they only exist for the renderers that produce them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RendererIdentity {
    Pixel {
        pixel_checksum: String,
    },
    Classic {
        metrics: MetricsIdentity,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        draw_list_checksum: String,
    },
    Smooth {
        metrics: MetricsIdentity,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        plan_checksum: String,
        draw_order: Vec<usize>,
    },
}

impl RendererIdentity {
    fn from_source(source: RendererIdentitySource<'_>) -> Self {
        match source {
            RendererIdentitySource::Pixel { frame } => {
                RendererIdentity::Pixel { pixel_checksum: debug_checksum(frame) }
            }
            RendererIdentitySource::Classic {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                draw_list,
            } => RendererIdentity::Classic {
                metrics: MetricsIdentity::from_metrics(metrics),
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                draw_list_checksum: debug_checksum(draw_list),
            },
            RendererIdentitySource::Smooth {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                plan,
                draw_order,
            } => RendererIdentity::Smooth {
                metrics: MetricsIdentity::from_metrics(metrics),
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                plan_checksum: debug_checksum(plan),
                draw_order: draw_order.to_vec(),
            },
        }
    }
}

/// A borrowed view of a prepared frame's renderer payload, handed out by
/// [`PreparedCompanionFrame::renderer_source`] so this module can project the
/// renderer identity without app.rs depending on the serde projection types.
pub(super) enum RendererIdentitySource<'a> {
    Pixel {
        frame: &'a PixelFrame,
    },
    Classic {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        draw_list: &'a SceneDrawList,
    },
    Smooth {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        plan: &'a SmoothCompanionScenePlan,
        draw_order: &'a [usize],
    },
}

/// Lowercase hex SHA-256 over a deterministic serialization of ONLY the frozen
/// identity. It never mixes in the live `WatchViewModel`, `AppState`, wall-clock
/// time, or any input outside the frozen frame.
pub fn canonical_frame_checksum(identity: &PairedReviewIdentity) -> String {
    let serialized =
        serde_json::to_vec(identity).expect("paired review identity is always serializable");
    sha256_hex(&serialized)
}

/// Where a capture may be written, and how sensitive its HUD is. Redacted-HUD is
/// the default; sensitive live values are opt-in and confined to a separate root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapturePrivacy {
    /// The HUD is redacted; captures land under `target/glorp-review`.
    #[default]
    RedactedHud,
    /// The HUD shows live values; captures are confined to
    /// `target/glorp-review-sensitive`.
    SensitiveLiveValues,
}

impl CapturePrivacy {
    /// Maps the opt-in `--review-capture-live-values` flag onto the policy.
    pub fn from_live_values(review_capture_live_values: bool) -> Self {
        if review_capture_live_values {
            CapturePrivacy::SensitiveLiveValues
        } else {
            CapturePrivacy::RedactedHud
        }
    }

    /// The repo-relative review root this policy confines captures to.
    pub fn review_root(self) -> &'static str {
        match self {
            CapturePrivacy::RedactedHud => "target/glorp-review",
            CapturePrivacy::SensitiveLiveValues => "target/glorp-review-sensitive",
        }
    }
}

/// Why a proposed review-capture destination was rejected. Every variant is a
/// static category, so no user-derived path text reaches a diagnostic surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutputError {
    /// The destination was absolute; review captures must be repo-relative.
    AbsoluteDestination,
    /// The destination contained a `..`, root, or drive-prefix component.
    NonLexicalComponent,
    /// The destination did not resolve strictly under the review root.
    OutsideReviewRoot,
    /// A component of the destination resolved through a symlink.
    SymlinkTraversal,
    /// The repository root could not be canonicalized.
    RepoUnavailable,
}

impl ReviewOutputError {
    /// A sanitized, static category string for privacy-safe logging.
    pub fn category(self) -> &'static str {
        match self {
            ReviewOutputError::AbsoluteDestination => "absolute-destination",
            ReviewOutputError::NonLexicalComponent => "non-lexical-component",
            ReviewOutputError::OutsideReviewRoot => "outside-review-root",
            ReviewOutputError::SymlinkTraversal => "symlink-traversal",
            ReviewOutputError::RepoUnavailable => "repo-unavailable",
        }
    }
}

/// Validates a proposed review-capture destination against the privacy policy
/// and returns the resolved, symlink-free absolute path on success.
///
/// The checks are ordered cheapest-and-lexical first, then filesystem-resolving:
///
/// 1. **Absolute reject** — `dest` must be repo-relative.
/// 2. **Component reject** — every component must be `Normal` (or the harmless
///    leading `.`); any `..`, root, or prefix component is rejected lexically,
///    before any path is joined or touched.
/// 3. **Lexical containment** — `dest` must start with the policy's review root
///    subdirectory. This rejects a differently-named sibling (including a
///    symlink under a different name) without touching the filesystem.
/// 4. **Symlink defense** — after canonicalizing the repo root, the deepest
///    EXISTING ancestor of the repo-joined destination is canonicalized and
///    required to equal its lexical (symlink-free) form. Because the trailing
///    non-existent components are all `Normal` (step 2), a symlink anywhere in
///    the existing prefix makes the canonical form diverge and is rejected. The
///    capture leaf itself need not exist yet.
/// 5. **Canonical containment** — the resolved destination must be a strict
///    descendant of the canonicalized review root.
pub fn validate_review_output(
    repo: &Path,
    dest: &Path,
    privacy: CapturePrivacy,
) -> Result<PathBuf, ReviewOutputError> {
    // 1. Absolute destinations are rejected outright (lexical).
    if dest.is_absolute() {
        return Err(ReviewOutputError::AbsoluteDestination);
    }

    // 2. Only in-place `Normal`/`CurDir` components are allowed (lexical). A
    //    `..`, root, or drive prefix can escape the review root and is rejected
    //    before the path is ever joined or resolved.
    for component in dest.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ReviewOutputError::NonLexicalComponent);
            }
        }
    }

    // 3. The destination must live under the policy's review root by name
    //    (lexical). This rejects any differently-named path, so a symlink under
    //    a different name never even reaches the filesystem checks below.
    let review_root_rel = Path::new(privacy.review_root());
    if !dest.starts_with(review_root_rel) {
        return Err(ReviewOutputError::OutsideReviewRoot);
    }

    // 4. Canonicalize the repo root, then walk the repo-joined destination up to
    //    its deepest existing ancestor and require that ancestor to be
    //    symlink-free (its canonical form equals its lexical form).
    let repo_canonical = repo
        .canonicalize()
        .map_err(|_| ReviewOutputError::RepoUnavailable)?;
    let resolved = repo_canonical.join(dest);

    let mut existing = resolved.as_path();
    loop {
        match existing.canonicalize() {
            Ok(canonical) => {
                if canonical != existing {
                    return Err(ReviewOutputError::SymlinkTraversal);
                }
                break;
            }
            Err(_) => match existing.parent() {
                Some(parent) => existing = parent,
                None => return Err(ReviewOutputError::OutsideReviewRoot),
            },
        }
    }

    // 5. The resolved destination must be a strict descendant of the
    //    canonicalized review root. Steps 2–4 guarantee it has no `..` and a
    //    symlink-free existing prefix, so this lexical check on the canonical
    //    repo path is a true containment guarantee.
    let review_root = repo_canonical.join(review_root_rel);
    if !resolved.starts_with(&review_root) || resolved == review_root {
        return Err(ReviewOutputError::OutsideReviewRoot);
    }

    Ok(resolved)
}

/// Projects a `RoundColor` into its channel array.
fn color_channels(color: RoundColor) -> [f32; 4] {
    [color.0, color.1, color.2, color.3]
}

/// A static tag for an overlay draw kind.
fn round_draw_kind_tag(kind: RoundDrawKind) -> &'static str {
    match kind {
        RoundDrawKind::Background => "background",
        RoundDrawKind::Halo => "halo",
        RoundDrawKind::Trouble => "trouble",
    }
}

/// A deterministic SHA-256 (lowercase hex) over the `Debug` rendering of a
/// value. Used for the renderer payloads that do not implement `Serialize`; the
/// derived `Debug` of their fixed-precision numeric fields is stable.
fn debug_checksum<T: Debug>(value: &T) -> String {
    sha256_hex(format!("{value:?}").as_bytes())
}

/// Lowercase hex SHA-256 of the given bytes.
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The only pair-manifest schema this build reads or writes.
pub const PAIR_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// A machine-verifiable record of one paired Smooth/Retained capture. It carries
/// only redacted metadata — renderer names, kebab-case milestones and terminal
/// dispositions, resource counters, the frozen-frame checksum, and the two
/// artifact paths. It NEVER records the frozen frame's HUD strings; the checksum
/// is the sole (one-way) witness of the frame's chrome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairManifest {
    pub schema_version: u32,
    /// `"success"` only when both artifacts were produced and the retained
    /// readback completed; any capture fault flips this to `"failed"`.
    pub status: String,
    pub requested_renderer: String,
    pub effective_renderer: String,
    pub compiled_capabilities: Vec<String>,
    pub frame_checksum: String,
    pub logical: PairDimensions,
    pub physical: PairDimensions,
    pub backing_scale: f64,
    pub frame_id: u64,
    pub resource_generation: u64,
    pub fallback_transition_count: u64,
    pub last_fallback_reason: Option<String>,
    pub privacy_mode: String,
    pub smooth: PairRendererSection,
    pub retained: PairRendererSection,
}

/// Logical (points) or physical (pixels) dimensions recorded in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairDimensions {
    pub width: f64,
    pub height: f64,
}

/// The per-renderer half of a paired capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRendererSection {
    pub effective_renderer: String,
    pub milestones: Vec<String>,
    pub terminal_disposition: String,
    pub frame_id: u64,
    pub resource_generation: u64,
    pub resource_counters: PairResourceCounters,
    pub png_path: String,
}

/// The concrete artifact counters a reviewer or validator can cross-check.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairResourceCounters {
    pub captured_width: u32,
    pub captured_height: u32,
}

impl PairManifest {
    /// A deterministic, valid paired manifest for tests. Both sections are frozen
    /// from the same identity, so [`validate_review_pair`] accepts it.
    pub fn fixture() -> Self {
        Self {
            schema_version: PAIR_MANIFEST_SCHEMA_VERSION,
            status: "success".to_string(),
            requested_renderer: "retained".to_string(),
            effective_renderer: "retained".to_string(),
            compiled_capabilities: vec!["retained-renderer".to_string()],
            frame_checksum: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            logical: PairDimensions { width: 360.0, height: 360.0 },
            physical: PairDimensions { width: 720.0, height: 720.0 },
            backing_scale: 2.0,
            frame_id: 3,
            resource_generation: 1,
            fallback_transition_count: 0,
            last_fallback_reason: None,
            privacy_mode: "redacted".to_string(),
            smooth: PairRendererSection {
                effective_renderer: "smooth".to_string(),
                milestones: vec!["surface-present-called".to_string()],
                terminal_disposition: "smooth-painted".to_string(),
                frame_id: 3,
                resource_generation: 1,
                resource_counters: PairResourceCounters {
                    captured_width: 720,
                    captured_height: 720,
                },
                png_path: "smooth.png".to_string(),
            },
            retained: PairRendererSection {
                effective_renderer: "retained".to_string(),
                milestones: vec![
                    "gpu-completed".to_string(),
                    "readback-completed".to_string(),
                ],
                terminal_disposition: "captured".to_string(),
                frame_id: 3,
                resource_generation: 1,
                resource_counters: PairResourceCounters {
                    captured_width: 720,
                    captured_height: 720,
                },
                png_path: "retained.png".to_string(),
            },
        }
    }
}

/// Validates a paired-capture manifest. A Smooth-fallback retained section can
/// never satisfy a retained capture, and a retained artifact is only trustworthy
/// once the GPU work and its readback are both observed. Beyond those two
/// contract checks, this hardens the manifest: the sections must be frozen from
/// one frame (matching frame/generation IDs and overlaid physical dimensions),
/// the Smooth section must actually be Smooth, and — for an integral backing
/// scale — the physical size must equal the logical size times that scale.
pub fn validate_review_pair(manifest: &PairManifest) -> Result<(), String> {
    if manifest.schema_version != PAIR_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported pair manifest schema version {}",
            manifest.schema_version
        ));
    }
    if manifest.status != "success" {
        return Err(format!("paired capture status is {}", manifest.status));
    }
    // A retained section that fell back to Smooth is a Smooth capture wearing a
    // retained label; it can never be the retained half of the pair.
    if manifest.retained.effective_renderer == "smooth" {
        return Err("retained capture fell back to the smooth renderer".to_string());
    }
    if manifest.retained.effective_renderer != "retained" {
        return Err(format!(
            "retained section reports the {} renderer",
            manifest.retained.effective_renderer
        ));
    }
    if !manifest
        .retained
        .milestones
        .iter()
        .any(|milestone| milestone == "readback-completed")
    {
        return Err("retained capture missing readback-completed".to_string());
    }
    if !manifest
        .retained
        .milestones
        .iter()
        .any(|milestone| milestone == "gpu-completed")
    {
        return Err("retained capture missing gpu-completed".to_string());
    }
    if manifest.retained.terminal_disposition != "captured" {
        return Err(format!(
            "retained capture terminal disposition is {}",
            manifest.retained.terminal_disposition
        ));
    }
    if manifest.smooth.effective_renderer != "smooth" {
        return Err(format!(
            "smooth section reports the {} renderer",
            manifest.smooth.effective_renderer
        ));
    }
    if manifest.smooth.frame_id != manifest.retained.frame_id
        || manifest.smooth.frame_id != manifest.frame_id
    {
        return Err("paired sections disagree on the frozen frame id".to_string());
    }
    if manifest.smooth.resource_generation != manifest.retained.resource_generation
        || manifest.smooth.resource_generation != manifest.resource_generation
    {
        return Err("paired sections disagree on the resource generation".to_string());
    }
    // Both artifacts must overlay, so their captured pixel dimensions must match.
    if manifest.smooth.resource_counters != manifest.retained.resource_counters {
        return Err("paired captures differ in physical dimensions".to_string());
    }
    // Physical == logical × backing scale, verified exactly only for an integral
    // scale (fractional scales round and are checked by the native run).
    if manifest.backing_scale.fract() == 0.0 {
        let expected_width = manifest.logical.width * manifest.backing_scale;
        let expected_height = manifest.logical.height * manifest.backing_scale;
        if (expected_width - manifest.physical.width).abs() > f64::EPSILON
            || (expected_height - manifest.physical.height).abs() > f64::EPSILON
        {
            return Err(
                "physical size does not equal logical size times the integral backing scale"
                    .to_string(),
            );
        }
        if f64::from(manifest.retained.resource_counters.captured_width) != manifest.physical.width
            || f64::from(manifest.retained.resource_counters.captured_height)
                != manifest.physical.height
        {
            return Err(
                "captured pixel dimensions do not match the declared physical size".to_string(),
            );
        }
    }
    Ok(())
}

/// The stable kebab-case tag for a retained frame milestone. Recorded verbatim in
/// the manifest so a validator can assert the readback ladder without importing
/// the internal enum.
pub(crate) const fn milestone_kebab(milestone: FrameMilestone) -> &'static str {
    match milestone {
        FrameMilestone::Prepared => "prepared",
        FrameMilestone::Encoded => "encoded",
        FrameMilestone::Submitted => "submitted",
        FrameMilestone::SurfacePresentCalled => "surface-present-called",
        FrameMilestone::GpuCompleted => "gpu-completed",
        FrameMilestone::ReadbackCompleted => "readback-completed",
    }
}

/// The stable kebab-case tag for a terminal frame disposition. A failed
/// disposition maps to its static failure category so the manifest carries the
/// specific fault without dynamic text.
pub(crate) fn disposition_kebab(disposition: FrameDisposition) -> &'static str {
    match disposition {
        FrameDisposition::SurfacePresentCalled => "surface-present-called",
        FrameDisposition::Captured => "captured",
        FrameDisposition::Skipped(_) => "skipped",
        FrameDisposition::Failed(category) => category.category(),
        FrameDisposition::FallbackPending(_) => "fallback-pending",
        FrameDisposition::FallbackPainted(_) => "fallback-painted",
    }
}

impl CapturePrivacy {
    /// The manifest tag for this privacy policy.
    pub(crate) const fn manifest_tag(self) -> &'static str {
        match self {
            CapturePrivacy::RedactedHud => "redacted",
            CapturePrivacy::SensitiveLiveValues => "sensitive-live-values",
        }
    }
}

/// The manifest tag for a renderer request. `Auto` is recorded as `"auto"` so the
/// manifest shows the operator's intent even before resolution.
fn renderer_request_tag(request: CompanionRendererRequest) -> &'static str {
    request.forwarded_arg().unwrap_or("auto")
}

/// Produces a paired Smooth/Retained capture from ONE frozen review frame.
///
/// The coordinator consumes a single [`PairedReviewFrame`]: it paints that exact
/// frozen frame into an off-screen bitmap for the Smooth artifact and reads the
/// same frozen frame back off the GPU for the Retained artifact, so the two PNGs
/// overlay pixel-for-pixel. Neither path rebuilds the scene or reads an
/// `AppState` clock. A retained readback fault is recorded as the retained
/// terminal disposition, flips the manifest to `"failed"`, and surfaces as a
/// `GlorpError` — a Smooth fallback therefore cannot masquerade as a retained
/// capture.
pub(super) struct PairedCaptureCoordinator<'a> {
    frame: &'a PairedReviewFrame,
    host: &'a mut ActiveRetainedHost,
    runtime: &'a RendererRuntimeState,
    privacy: CapturePrivacy,
    out_dir: PathBuf,
}

impl<'a> PairedCaptureCoordinator<'a> {
    pub(super) fn new(
        frame: &'a PairedReviewFrame,
        host: &'a mut ActiveRetainedHost,
        runtime: &'a RendererRuntimeState,
        privacy: CapturePrivacy,
        out_dir: PathBuf,
    ) -> Self {
        Self { frame, host, runtime, privacy, out_dir }
    }

    /// Captures both renderers, writes `smooth.png`, `retained.png`, and
    /// `pair-manifest.json`, then returns `Ok` only when the written manifest is a
    /// successful pair that passes [`validate_review_pair`].
    pub(super) fn run(self) -> crate::error::Result<()> {
        std::fs::create_dir_all(&self.out_dir)?;
        let identity = &self.frame.identity;
        let physical_width = identity.physical.width.round() as u32;
        let physical_height = identity.physical.height.round() as u32;

        // Smooth half: repaint the frozen frame into an off-screen bitmap.
        let smooth_rgba = crate::companion::app::render_prepared_frame_to_rgba(
            self.frame.prepared(),
            physical_width,
            physical_height,
            identity.backing_scale,
        )?;
        write_rgba_png(
            &self.out_dir.join("smooth.png"),
            physical_width,
            physical_height,
            &smooth_rgba,
        )?;
        let smooth_counters = PairResourceCounters {
            captured_width: physical_width,
            captured_height: physical_height,
        };
        let smooth = PairRendererSection {
            effective_renderer: EffectiveCompanionRenderer::Smooth.as_str().to_string(),
            milestones: vec![milestone_kebab(FrameMilestone::SurfacePresentCalled).to_string()],
            terminal_disposition: "smooth-painted".to_string(),
            frame_id: identity.frame_id,
            resource_generation: identity.resource_generation,
            resource_counters: smooth_counters,
            png_path: "smooth.png".to_string(),
        };

        // Retained half: GPU readback of the SAME frozen frame.
        let (retained, status) = match self.host.capture(self.frame) {
            Ok(captured) => {
                write_rgba_png(
                    &self.out_dir.join("retained.png"),
                    captured.width(),
                    captured.height(),
                    captured.rgba(),
                )?;
                let retained = PairRendererSection {
                    effective_renderer: EffectiveCompanionRenderer::Retained.as_str().to_string(),
                    // A successful capture polled the GPU submission to completion
                    // and read it back, so both milestones are observed.
                    milestones: vec![
                        milestone_kebab(FrameMilestone::GpuCompleted).to_string(),
                        milestone_kebab(FrameMilestone::ReadbackCompleted).to_string(),
                    ],
                    terminal_disposition: disposition_kebab(FrameDisposition::Captured).to_string(),
                    frame_id: captured.frame_id(),
                    resource_generation: captured.resource_generation(),
                    resource_counters: PairResourceCounters {
                        captured_width: captured.width(),
                        captured_height: captured.height(),
                    },
                    png_path: "retained.png".to_string(),
                };
                (retained, "success")
            }
            Err(category) => {
                let retained = PairRendererSection {
                    effective_renderer: EffectiveCompanionRenderer::Retained.as_str().to_string(),
                    milestones: Vec::new(),
                    terminal_disposition: disposition_kebab(FrameDisposition::Failed(category))
                        .to_string(),
                    frame_id: identity.frame_id,
                    resource_generation: identity.resource_generation,
                    resource_counters: smooth_counters,
                    png_path: "retained.png".to_string(),
                };
                (retained, "failed")
            }
        };

        let manifest = PairManifest {
            schema_version: PAIR_MANIFEST_SCHEMA_VERSION,
            status: status.to_string(),
            requested_renderer: renderer_request_tag(self.runtime.requested()).to_string(),
            effective_renderer: self.runtime.effective().as_str().to_string(),
            compiled_capabilities: vec!["retained-renderer".to_string()],
            frame_checksum: self.frame.checksum.clone(),
            logical: PairDimensions {
                width: identity.logical.width,
                height: identity.logical.height,
            },
            physical: PairDimensions {
                width: identity.physical.width,
                height: identity.physical.height,
            },
            backing_scale: identity.backing_scale,
            frame_id: identity.frame_id,
            resource_generation: identity.resource_generation,
            fallback_transition_count: self.runtime.transition_count(),
            last_fallback_reason: self.runtime.last_fallback_reason().map(str::to_string),
            privacy_mode: self.privacy.manifest_tag().to_string(),
            smooth,
            retained,
        };

        let json = serde_json::to_vec_pretty(&manifest)?;
        std::fs::write(self.out_dir.join("pair-manifest.json"), json)?;

        if status != "success" {
            return Err(crate::error::GlorpError::Message(format!(
                "paired retained capture failed: {}",
                manifest.retained.terminal_disposition
            )));
        }
        validate_review_pair(&manifest).map_err(|reason| {
            crate::error::GlorpError::Message(format!("paired capture manifest invalid: {reason}"))
        })?;
        Ok(())
    }
}

/// Writes straight RGBA8 bytes as a PNG. Task 11 canonicalizes color/orientation;
/// this writes the raw captured bytes at the given physical dimensions.
fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> crate::error::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba))
        .map_err(|error| {
            crate::error::GlorpError::Message(format!("paired capture png failed: {error}"))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion::retained::RetainedFailureCategory;

    #[test]
    fn frozen_frame_checksum_changes_with_chrome_or_geometry() {
        let frame = PairedReviewFrame::fixture();
        let mut changed = frame.clone();
        changed.identity.gauges.pace_fraction = 0.75;
        assert_ne!(frame.checksum, changed.recompute_checksum());
    }

    #[test]
    fn sensitive_capture_rejects_escape_and_symlink() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        std::fs::create_dir_all(repo.join("target/glorp-review-sensitive")).unwrap();
        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/glorp-review-sensitive/pair"),
            CapturePrivacy::SensitiveLiveValues,
        )
        .is_ok());
        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/glorp-review-sensitive/../../private"),
            CapturePrivacy::SensitiveLiveValues,
        )
        .is_err());
        std::os::unix::fs::symlink(
            repo.join("target/glorp-review-sensitive"),
            repo.join("target/review-link"),
        )
        .unwrap();
        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/review-link/pair"),
            CapturePrivacy::SensitiveLiveValues,
        )
        .is_err());
        assert!(validate_review_output(
            repo,
            repo.join("target/glorp-review-sensitive/pair").as_path(),
            CapturePrivacy::SensitiveLiveValues,
        )
        .is_err());
    }

    #[test]
    fn capture_privacy_defaults_to_redacted_and_maps_the_opt_in_flag() {
        assert_eq!(CapturePrivacy::default(), CapturePrivacy::RedactedHud);
        assert_eq!(
            CapturePrivacy::from_live_values(false),
            CapturePrivacy::RedactedHud
        );
        assert_eq!(
            CapturePrivacy::from_live_values(true),
            CapturePrivacy::SensitiveLiveValues
        );
        assert_eq!(
            CapturePrivacy::RedactedHud.review_root(),
            "target/glorp-review"
        );
        assert_eq!(
            CapturePrivacy::SensitiveLiveValues.review_root(),
            "target/glorp-review-sensitive"
        );
    }

    #[test]
    fn redacted_capture_accepts_default_root_and_rejects_escapes() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        std::fs::create_dir_all(repo.join("target/glorp-review")).unwrap();

        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/glorp-review/pair"),
            CapturePrivacy::RedactedHud,
        )
        .is_ok());
        // A sensitive-root destination is not the redacted root.
        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/glorp-review-sensitive/pair"),
            CapturePrivacy::RedactedHud,
        )
        .is_err());
        // Absolute and parent-escape destinations are rejected here too.
        assert!(validate_review_output(
            repo,
            repo.join("target/glorp-review/pair").as_path(),
            CapturePrivacy::RedactedHud,
        )
        .is_err());
        assert!(validate_review_output(
            repo,
            std::path::Path::new("target/glorp-review/../../escape"),
            CapturePrivacy::RedactedHud,
        )
        .is_err());
    }

    #[test]
    fn sensitive_capture_rejects_a_symlinked_review_root() {
        // The mandated test's symlink has a different NAME, so the lexical root
        // check catches it. This exercises the canonical defense directly: the
        // review root itself is a symlink pointing outside the repo, so its
        // name matches but its canonical form diverges.
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let repo = root.path();
        std::fs::create_dir_all(repo.join("target")).unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.join("target/glorp-review-sensitive"))
            .unwrap();

        assert_eq!(
            validate_review_output(
                repo,
                std::path::Path::new("target/glorp-review-sensitive/pair"),
                CapturePrivacy::SensitiveLiveValues,
            ),
            Err(ReviewOutputError::SymlinkTraversal),
        );
    }

    #[test]
    fn sensitive_capture_rejects_the_review_root_itself() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        std::fs::create_dir_all(repo.join("target/glorp-review-sensitive")).unwrap();

        assert_eq!(
            validate_review_output(
                repo,
                std::path::Path::new("target/glorp-review-sensitive"),
                CapturePrivacy::SensitiveLiveValues,
            ),
            Err(ReviewOutputError::OutsideReviewRoot),
        );
    }

    #[test]
    fn recompute_matches_the_frozen_checksum_when_unmutated() {
        let frame = PairedReviewFrame::fixture();
        assert_eq!(frame.checksum, frame.recompute_checksum());
        // The checksum is lowercase hex SHA-256 (32 bytes = 64 hex chars).
        assert_eq!(frame.checksum.len(), 64);
        assert!(frame
            .checksum
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn retained_pair_requires_matching_gpu_and_readback_milestones() {
        let mut manifest = PairManifest::fixture();
        manifest
            .retained
            .milestones
            .retain(|m| m != "readback-completed");
        assert_eq!(
            validate_review_pair(&manifest).unwrap_err(),
            "retained capture missing readback-completed",
        );
    }

    #[test]
    fn smooth_fallback_cannot_satisfy_retained_capture() {
        let mut manifest = PairManifest::fixture();
        manifest.retained.effective_renderer = "smooth".into();
        assert!(validate_review_pair(&manifest).is_err());
    }

    #[test]
    fn pair_manifest_round_trips_through_serde() {
        let manifest = PairManifest::fixture();
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: PairManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
        assert!(validate_review_pair(&parsed).is_ok());
    }

    #[test]
    fn fixture_manifest_passes_validation() {
        assert!(validate_review_pair(&PairManifest::fixture()).is_ok());
    }

    #[test]
    fn retained_capture_requires_gpu_completed_milestone() {
        let mut manifest = PairManifest::fixture();
        manifest
            .retained
            .milestones
            .retain(|m| m != "gpu-completed");
        assert_eq!(
            validate_review_pair(&manifest).unwrap_err(),
            "retained capture missing gpu-completed",
        );
    }

    #[test]
    fn smooth_section_must_report_the_smooth_renderer() {
        let mut manifest = PairManifest::fixture();
        manifest.smooth.effective_renderer = "retained".into();
        assert!(validate_review_pair(&manifest).is_err());
    }

    #[test]
    fn failed_status_manifest_is_rejected() {
        let mut manifest = PairManifest::fixture();
        manifest.status = "failed".into();
        assert!(validate_review_pair(&manifest).is_err());
    }

    #[test]
    fn integral_backing_scale_requires_physical_equals_logical_times_scale() {
        let mut manifest = PairManifest::fixture();
        manifest.physical.width += 1.0;
        assert!(validate_review_pair(&manifest).is_err());
    }

    #[test]
    fn sections_must_agree_on_frame_and_generation_ids() {
        let mut manifest = PairManifest::fixture();
        manifest.retained.frame_id = manifest.smooth.frame_id + 1;
        assert!(validate_review_pair(&manifest).is_err());
        let mut manifest = PairManifest::fixture();
        manifest.retained.resource_generation = manifest.smooth.resource_generation + 1;
        assert!(validate_review_pair(&manifest).is_err());
    }

    #[test]
    fn frame_milestones_map_to_stable_kebab_strings() {
        assert_eq!(
            milestone_kebab(FrameMilestone::GpuCompleted),
            "gpu-completed"
        );
        assert_eq!(
            milestone_kebab(FrameMilestone::ReadbackCompleted),
            "readback-completed"
        );
        assert_eq!(
            milestone_kebab(FrameMilestone::SurfacePresentCalled),
            "surface-present-called"
        );
    }

    #[test]
    fn frame_dispositions_map_to_stable_kebab_strings() {
        assert_eq!(disposition_kebab(FrameDisposition::Captured), "captured");
        assert_eq!(
            disposition_kebab(FrameDisposition::SurfacePresentCalled),
            "surface-present-called"
        );
        assert_eq!(
            disposition_kebab(FrameDisposition::Failed(
                RetainedFailureCategory::CapturePollTimeout
            )),
            "retained-capture-poll-timeout"
        );
    }

    #[test]
    fn fixture_manifest_records_no_hud_strings() {
        let json = serde_json::to_string(&PairManifest::fixture())
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in ["today", "pace", "%", "tokens", "daily_percent", "→"] {
            assert!(
                !json.contains(forbidden),
                "pair manifest leaked HUD-shaped token: {forbidden}"
            );
        }
    }
}
