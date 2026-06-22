use crate::game::evolution::Stage;
use crate::pet::generation::Species;

pub fn stage_label(species: Species, stage: Stage) -> &'static str {
    let labels = match species {
        Species::Fuzz => [
            "fluff",
            "fuzzling",
            "kit",
            "pup",
            "fuzz",
            "archfuzz",
            "mythic-fuzz",
        ],
        Species::Blob => [
            "droplet",
            "blip",
            "globule",
            "wee-blob",
            "blob",
            "mega-blob",
            "primordial",
        ],
        Species::Ghost => [
            "whisper",
            "wisp",
            "shade",
            "phantom-pup",
            "ghost",
            "wraith",
            "revenant",
        ],
        Species::Glitch => [
            "bit", "byte", "packet", "thread", "glitch", "daemon", "kernel",
        ],
        Species::Crystal => [
            "grain", "shard", "facet", "cluster", "crystal", "spire", "lodestar",
        ],
        Species::Mech => [
            "chip", "bolt", "rivet", "drone", "mech", "archmech", "titan",
        ],
    };
    labels[stage.index()]
}

/// Number of deterministic interior-texture variants a (species, stage) can
/// render. Per-pet variety is algorithmic (interior texture), not hand-drawn
/// silhouette pools, so this is the interior-texture-variant count (>= 1 for
/// every stage; 1 where texture is pinned). It is NOT a silhouette-pool size.
/// Phase 1: `apply_interior_texture` is an identity passthrough, so every
/// (species, stage) has exactly one variant. Phase 2 raises this where texture
/// adds variants.
pub fn morph_count(_species: Species, _stage: Stage) -> usize {
    1
}

/// One hand-drawn base silhouette per (species, stage). 42 total. Phase 1 wires
/// the existing art into this map; Phase 2 replaces the bodies with the new cast.
/// The S4/S5/S6 picks are three distinct existing adult shapes so growth still
/// reads as change without the retired `elder_morph_index` reshuffle.
pub(crate) fn stage_base_template(species: Species, stage: Stage) -> &'static Template {
    match species {
        Species::Fuzz => fuzz_base(stage),
        Species::Blob => blob_base(stage),
        Species::Ghost => ghost_base(stage),
        Species::Glitch => glitch_base(stage),
        Species::Crystal => crystal_base(stage),
        Species::Mech => mech_base(stage),
    }
}

// Validated Hearthfloof cast: ear-cones + mitten-feet + heart-locket, block-mass
// edges (▒▓█). Validated by the art pipeline against the Phase-1 invariants. The
// locket fills with age (◌ → ◆ → ◈◈◈); the ◆/◈ glyphs are EAW-Ambiguous and rely
// on the documented ambiguous=narrow rendering assumption (same as Crystal).
fn fuzz_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &FUZZ_S0,
        Stage::S1 => &FUZZ_S1,
        Stage::S2 => &FUZZ_S2,
        Stage::S3 => &FUZZ_S3,
        Stage::S4 => &FUZZ_S4,
        Stage::S5 => &FUZZ_S5,
        Stage::S6 => &FUZZ_S6,
    }
}

// Validated Deep-Light Medusa cast: translucent jelly bell with a glowing
// organ-core and a tendril curtain that lengthens with age. ▒▓ gravity-shaded
// core carries the flat-color figure-ground; ◉ lens eyes. Validated by the art
// pipeline against the Phase-1 invariants. S6 is fully baked (primordial).
fn blob_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &BLOB_S0,
        Stage::S1 => &BLOB_S1,
        Stage::S2 => &BLOB_S2,
        Stage::S3 => &BLOB_S3,
        Stage::S4 => &BLOB_S4,
        Stage::S5 => &BLOB_S5,
        Stage::S6 => &BLOB_S6,
    }
}

// Validated The Pall cast: a dense ▒▓█ shroud with a living face at rest and
// a sealed wavering \_/ hem; S6 fills all 8 rows. Validated by the art
// pipeline against the Phase-1 invariants.
fn ghost_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &GHOST_S0,
        Stage::S1 => &GHOST_S1,
        Stage::S2 => &GHOST_S2,
        Stage::S3 => &GHOST_S3,
        Stage::S4 => &GHOST_S4,
        Stage::S5 => &GHOST_S5,
        Stage::S6 => &GHOST_S6,
    }
}

// Validated Packet Daemon cast: a closed packet-frame silhouette (not open ▌▐
// walls), living lens eyes (◉ via the {eyes} slot), torn-data base. The mouth is
// wired to the {mouth} slot at S2-S4 (clean 1-cell mouth per spec Rendering #5);
// S5/S6 dense elder bodies bake their mouth decoration verbatim (no clean 1-cell
// mouth slot in the elder data-cube). Validated cell ramp [3,10,19,32,44,62,85].
fn glitch_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &GLITCH_S0,
        Stage::S1 => &GLITCH_S1,
        Stage::S2 => &GLITCH_S2,
        Stage::S3 => &GLITCH_S3,
        Stage::S4 => &GLITCH_S4,
        Stage::S5 => &GLITCH_S5,
        Stage::S6 => &GLITCH_S6,
    }
}

// Validated The Caged Lumen cast: faceted crystal silhouette with a caged core
// that fills with age. The lens glyph escalates ◇ (young) → ◆ (mid) → ◈
// (elder) baked directly into the body — Crystal has no {eyes}/{mouth} slots;
// the lens is the face. ◇◆◈ are EAW-Ambiguous and kept under the documented
// ambiguous=narrow assumption. Validated cell ramp [4,10,20,31,39,53,73].
// Transcribed verbatim from
// docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
fn crystal_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &CRYSTAL_S0,
        Stage::S1 => &CRYSTAL_S1,
        Stage::S2 => &CRYSTAL_S2,
        Stage::S3 => &CRYSTAL_S3,
        Stage::S4 => &CRYSTAL_S4,
        Stage::S5 => &CRYSTAL_S5,
        Stage::S6 => &CRYSTAL_S6,
    }
}

// Validated Mech · Bulwark cast: box-draw chassis silhouette (┌─┐ head,
// ┴─┴ shoulder bolts, ▟█▙ heavy armor) that grows unambiguously across
// every stage. {eyes}/{mouth} slots wired at S2–S6 so the per-mood expression
// vocabulary applies (Phase 3 face-coloring also relies on the slot).
// S5/S6 faces are slotted per Spec Rendering #5 — the silhouette is
// transcribed verbatim from the art-assets doc; only the 2 face rows per
// stage are wired to the spec-mandated slot instead of baked glyphs.
// Validated cell ramp [4,10,20,31,44,52,69].
// Transcribed from
// docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
fn mech_base(stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => &MECH_S0,
        Stage::S1 => &MECH_S1,
        Stage::S2 => &MECH_S2,
        Stage::S3 => &MECH_S3,
        Stage::S4 => &MECH_S4,
        Stage::S5 => &MECH_S5,
        Stage::S6 => &MECH_S6,
    }
}

/// Deterministic interior-texture variation. Swaps interior fill cells between
/// ▒ and ▓ per `seed`, preserving the closed outline, width-1, and occupancy
/// (so the stage cell band is never crossed). Pinned (no-op) on S0..S2.
pub(crate) fn apply_interior_texture(
    base: &Template,
    _species: Species,
    stage: Stage,
    seed: u64,
) -> [String; 8] {
    let pinned = matches!(stage, Stage::S0 | Stage::S1 | Stage::S2);
    let mut out: [String; 8] = Default::default();
    for (row, line) in base.iter().enumerate() {
        if pinned {
            out[row] = (*line).to_string();
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        // Find the outline bounds (first/last non-space cell) to protect them.
        let first = chars.iter().position(|c| !c.is_whitespace());
        let last = chars.iter().rposition(|c| !c.is_whitespace());
        let mut rebuilt = String::with_capacity(line.len());
        for (col, &ch) in chars.iter().enumerate() {
            let is_edge = Some(col) == first || Some(col) == last;
            let is_interior_fill = !is_edge && matches!(ch, '\u{2592}' | '\u{2593}'); // ▒ or ▓
            if is_interior_fill {
                // Hash (seed, row, col) -> pick ▒ or ▓. Never ░, never space.
                let h = mix_seed(seed, row as u64, col as u64);
                rebuilt.push(if h & 1 == 0 { '\u{2592}' } else { '\u{2593}' });
            } else {
                rebuilt.push(ch);
            }
        }
        out[row] = rebuilt;
    }
    out
}

/// Small deterministic mixer for interior-texture draws. Not crypto; just a
/// stable per-(seed,row,col) bit source independent of std hashing iteration.
fn mix_seed(seed: u64, row: u64, col: u64) -> u64 {
    let mut x = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(row.wrapping_mul(0x0000_0100_0000_01b3))
        .wrapping_add(col.wrapping_mul(0xff51_afd7_ed55_8ccd));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 29;
    x
}

/// Public render entry replacing `template_lines`. Returns owned Strings because
/// interior texture is computed, not 'static. `seed` is the interior-texture
/// draw (render.rs passes `pet.traits.seed_hue`).
pub(crate) fn stage_template_lines(species: Species, stage: Stage, seed: u64) -> [String; 8] {
    let base = stage_base_template(species, stage);
    apply_interior_texture(base, species, stage, seed)
}

// Each species template is 8 lines x 11 chars.
type Template = [&'static str; 8];

// ── Fuzz ──────────────────────────────────────────────────────────
// Hearthfloof: ear-cones + mitten-feet + heart-locket, block-mass edges (▒▓█).
// Validated by the art pipeline against the Phase-1 invariants. Cell ramp
// [4, 10, 19, 33, 50, 59, 69], strictly increasing, every value in-band.
// Transcribed verbatim from docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
const FUZZ_S0: Template = [
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▒▒     ",
    "    ▟▙     ",
    "           ",
];
const FUZZ_S1: Template = [
    "           ",
    "           ",
    "    ▟▙     ",
    "    ▒▒     ",
    "    ▓▓     ",
    "    ▒▒     ",
    "    ▘▝     ",
    "           ",
];
const FUZZ_S2: Template = [
    "           ",
    "           ",
    "    ▟▙     ",
    "   ▓▒▒▓    ",
    "  ▒{eyes}▒    ",
    "   ▒{mouth}▒     ",
    "   ▓▒▒▓    ",
    "    ▘▝     ",
];
const FUZZ_S3: Template = [
    "           ",
    "   ▟▙▟▙    ",
    "  ▓▒▒▒▒▓   ",
    "  ▓▒{eyes}▒▓  ",
    "  ▓▒ {mouth} ▒▓  ",
    "  ▓▒◌▒▒▓   ",
    "   ▙▒▒▟    ",
    "   ▘  ▝    ",
];
const FUZZ_S4: Template = [
    "   ▟▙ ▟▙   ",
    "  ▓▒▒▒▒▒▓  ",
    "  ▓▒{eyes}▒▒▓ ",
    "  ▓▒ {mouth} ▒▒▓ ",
    "  ▓▒▒◆▒▒▒▓ ",
    "  ▓▒▒▒▒▒▒▓ ",
    "  ▙▒▒▒▒▒▒▟ ",
    "   ▘    ▝  ",
];
const FUZZ_S5: Template = [
    "  ▟█▙ ▟█▙  ",
    "  ▓▓▒▒▒▒▓▓ ",
    "  ▓▒{eyes}▒▒▓ ",
    "  ▓▒ {mouth} ▒▒▓ ",
    "  ▓▒▒◆◆▒▒▓ ",
    "  ▓█▒▒▒▒█▓ ",
    "  ▓█▒▒▒▒█▓ ",
    "  ▙█▒▘▝▒█▟ ",
];
const FUZZ_S6: Template = [
    " ▟██▙▟██▙  ",
    " ▓██▒▒▒██▓ ",
    " ▓█▒{eyes}▒█▓ ",
    " ▓█▒ {mouth} ▒█▓ ",
    " ▓█▒◈◈◈▒█▓ ",
    " ▓██▒▒▒██▓ ",
    " ▓██▒▒▒██▓ ",
    " ▙██▒▘▝▒██▟",
];

// ── Blob ──────────────────────────────────────────────────────────
// Deep-Light Medusa: translucent bioluminescent jelly bell housing a glowing
// organ-core, with a tendril curtain that grows downward. ( ) rounded walls,
// gravity-shaded ▒▓ core (figure-ground), ◉ lens eyes. Validated by the art
// pipeline against the Phase-1 invariants. Cell ramp
// [3, 7, 15, 33, 38, 55, 78], strictly increasing, every value in-band.
// Transcribed from docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
// S0/S1 are single-eye buds (no slot); S2-S5 carry {eyes}/{mouth} slots so the
// mood-face vocabulary can substitute; S6 is fully baked (primordial form).
const BLOB_S0: Template = [
    "           ",
    "           ",
    "    \u{2584}\u{2584}     ",
    "    \u{2592}      ",
    "           ",
    "           ",
    "           ",
    "           ",
];
const BLOB_S1: Template = [
    "           ",
    "    \u{2584}\u{2584}     ",
    "   \u{259f}\u{2592}\u{2599}     ",
    "    \u{25c9}      ",
    "    |      ",
    "           ",
    "           ",
    "           ",
];
const BLOB_S2: Template = [
    "           ",
    "    \u{2584}\u{2584}\u{2584}    ",
    "   \u{259f}\u{2592}\u{2592}\u{2599}    ",
    "   {eyes}     ",
    "   \u{2592}{mouth}\u{2592}     ",
    "   |\u{254e}|     ",
    "           ",
    "           ",
];
const BLOB_S3: Template = [
    "    \u{2584}\u{2584}\u{2584}    ",
    "   \u{259f}\u{2592}\u{2592}\u{2599}    ",
    "  (\u{2592}{eyes}\u{2592})  ",
    "  (\u{2592}\u{2592}{mouth}\u{2592}\u{2592})  ",
    "   \u{2592}\u{2593}\u{2592}     ",
    "   \u{2592}\u{2593}\u{2592}     ",
    "   |\u{254e}|\u{250a}    ",
    "   ' ' '   ",
];
const BLOB_S4: Template = [
    "    \u{2584}\u{2584}\u{2584}    ",
    "   \u{259f}\u{2592}\u{2592}\u{2599}    ",
    "  (\u{2592}{eyes}\u{2592})  ",
    "  (\u{2592}\u{2592}{mouth}\u{2592}\u{2592})  ",
    "  (\u{2591}\u{2593}\u{25c6}\u{2593}\u{2591})  ",
    "   \u{2592}\u{2593}\u{2593}\u{2592}    ",
    "   |\u{254e}|\u{250a}    ",
    "   ' ' '   ",
];
const BLOB_S5: Template = [
    "   \u{2584}\u{2584}\u{2584}\u{2584}    ",
    "  \u{259f}\u{2592}\u{2592}\u{2592}\u{2592}\u{2599}   ",
    " (\u{2592}\u{2592}{eyes}\u{2592}\u{2592}) ",
    " (\u{2592}\u{2592}\u{2592}{mouth}\u{2592}\u{2592}\u{2592}) ",
    " (\u{2591}\u{2593}\u{25c6}\u{25c9}\u{25c6}\u{2593}\u{2591}) ",
    " (\u{2591}\u{2592}\u{2593}\u{2593}\u{2593}\u{2592}\u{2591}) ",
    "  |\u{250a}|\u{254e}|\u{250a}   ",
    "  ' ' ' '  ",
];
const BLOB_S6: Template = [
    " \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584} ",
    "\u{259f}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2599}",
    "\u{2590}\u{2592}\u{2593}\u{2588}\u{2588}\u{2588}\u{2593}\u{2592}\u{2592}\u{2592}\u{258c}",
    "\u{2590}\u{2592}{eyes}\u{2592}\u{2593}\u{2592}\u{2591}\u{258c} ",
    "(\u{2592}\u{2592}{mouth}\u{2592}\u{2592}\u{2593}\u{2592}\u{2591}) ",
    "(\u{25c6}\u{2593}\u{25c9}\u{25c6}\u{25c9}\u{2593}\u{25c6}\u{2592}) ",
    "\u{259d}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2598} ",
    " |\u{250a}|\u{254e}|\u{250a}|\u{254e}  ",
];

// ── Ghost ─────────────────────────────────────────────────────────
// The Pall: a dense ▒▓█ shroud with a living face at rest; sealed wavering
// \_/ hem, S6 fills all 8 rows. Validated by the art pipeline against the
// Phase-1 invariants. Cell ramp [3, 9, 18, 31, 44, 63, 79], strictly
// increasing, every value in-band. Transcribed verbatim from
// docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
// S0/S1 are pre-face buds; S2-S6 carry {eyes}/{mouth} slots and S4-S6 carry
// {pattern} so the mood-face vocabulary can substitute.
const GHOST_S0: Template = [
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▒▒▒    ",
    "           ",
];
const GHOST_S1: Template = [
    "           ",
    "           ",
    "           ",
    "           ",
    "    ▄▄▄    ",
    "    ▒▒▒    ",
    "    \\_/    ",
    "           ",
];
const GHOST_S2: Template = [
    "           ",
    "           ",
    "    ▄▄▄    ",
    "   ▒{eyes}▒   ",
    "   ▒░{mouth}░▒   ",
    "    ▒▒▒    ",
    "    \\_/    ",
    "           ",
];
const GHOST_S3: Template = [
    "    ╭─╮    ",
    "   ╭╯ ╰╮   ",
    "   ▒▒▒▒▒   ",
    "   ▒{eyes}▒   ",
    "   ▒░{mouth}░▒   ",
    "   ▒▒▒▒▒   ",
    "   \\_/\\_   ",
    "           ",
];
const GHOST_S4: Template = [
    "   ╭───╮   ",
    "  ╭╯   ╰╮  ",
    "  ░▒▒▒▒▒░  ",
    "  ░▒{eyes}▒░  ",
    "  ░▒░{mouth}░▒░  ",
    "  ░▒{pattern}▒░  ",
    "   ▒▒▒▒▒   ",
    "   \\_/\\_   ",
];
const GHOST_S5: Template = [
    "  ╭╯───╰╮  ",
    "  ▒▓▓▓▓▓▒  ",
    " ░▒▓{eyes}▓▒░ ",
    " ░▒▓░{mouth}░▓▒░ ",
    " ░▒▓{pattern}▓▒░ ",
    " ░▒▓▓▓▓▓▒░ ",
    "  ▒▓▓▓▓▓▒  ",
    " \\_/\\_/\\_/ ",
];
const GHOST_S6: Template = [
    " ░▒▓▓▓▓▓▒░ ",
    "▒▓███████▓▒",
    "▓███{eyes}███▓",
    "▓███░{mouth}░███▓",
    "▒▓██{pattern}██▓▒",
    "▒▓███████▓▒",
    " ▓███████▓ ",
    " ░▒▓█▓█▓▒░ ",
];

// ── Glitch · Packet Daemon (validated) ── S0-S6 base silhouettes. Transcribed
// verbatim from the art-assets doc with the mouth wired to {mouth} at S2-S4
// (per spec Rendering #5; the {eyes} slot is already in the doc). S5/S6 keep
// their baked mouth decoration (no clean 1-cell mouth in the elder data-cube).
const GLITCH_S0: Template = [
    "           ",
    "           ",
    "           ",
    "     ◉     ",
    "    ▟▙     ",
    "           ",
    "           ",
    "           ",
];
const GLITCH_S1: Template = [
    "           ",
    "           ",
    "    ▛▀▜    ",
    "    ▌◉▐    ",
    "    ▙▄▟    ",
    "     ▚     ",
    "           ",
    "           ",
];
const GLITCH_S2: Template = [
    "           ",
    "   ▛▀▀▀▜   ",
    "   ▌{eyes}▐   ",
    "   ▌ {mouth} ▐   ",
    "   ▙▄▄▄▟   ",
    "    ▚ ▞    ",
    "           ",
    "           ",
];
const GLITCH_S3: Template = [
    "           ",
    "  ▛▀▀▀▀▀▜  ",
    "  ▌ {eyes} ▐  ",
    "  ▌  {mouth}  ▐  ",
    "  ▌ ░▄░ ▐  ",
    "  ▙▄▄▄▄▄▟  ",
    "   ▚▞ ▚▞   ",
    "    ▘  ▝   ",
];
const GLITCH_S4: Template = [
    " ▛▀▀▀▀▀▀▀▜ ",
    " ▌  {eyes}  ▐ ",
    " ▌   {mouth}   ▐ ",
    " ▌  ▒▓▒  ▐ ",
    " ▌  ░▒░  ▐ ",
    " ▙▄▄▄▄▄▄▄▟ ",
    "  ▚▞▙ ▟▚▞  ",
    "   ▘ ▝ ▘   ",
];
const GLITCH_S5: Template = [
    " ▛▀▀▀▀▀▀▀▜ ",
    " ▌▒ {eyes} ▒▐ ",
    " ▌░▄▄▄▄▄░▐ ",
    " ▌▒░ █ ░▒▐ ",
    " ▌▓░▒▒▒░▓▐ ",
    " ▌░▒ ▒ ▒░▐ ",
    " ▙▄▄▄▄▄▄▄▟ ",
    "  ▝▟▙ ▟▙▘  ",
];
const GLITCH_S6: Template = [
    "▛▀▀▀▀▀▀▀▀▀▜",
    "▌▓▒ {eyes} ▒▓▐",
    "▌▒░█▀▀▀█░▒▐",
    "▌▓▒░▓▓▓░▒▓▐",
    "▌█▒░▒▒▒░▒█▐",
    "▌▓▒░▒▒▒░▒▓▐",
    "▙▟▙▟▙▟▙▟▙▟▟",
    "▝▟▙▟▙▟▙▟▙▟▘",
];

// ── Crystal ───────────────────────────────────────────────────────
// Validated The Caged Lumen silhouettes S0–S6. Transcribed verbatim from
// docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
// Cell ramp [4,10,20,31,39,53,73]. No {eyes}/{mouth} slots — the ◇/◆/◈ lens
// is baked into each silhouette per the eye-fill design.
const CRYSTAL_S0: Template = [
    "           ",
    "           ",
    "           ",
    "     /\\    ",
    "     \\/    ",
    "           ",
    "           ",
    "           ",
];
const CRYSTAL_S1: Template = [
    "           ",
    "           ",
    "     /\\    ",
    "    /◇\\    ",
    "    \\▒/    ",
    "     \\/    ",
    "           ",
    "           ",
];
const CRYSTAL_S2: Template = [
    "           ",
    "    /\\     ",
    "   /◇◇\\    ",
    "  /▒▓▒\\    ",
    "  \\▒▒/     ",
    "   \\▓/     ",
    "    \\/     ",
    "           ",
];
const CRYSTAL_S3: Template = [
    "    /\\     ",
    "   /◆◆\\    ",
    "  /▒▓▒\\    ",
    " /▒▓▓▒\\    ",
    " \\▒▓▒/     ",
    "  \\▓▓/     ",
    "   \\▓/     ",
    "    \\/     ",
];
const CRYSTAL_S4: Template = [
    "    /\\     ",
    "   /◆◆\\    ",
    "  /▒◆◆▒\\   ",
    " /▒▓▿▓▒\\   ",
    " \\▒▓█▓▒/   ",
    "  \\▒█▒/    ",
    "  \\▓█▓/    ",
    "   \\▼/     ",
];
const CRYSTAL_S5: Template = [
    "  /\\/\\/\\   ",
    " /◈▒◈▒◈\\   ",
    "/▒▓█▓█▓▒\\  ",
    "\\▒▓███▓▒/  ",
    " \\▒▓█▓▒/   ",
    " \\▒▓█▓▒/   ",
    "  \\▓█▓/    ",
    "   \\▼/     ",
];
const CRYSTAL_S6: Template = [
    "/\\/\\/\\/\\/\\ ",
    "▒██◈██◈██▒ ",
    "▒█▓██▓██▓▒ ",
    "▒█▓█▿█▓█▓▒ ",
    "▒█▓█████▓▒ ",
    "▒▓█▓███▓█▒ ",
    " \\▒▓█▓█▒/  ",
    "  \\▼▼▼/    ",
];

// ── Mech ──────────────────────────────────────────────────────────
// Validated Mech · Bulwark silhouettes S0–S6. Box-draw war-frame chassis:
// head/torso/legs bolt onto the chassis as it ages (legibility benchmark).
// {eyes}/{mouth} slots wired at S2–S6. S5/S6 are slotted per Spec Rendering #5
// (binding reconciliation rule): the silhouette is transcribed verbatim from the
// art-assets doc; only the 2 face rows per stage replace baked ◉ ◉/═ glyphs
// with the spec-mandated {eyes}/{mouth} slots so mood expressions apply.
// Cell ramp [4,10,20,31,44,52,69], strictly increasing, every value in-band.
// Transcribed from docs/superpowers/plans/2026-06-21-glorp-overhaul-phase2-art-assets.md.
const MECH_S0: Template = [
    "           ",
    "           ",
    "           ",
    "           ",
    "     ▄     ",
    "    ▐◉▌    ",
    "           ",
    "           ",
];
const MECH_S1: Template = [
    "           ",
    "           ",
    "           ",
    "    ▗▄▖    ",
    "    ▌◉◉▐   ",
    "    ▝▀▘    ",
    "           ",
    "           ",
];
const MECH_S2: Template = [
    "           ",
    "     ╷     ",
    "    ┌───┐  ",
    "    │{eyes}│  ",
    "    │ {mouth} │  ",
    "    └┬─┬┘  ",
    "     ╨ ╨   ",
    "           ",
];
const MECH_S3: Template = [
    "    ╷ ╷    ",
    "   ┌───┐   ",
    "   │{eyes}│   ",
    "   │ {mouth} │   ",
    "   ├───┤   ",
    "   │▒▓▒│   ",
    "   └┬─┬┘   ",
    "    ╨ ╨    ",
];
const MECH_S4: Template = [
    "    ╷╷╷    ",
    "   ┌───┐   ",
    "   │{eyes}│   ",
    "  ┌┴─{mouth}─┴┐  ",
    "  ║▓███▓║  ",
    "  ║▓▒▒▒▓║  ",
    "  ╜└┬─┬┘╙  ",
    "   ██ ██   ",
];
const MECH_S5: Template = [
    "   ╲╷╷╷╱   ",
    "   ┌───┐   ",
    "   │{eyes}│   ",
    "  ┌┴─{mouth}─┴┐  ",
    " ▟█▌███▐█▙ ",
    " ▝█▌▒◈▒▐█▘ ",
    "  ║▌▓─▓▐║  ",
    "  ▟█▙ ▟█▙  ",
];
const MECH_S6: Template = [
    " ██┌───┐██ ",
    " ██│{eyes}│██ ",
    " ██│ {mouth} │██ ",
    " ██▙▓◆▓▟██ ",
    " █████████ ",
    " ███▒◈▒███ ",
    " ██▙┬─┬▟██ ",
    " ▟███████▙ ",
];

#[cfg(test)]
mod tests {
    use super::*;

    // Slot widths must match `visible_traits` in `src/pet/generation.rs`:
    // eyes=3, mouth=1, pattern=3, accent=1.
    fn substitute_slots(line: &str) -> String {
        line.replace("{eyes}", "o o")
            .replace("{mouth}", "w")
            .replace("{pattern}", "...")
            .replace("{accent}", "*")
    }

    const ALL_STAGES: [Stage; 7] = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];

    #[test]
    fn every_template_line_is_eleven_cells_wide() {
        for species in Species::all() {
            for stage in ALL_STAGES {
                let lines = stage_base_template(species, stage);
                for (row, line) in lines.iter().enumerate() {
                    let rendered = substitute_slots(line);
                    let width = rendered.chars().count();
                    assert_eq!(
                        width, 11,
                        "template width != 11 for species={species:?} stage={stage:?} row={row}: \
                         {rendered:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_template_line_is_eleven_display_columns() {
        use unicode_width::UnicodeWidthStr;
        // Terminal columns under unicode-width's default (ambiguous=narrow).
        for species in Species::all() {
            for stage in ALL_STAGES {
                let lines = stage_base_template(species, stage);
                for (row, line) in lines.iter().enumerate() {
                    let rendered = substitute_slots(line);
                    let columns = UnicodeWidthStr::width(rendered.as_str());
                    assert_eq!(
                        columns, 11,
                        "display width != 11 for species={species:?} stage={stage:?} row={row}: \
                         {rendered:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_template_is_eight_lines() {
        for species in Species::all() {
            for stage in ALL_STAGES {
                let lines = stage_base_template(species, stage);
                assert_eq!(
                    lines.len(),
                    8,
                    "template height != 8 for species={species:?} stage={stage:?}"
                );
            }
        }
    }

    #[test]
    fn elder_stages_are_distinct_base_templates() {
        // Replaces elder_morph_skips_singleton_for_carved_species / _for_glitch.
        // The retired `elder_morph_index` ensured S5/S6 were not the S4 form; the
        // per-stage base map encodes that directly by mapping S4/S5/S6 to three
        // different existing shapes. Strict occupied-cell growth (S4<S5<S6) is the
        // Phase 2 band gate, not a Phase 1 property of the placeholder art.
        for species in Species::all() {
            let s4 = stage_base_template(species, Stage::S4);
            let s5 = stage_base_template(species, Stage::S5);
            let s6 = stage_base_template(species, Stage::S6);
            assert_ne!(
                s4, s5,
                "{species:?} S4 and S5 must be different base templates"
            );
            assert_ne!(
                s5, s6,
                "{species:?} S5 and S6 must be different base templates"
            );
            assert_ne!(
                s4, s6,
                "{species:?} S4 and S6 must be different base templates"
            );
        }
    }

    #[test]
    fn stage_base_template_returns_a_valid_template_for_every_species_stage() {
        for species in Species::all() {
            for stage in ALL_STAGES {
                let tpl = stage_base_template(species, stage);
                assert_eq!(tpl.len(), 8, "{species:?} {stage:?} must be 8 lines");
                for (row, line) in tpl.iter().enumerate() {
                    let rendered = substitute_slots(line);
                    assert_eq!(
                        rendered.chars().count(),
                        11,
                        "{species:?} {stage:?} row {row} must be 11 chars: {rendered:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn morph_count_is_at_least_one_for_every_stage() {
        for species in Species::all() {
            for stage in ALL_STAGES {
                assert!(
                    morph_count(species, stage) >= 1,
                    "{species:?} {stage:?} must have >= 1 interior-texture variant"
                );
            }
        }
    }

    #[test]
    fn stage_template_lines_has_eight_lines_for_every_species_stage() {
        // stage_template_lines feeds render.rs; it must always return 8 lines.
        for species in Species::all() {
            for stage in ALL_STAGES {
                let lines = stage_template_lines(species, stage, 42);
                assert_eq!(lines.len(), 8, "{species:?} {stage:?} must be 8 lines");
            }
        }
    }

    #[test]
    fn stage_template_lines_delegates_to_apply_interior_texture() {
        // stage_template_lines is the public render boundary; it must call through to
        // apply_interior_texture. Guards against the delegation being accidentally
        // severed (line-count alone would not catch that).
        for species in Species::all() {
            for stage in ALL_STAGES {
                let base = stage_base_template(species, stage);
                for seed in [0u64, 42, 9999, 123_456_789] {
                    assert_eq!(
                        stage_template_lines(species, stage, seed),
                        apply_interior_texture(base, species, stage, seed),
                        "{species:?} {stage:?} seed={seed}: stage_template_lines must delegate to apply_interior_texture"
                    );
                }
            }
        }
    }

    // Inclusive [lo, hi] occupied-cell band per stage (the audit target for the
    // Phase 2 art). Disjoint and strictly increasing.
    const STAGE_CELL_BANDS: [(usize, usize); 7] = [
        (1, 4),   // S0
        (5, 10),  // S1
        (11, 20), // S2
        (21, 34), // S3
        (35, 50), // S4
        (51, 66), // S5
        (67, 88), // S6
    ];

    // Rendered occupied (non-space) cell count of the 8 art rows at the fixed
    // reference state: mood = Content, resting (non-blink) expression, no work
    // accent, fixed tick, each {slot} replaced by its canonical width-correct
    // filler ({eyes}->"o o", {mouth}->"w", {pattern}->"...", {accent}->"*").
    // Excludes the particle gutter and any frame substitution. Built from
    // stage_template_lines so it tracks the Phase 1 (identity) and Phase 2 (real)
    // texture path.
    #[cfg(test)]
    fn rendered_occupied_cells(species: Species, stage: Stage) -> usize {
        stage_template_lines(species, stage, REFERENCE_SEED)
            .iter()
            .map(|line| {
                substitute_slots(line)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .count()
            })
            .sum()
    }

    // A fixed reference seed for the tick-independent measurement. The slot fill is
    // canonical (substitute_slots), so this is only the interior-texture draw.
    #[cfg(test)]
    const REFERENCE_SEED: u64 = 0;

    // Band membership + S0->S6 monotonicity (S4 < S5 < S6) over the occupied-cell
    // count. NOTE: in Phase 1 this is NOT run over the real templates (they predate
    // the band redesign); Phase 2 calls it as its growth-acceptance gate. It is
    // exercised here only via the synthetic self-test below so its logic is proven.
    #[cfg(test)]
    fn assert_in_stage_band_value(stage: Stage, occupied: usize) {
        let (lo, hi) = STAGE_CELL_BANDS[stage.index()];
        assert!(
            occupied >= lo && occupied <= hi,
            "occupied cells {occupied} for {stage:?} outside band [{lo}, {hi}]"
        );
    }

    // Called by the Phase 2 growth gate; see plan Task 4.
    #[allow(dead_code)]
    #[cfg(test)]
    fn assert_in_stage_band(species: Species, stage: Stage) {
        assert_in_stage_band_value(stage, rendered_occupied_cells(species, stage));
    }

    #[test]
    fn stage_cell_bands_are_disjoint_and_strictly_increasing() {
        for w in STAGE_CELL_BANDS.windows(2) {
            let (lo, hi) = w[0];
            let (next_lo, next_hi) = w[1];
            assert!(lo <= hi, "band [{lo}, {hi}] is inverted");
            assert!(
                hi < next_lo,
                "bands must be disjoint and increasing: [{lo},{hi}] then [{next_lo},{next_hi}]"
            );
        }
        // S4 < S5 < S6 lower bounds (the explicit monotonicity callout).
        assert!(STAGE_CELL_BANDS[4].0 < STAGE_CELL_BANDS[5].0);
        assert!(STAGE_CELL_BANDS[5].0 < STAGE_CELL_BANDS[6].0);
    }

    #[test]
    fn assert_in_stage_band_value_accepts_in_band_rejects_out_of_band() {
        // Proves the band check logic without depending on placeholder art.
        assert_in_stage_band_value(Stage::S0, 1);
        assert_in_stage_band_value(Stage::S0, 4);
        assert_in_stage_band_value(Stage::S6, 67);
        assert_in_stage_band_value(Stage::S6, 88);
        let rejected = std::panic::catch_unwind(|| assert_in_stage_band_value(Stage::S0, 5));
        assert!(
            rejected.is_err(),
            "5 cells must be rejected from the S0 band"
        );
        let rejected_high = std::panic::catch_unwind(|| assert_in_stage_band_value(Stage::S6, 89));
        assert!(
            rejected_high.is_err(),
            "89 cells must be rejected from the S6 band"
        );
    }

    // Ambiguous=WIDE width check. Per the Crystal eye-fill decision this is a
    // NON-BLOCKING lint: it WARNS (eprintln!) on East-Asian-Width Ambiguous glyphs
    // and returns them, but never asserts — failing the build would contradict
    // keeping the ◇/◆/◈ eye-fill. The blocking width invariant
    // (every_template_line_is_eleven_display_columns) stays under the default
    // narrow assumption.
    #[cfg(test)]
    fn ambiguous_wide_width_warnings(species: Species, stage: Stage) -> Vec<char> {
        use unicode_width::UnicodeWidthChar;
        let mut offenders = Vec::new();
        for line in stage_template_lines(species, stage, REFERENCE_SEED).iter() {
            for ch in substitute_slots(line).chars() {
                // width_cjk() applies the ambiguous=wide rule. A glyph whose narrow
                // width is 1 but whose cjk width is 2 is the Ambiguous case.
                let narrow = UnicodeWidthChar::width(ch).unwrap_or(0);
                let wide = UnicodeWidthChar::width_cjk(ch).unwrap_or(0);
                if narrow == 1 && wide == 2 {
                    offenders.push(ch);
                }
            }
        }
        offenders
    }

    #[test]
    fn ambiguous_width_lint_warns_but_does_not_fail() {
        // Surfaces ambiguous-width glyphs for human review without failing CI.
        // The eprintln! below is an intentional captured lint signal: it only
        // appears under `--nocapture`, so default `cargo test` output stays clean.
        let mut total = 0usize;
        for species in Species::all() {
            for stage in ALL_STAGES {
                let offenders = ambiguous_wide_width_warnings(species, stage);
                if !offenders.is_empty() {
                    eprintln!("ambiguous-width glyphs in {species:?} {stage:?}: {offenders:?}");
                    total += offenders.len();
                }
            }
        }
        // The lint is informational. Assert only that the helper ran over every
        // species/stage (the count is allowed to be zero or positive).
        let _ = total;
    }

    // Structural: S6 fills all 8 art rows from the creature (no sparkle row
    // substitution). Phase 1 satisfies this because S6 now maps to a full adult
    // template instead of the retired SAGE_TOP/SAGE_BOT framing. The "S6 fills
    // all 8 rows" density requirement (67-88 occupied cells) is a Phase 2 band
    // gate, NOT Phase 1 — the existing adult templates may leave a legitimately
    // blank trailing row, which this check accepts; it rejects only sparkle-
    // substituted rows.
    #[cfg(test)]
    fn assert_s6_fills_art_rows_no_sparkle(species: Species) {
        let lines = stage_base_template(species, Stage::S6);
        // No row is a literal sparkle frame (the old SAGE rows were sparkle-only).
        for (row, line) in lines.iter().enumerate() {
            let only_sparkle_and_space = line
                .chars()
                .all(|c| c == ' ' || matches!(c, '\u{2726}' | '\u{2727}' | '*' | '.'));
            assert!(
                !only_sparkle_and_space || substitute_slots(line).trim().is_empty(),
                "{species:?} S6 row {row} looks like a sparkle frame, not creature art: {line:?}"
            );
        }
    }

    #[test]
    fn s6_fills_art_rows_for_every_species() {
        for species in Species::all() {
            assert_s6_fills_art_rows_no_sparkle(species);
        }
    }

    #[test]
    fn fuzz_base_art_passes_phase1_invariants() {
        let species = Species::Fuzz;
        for stage in ALL_STAGES {
            // width-1 / 11-col on the rendered base (canonical fillers).
            // Reuses the Phase-1 helper that renders with mood=Content, resting eyes.
            assert_in_stage_band(species, stage); // band membership + monotonicity
        }
        assert_s6_fills_art_rows_no_sparkle(species);
        // Fuzz does NOT assert `ambiguous_wide_width_warnings(Fuzz,_).is_empty()`:
        // the ▒▓█▟▙▘▝ body vocabulary (and the S4-S6 ◆/◈ locket) are
        // EAW-Ambiguous by design, relied on under the documented
        // ambiguous=narrow assumption. The wide-mode check is a non-blocking
        // lint per the README's binding Crystal decision; the band + s6
        // structural checks above are the real transcription-error gate,
        // matching the other 5 species tests.
    }

    #[test]
    fn blob_base_art_passes_phase1_invariants() {
        let species = Species::Blob;
        for stage in ALL_STAGES {
            assert_in_stage_band(species, stage);
        }
        assert_s6_fills_art_rows_no_sparkle(species);

        // Flat-color figure-ground: at every stage that has a body (S2+), the base
        // must contain at least one ▒ or ▓ core cell so it reads as solid with
        // color off — a ░-only body in a ░ dot field is the failure this guards.
        for stage in [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            let base = stage_base_template(species, stage);
            let has_dense_core = base
                .iter()
                .any(|line| line.contains('\u{2592}') || line.contains('\u{2593}'));
            assert!(
                has_dense_core,
                "Blob {stage:?} base must carry a ▒/▓ core for flat-color figure-ground"
            );
        }
    }

    #[test]
    fn ghost_base_art_passes_phase1_invariants() {
        let species = Species::Ghost;
        for stage in ALL_STAGES {
            assert_in_stage_band(species, stage);
        }
        assert_s6_fills_art_rows_no_sparkle(species);
        // Ghost is soft but must still read solid with color off: dense core at S2+.
        for stage in [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            let base = stage_base_template(species, stage);
            let has_dense_core = base
                .iter()
                .any(|line| line.contains('\u{2592}') || line.contains('\u{2593}'));
            assert!(has_dense_core, "Ghost {stage:?} base needs a ▒/▓ core");
        }
    }

    #[test]
    fn glitch_base_art_passes_phase1_invariants() {
        let species = Species::Glitch;
        for stage in ALL_STAGES {
            assert_in_stage_band(species, stage);
        }
        assert_s6_fills_art_rows_no_sparkle(species);
    }

    #[test]
    fn crystal_base_art_passes_phase1_invariants() {
        let species = Species::Crystal;
        for stage in ALL_STAGES {
            assert_in_stage_band(species, stage);
        }
        assert_s6_fills_art_rows_no_sparkle(species);
        // Crystal deliberately keeps ambiguous-wide ◆◈; the lint WARNS (non-blocking)
        // and we assert it surfaces them rather than silently dropping the signature.
        let mut any_warning = false;
        for stage in [Stage::S4, Stage::S5, Stage::S6] {
            if !ambiguous_wide_width_warnings(species, stage).is_empty() {
                any_warning = true;
            }
        }
        assert!(
            any_warning,
            "Crystal elder stages keep the ◆/◈ signature; the ambiguous lint should warn"
        );
    }

    #[test]
    fn crystal_eye_fill_ages_with_stage() {
        // The caged-core glyph fills with age: a young facet (◇) at S2, a filled
        // diamond (◆) by S4, the multi-facet (◈) by S6. Structural, not subjective.
        let young = stage_base_template(Species::Crystal, Stage::S2)
            .iter()
            .copied()
            .collect::<String>();
        let elder = stage_base_template(Species::Crystal, Stage::S6)
            .iter()
            .copied()
            .collect::<String>();
        assert!(
            elder.contains('\u{25c8}') || elder.contains('\u{25c6}'),
            "Crystal S6 must show a filled facet core (◆/◈), got:\n{elder}"
        );
        // young may use the hollow ◇ — assert it is NOT already the elder ◈.
        assert!(
            !young.contains('\u{25c8}'),
            "Crystal S2 must not pre-show the elder ◈ facet, got:\n{young}"
        );
    }

    #[test]
    fn mech_base_art_passes_phase1_invariants() {
        let species = Species::Mech;
        for stage in ALL_STAGES {
            assert_in_stage_band(species, stage);
        }
        assert_s6_fills_art_rows_no_sparkle(species);
        // Mech keeps its own chassis rows at S6 (gutter_content_for == None per the
        // Phase-1/CONTRACT Mech-S6 decision); assert S6 fills all 8 rows itself.
        let s6 = stage_base_template(species, Stage::S6);
        let nonblank_rows = s6
            .iter()
            .filter(|line| line.chars().any(|c| !c.is_whitespace()))
            .count();
        assert_eq!(nonblank_rows, 8, "Mech S6 must fill all 8 art rows itself");
    }

    #[test]
    fn interior_texture_varies_per_seed() {
        // On adult stages, two different seeds produce different interior texture.
        for species in Species::all() {
            let base = stage_base_template(species, Stage::S5);
            let a = apply_interior_texture(base, species, Stage::S5, 11);
            let b = apply_interior_texture(base, species, Stage::S5, 91037);
            // Some species/stage may have too few interior fill cells to vary; require
            // that at least one species varies (the mechanism works), and that no call
            // panics. Stronger per-species variation is exercised in the preview lab.
            if a != b {
                return;
            }
        }
        panic!("interior texture did not vary for any species at S5");
    }

    #[test]
    fn interior_texture_preserves_band_and_width() {
        use unicode_width::UnicodeWidthStr;
        for species in Species::all() {
            for stage in ALL_STAGES {
                let base = stage_base_template(species, stage);
                let base_occupied: usize = base
                    .iter()
                    .map(|l| {
                        substitute_slots(l)
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .count()
                    })
                    .sum();
                for seed in [0u64, 7, 42, 9999, 123_456_789] {
                    let textured = apply_interior_texture(base, species, stage, seed);
                    assert_eq!(
                        textured.len(),
                        8,
                        "{species:?} {stage:?}: must stay 8 lines"
                    );
                    let mut occupied = 0;
                    for (row, line) in textured.iter().enumerate() {
                        let rendered = substitute_slots(line);
                        assert_eq!(
                            UnicodeWidthStr::width(rendered.as_str()),
                            11,
                            "{species:?} {stage:?} seed={seed} row={row}: rendered width must stay 11, got {:?}",
                            rendered
                        );
                        occupied += rendered.chars().filter(|c| !c.is_whitespace()).count();
                    }
                    assert_eq!(
                        occupied, base_occupied,
                        "{species:?} {stage:?} seed={seed}: rendered occupancy must not change (band-safe)"
                    );
                }
            }
        }
    }

    #[test]
    fn interior_texture_is_pinned_on_small_stages() {
        // S0..S2 must be byte-identical to the base across seeds (constant-occupancy,
        // the narrow bands cannot tolerate any swing).
        for species in Species::all() {
            for stage in [Stage::S0, Stage::S1, Stage::S2] {
                let base = stage_base_template(species, stage);
                let base_lines: Vec<String> = base.iter().map(|l| l.to_string()).collect();
                for seed in [0u64, 5, 500, 5_000_000] {
                    let textured = apply_interior_texture(base, species, stage, seed);
                    assert_eq!(
                        textured.to_vec(),
                        base_lines,
                        "{species:?} {stage:?} seed={seed}: small stages must be pinned"
                    );
                }
            }
        }
    }

    #[test]
    fn glitch_resting_face_is_alive() {
        use crate::game::metabolism::Mood;
        use crate::pet::generation::generate_pet;
        use crate::pet::render::{render_pet, AnimationFrame};
        let pet = generate_pet("glitch-face-seed").with_species(Species::Glitch);
        let art = render_pet(
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                ..AnimationFrame::default()
            },
        )
        .lines
        .join("\n");
        assert!(
            !art.contains("x x"),
            "Glitch resting face must be alive, never corpse eyes, got:\n{art}"
        );
        assert!(
            art.chars().filter(|c| !c.is_whitespace()).count() > 30,
            "Glitch S4 must render a dense closed packet, got:\n{art}"
        );
    }
}
