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
        Species::Crystal | Species::Mech => legacy_base_template(species, stage),
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

// Phase-1 stage dispatch for species whose validated silhouettes are not yet
// wired (Blob/Ghost/Glitch/Crystal/Mech). Delegates to the per-species
// tiny/pup/adult template pools. Task 8 removes this once all species are wired.
fn legacy_base_template(species: Species, stage: Stage) -> &'static Template {
    match stage {
        Stage::S0 => tiny_template(species, 0),
        Stage::S1 => tiny_template(species, 1),
        Stage::S2 => tiny_template(species, 2),
        Stage::S3 => &pup_templates(species)[0],
        Stage::S4 => &adult_templates(species)[0],
        Stage::S5 => &adult_templates(species)[1],
        Stage::S6 => {
            let adults = adult_templates(species);
            &adults[adults.len() - 1]
        }
    }
}

/// Deterministic per-seed interior-texture variation applied on top of a base
/// silhouette. Phase 1: identity passthrough (returns the base verbatim) — the
/// hook exists so render.rs and the invariant tests can target the final API
/// now; Phase 2 fills in the texture math. MUST preserve the closed outline,
/// width-1, and the stage cell band. On S0-S2 it is constrained to
/// constant-occupancy glyphs (band-safety).
pub(crate) fn apply_interior_texture(
    base: &Template,
    _species: Species,
    _stage: Stage,
    _seed: u64,
) -> [String; 8] {
    std::array::from_fn(|i| base[i].to_string())
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

fn pup_templates(species: Species) -> &'static [Template] {
    match species {
        Species::Fuzz => FUZZ_PUP,
        Species::Blob => BLOB_PUP,
        Species::Ghost => GHOST_PUP,
        Species::Glitch => GLITCH_PUP,
        Species::Crystal => CRYSTAL_PUP,
        Species::Mech => MECH_PUP,
    }
}

fn adult_templates(species: Species) -> &'static [Template] {
    match species {
        Species::Fuzz => FUZZ_ADULT,
        Species::Blob => BLOB_ADULT,
        Species::Ghost => GHOST_ADULT,
        Species::Glitch => GLITCH_ADULT,
        Species::Crystal => CRYSTAL_ADULT,
        Species::Mech => MECH_ADULT,
    }
}

fn tiny_template(species: Species, index: usize) -> &'static Template {
    match species {
        Species::Fuzz => &FUZZ_TINY[index],
        Species::Blob => &BLOB_TINY[index],
        Species::Ghost => &GHOST_TINY[index],
        Species::Glitch => &GLITCH_TINY[index],
        Species::Crystal => &CRYSTAL_TINY[index],
        Species::Mech => &MECH_TINY[index],
    }
}

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

// Chunky filled cat-creature: /\_/\ ears, two-tone ░▒ fur shading, and a tail
// present from the pup (S3) onward. Morph 0 wears a small resting curl; morphs
// 1-3 (S5+ via elder_morph_index) each sport a distinct, showier tail.
const FUZZ_PUP: &[Template] = &[[
    "           ",
    "   /\\_/\\   ",
    "  (\u{2591}{eyes}\u{2591})  ",
    "   ='{mouth}'=   ",
    " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\)",
    "  \\\u{2591}{accent}\u{2591}/ ~  ",
    "    d b ~  ",
    "           ",
]];

const FUZZ_ADULT: &[Template] = &[
    // Morph 0 — S4 fuzz: chunky cat-body with a small resting tail curl.
    [
        "   /\\_/\\   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "   ='{mouth}'=   ",
        " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\ ",
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591}) ",
        "  \\\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}/\\ ",
        "   d   b ) ",
        "           ",
    ],
    // Morph 1 — short curved tail extending behind to the right.
    [
        "   /\\_/\\   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "   ='{mouth}'=   ",
        " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\)",
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591})~",
        "  \\\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}/ ~",
        "   d   b ~ ",
        "           ",
    ],
    // Morph 2 — long fluffy tail dangling down behind the body.
    [
        "   /\\_/\\   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "   ='{mouth}'=   ",
        " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\)",
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591}} ",
        "  \\\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}/\u{2591} ",
        "   d   b\u{2591}  ",
        "        '  ",
    ],
    // Morph 3 — tail arched up high over the back, curled forward.
    [
        "          )",
        "   /\\_/\\  )",
        "  (\u{2591}{eyes}\u{2591}) )",
        "   ='{mouth}'=  )",
        " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\)",
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591}) ",
        "  \\\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}/  ",
        "   d   b   ",
    ],
];

const FUZZ_TINY: &[Template; 3] = &[
    // S0 fluff — small chunky fluffball.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "    \u{2591}\u{2591}\u{2591}    ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "    ' '    ",
    ],
    // S1 fuzzling — small forming creature, single ear emerging.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    /\\     ",
        "   \u{2591}\u{2592}\u{2591}\u{2592}\u{2591}   ",
        "   (\u{2591}\u{2591}\u{2591})   ",
        "    ' '    ",
    ],
    // S2 kit — small cat with face and ears.
    [
        "           ",
        "           ",
        "           ",
        "   /\\_/\\   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "   ='{mouth}'=   ",
        "   \\\u{2591}\u{2592}\u{2591}/   ",
        "    d b    ",
    ],
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

// Gooey gelatin: ( ) rounded walls (Blob owns "round"), gravity shading
// (light \u{2591} cap -> heavy \u{2592}\u{2593} belly) with a \u{b0} specular
// highlight, a lopsided cap, and uneven trailing drips so it reads as melting
// rather than a tidy capsule.
const BLOB_PUP: &[Template] = &[[
    "           ",
    "   .--.    ",
    "  (\u{2591}\u{2591}\u{2591}\u{b0})   ",
    " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
    " (\u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}) ",
    " (\u{2592}\u{2592}{pattern}\u{2592}\u{2592}) ",
    "  (\u{2592}\u{2593}{accent}\u{2593}\u{2592})  ",
    "   \u{b0}  .    ",
]];

const BLOB_ADULT: &[Template] = &[
    // Morph 0 — classic gooey melt: lopsided cap, \u{b0} specular, heavy belly,
    // uneven drips.
    [
        "   .--.    ",
        "  (\u{2591}\u{2591}\u{2591}\u{b0})   ",
        " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}) ",
        " (\u{2592}\u{2592}{pattern}\u{2592}\u{2592}) ",
        " (\u{2592}\u{2593}\u{2593}{accent}\u{2593}\u{2593}\u{2592}) ",
        "  \\\u{2592}\u{2593}\u{2593}\u{2592}/   ",
        "   \u{b0}. \u{b0}    ",
    ],
    // Morph 1 — wobble blob shedding bubbles up off the top.
    [
        "  \u{b0} . \u{b0}    ",
        "  (\u{2591}\u{2591}\u{2591}\u{2591})   ",
        " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}) ",
        " (\u{2592}\u{2592}{pattern}\u{2592}\u{2592}) ",
        " (\u{2592}\u{2593}\u{2591}{accent}\u{2591}\u{2593}\u{2592}) ",
        "  \\\u{2592}\u{2593}\u{2592}/    ",
        "   \u{b0} \u{b0}     ",
    ],
    // Morph 2 — mega-blob: full-width body, gravity-shaded, sits low.
    [
        "   .---.   ",
        "  /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\  ",
        " /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\ ",
        "(\u{2591}\u{2592}\u{2591}{eyes}\u{2591}\u{2592}\u{2591})",
        "(\u{2591}\u{2592}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2592}\u{2591})",
        "(\u{2592}\u{2593}\u{2591}{pattern}\u{2591}\u{2593}\u{2592})",
        "(\u{2592}\u{2593}\u{2593}\u{2591}{accent}\u{2591}\u{2593}\u{2593}\u{2592})",
        " \\\u{2592}\u{2593}\u{2593}\u{2593}\u{2593}\u{2593}\u{2592}/ ",
    ],
    // Morph 3 — twin blob: main body with two co-blobs budding at the base.
    [
        "   .--.    ",
        "  (\u{2591}\u{2591}\u{2591})    ",
        " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}) ",
        " (\u{2592}\u{2592}{pattern}\u{2592}\u{2592}) ",
        " (\u{2592}\u{2593}\u{2591}{accent}\u{2591}\u{2593}\u{2592}) ",
        "  \\\u{2592}\u{2593}\u{2592}/    ",
        " (o) \u{b0} (o) ",
    ],
];

const BLOB_TINY: &[Template; 3] = &[
    // S0 droplet — a single drop forming, with a tiny specular glint.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "     .     ",
        "    \u{2591}\u{2591}\u{2591}    ",
        "    \u{b0}.\u{b0}    ",
    ],
    // S1 blip — small gooey droplet.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    .-.    ",
        "   (\u{2591}\u{2591}\u{2591})   ",
        "   (\u{2592}\u{2592}\u{2592})   ",
        "    \u{b0} .    ",
    ],
    // S2 globule — small melting blob with eyes and mouth.
    [
        "           ",
        "           ",
        "           ",
        "   .--.    ",
        "  (\u{2591}\u{2591}\u{2591}\u{b0})   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "  (\u{2592}\u{2592}{mouth}\u{2592}\u{2592})  ",
        "   \u{b0}. .    ",
    ],
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

// Legacy Ghost pools (pre-validation). Retained because the legacy dispatch
// still serves Glitch/Crystal/Mech until their tasks wire the validated cast.
// Billowing edgeless sheet: rounded dome crown, soft two-tone \u{2592}/\u{2591}
// body that fades at the edges (no rigid side walls), and a scalloped \_/ hem.
// Each morph varies the crown (dome / hood / tattered / dense) and the hem
// (bumps / wavy tail / ragged / tangled tendrils) so same-stage pets read as
// distinct specters rather than recolored siblings.
const GHOST_PUP: &[Template] = &[[
    "           ",
    "   .---.   ",
    "  \u{2591}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2591}  ",
    "  \u{2591}\u{2592}{eyes}\u{2592}\u{2591}  ",
    "  \u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}  ",
    "  \u{2591}\u{2592}{pattern}\u{2592}\u{2591}  ",
    " \u{2591}\u{2592}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2592}\u{2591} ",
    "  \\_/\\_/\\  ",
]];

const GHOST_ADULT: &[Template] = &[
    // Morph 0 — classic ghost: dome crown, body billows wider toward the
    // base, three rounded \_/ hem bumps.
    [
        "   .---.   ",
        "  \u{2591}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}{eyes}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}  ",
        " \u{2591}\u{2592}\u{2592}{pattern}\u{2592}\u{2592}\u{2591} ",
        " \u{2591}\u{2592}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2592}\u{2591} ",
        " \u{2591}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2591} ",
        " \\_/\\_/\\_/ ",
    ],
    // Morph 1 — hooded wraith: peaked crown, narrow tapering body, a single
    // wavy tail trailing off.
    [
        "    .^.    ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "   \u{2591}{eyes}\u{2591}   ",
        "  \u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}{pattern}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}\u{2592}{accent}\u{2592}\u{2592}\u{2591}  ",
        "   \\_/\\_   ",
        "    ~ ~    ",
    ],
    // Morph 2 — tattered specter: frayed ~^~ crown, dappled body, ragged
    // uneven hem.
    [
        "   ~^~^~   ",
        "  \u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}{eyes}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}  ",
        " \u{2591}\u{2592}\u{2591}{pattern}\u{2591}\u{2592}\u{2591} ",
        " \u{2591}\u{2592}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2592}\u{2591} ",
        "  \u{2592}\u{2591} \u{2591} \u{2591}\u{2592}  ",
        "  ~  ~  ~  ",
    ],
    // Morph 3 — dense revenant: dark \u{2593}-cored body, billowing wide, with
    // long tangled tendrils.
    [
        "   .---.   ",
        "  \u{2591}\u{2592}\u{2593}\u{2593}\u{2593}\u{2592}\u{2591}  ",
        "  \u{2591}\u{2593}{eyes}\u{2593}\u{2591}  ",
        "  \u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}  ",
        " \u{2591}\u{2592}\u{2593}{pattern}\u{2593}\u{2592}\u{2591} ",
        " \u{2591}\u{2592}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2592}\u{2591} ",
        "  \u{2591} \u{2592} \u{2591} \u{2592}  ",
        "  \\ \\ / /  ",
    ],
];

const GHOST_TINY: &[Template; 3] = &[
    // S0 whisper — a wispy mark drifting.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "    .-.    ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "    ~ ~    ",
    ],
    // S1 wisp — small forming sheet, two-tone shimmer.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    .-.    ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "   \\_/\\_   ",
    ],
    // S2 shade — small ghost sheet with eyes and mouth.
    [
        "           ",
        "           ",
        "    .-.    ",
        "   \u{2591}\u{2592}\u{2592}\u{2592}\u{2591}   ",
        "   \u{2591}{eyes}\u{2591}   ",
        "   \u{2591}\u{2591}{mouth}\u{2591}\u{2591}   ",
        "   \\_/\\_   ",
        "           ",
    ],
];

// ── Glitch ────────────────────────────────────────────────────────
// S3 thread: a torn, misaligned frame (the broken silhouette is the point) —
// matches the adult morphs' offset \u{258c}\u{2590} edges rather than a clean box.
const GLITCH_PUP: &[Template] = &[[
    "           ",
    "   \u{2591}#_\u{2591}    ",
    "  \u{258c}\u{2580}\u{2580} \u{2580}\u{2590}   ",
    "  \u{258c} {eyes}\u{2590}#  ",
    "  \u{258c} {mouth}_\u{258c}    ",
    "  \u{258c}{pattern}\u{2590}    ",
    "   \u{2580}\u{2584}{accent}\u{2584}\u{2580}   ",
    "  _\u{258c}\u{2591} \u{2591}\u{2590}_  ",
]];

const GLITCH_ADULT: &[Template] = &[
    [
        "  \u{2591}#::_ \u{2591}  ",
        " \u{258c}\u{2580}\u{2580}\u{2580} \u{2580}\u{2590}   ",
        " \u{258c} {eyes} \u{2590}#  ",
        " \u{258c}  {mouth}_ \u{258c}   ",
        " \u{258c}{pattern} \u{2590}    ",
        "  \u{2580}\u{2584}{accent}\u{2584}\u{258c}    ",
        " _\u{258c}\u{2591} \u{2591}\u{2590}_   ",
        " :_#\u{2591}#_:   ",
    ],
    [
        " \u{2591}\u{2592}\u{2593}\u{2593}\u{2593}\u{2592}\u{2591}   ",
        "\u{2590}\u{2580}\u{2580}\u{2580}\u{2588}\u{2580}\u{2580}\u{258c}   ",
        "\u{2590}\u{2593}{eyes}\u{2593}\u{258c}    ",
        "\u{2590}\u{2593}\u{2593}{mouth}\u{2593}\u{2593}\u{258c}    ",
        "\u{2590}{pattern}\u{2593}\u{2593}\u{258c}    ",
        "\u{2590}\u{2593}\u{2584}{accent}\u{2584}\u{2593}\u{258c}    ",
        "\u{2590}_\u{258c}\u{2591}\u{2591}\u{2590}_\u{258c}   ",
        "\u{2570}:#\u{2591}#::\u{256f}   ",
    ],
    [
        "  \u{2591}\u{2593}\u{2588}\u{2593}#\u{2591}   ",
        " \u{258c}\u{2580}\u{2580}\u{2588}\u{2588}\u{2590}_   ",
        " \u{258c}\u{2593}{eyes}\u{2590}#   ",
        " \u{258c}\u{2593}\u{2593}{mouth}\u{2593}\u{2590}    ",
        " \u{258c}{pattern}\u{2593}\u{2590}    ",
        " \u{2580}\u{2584}{accent}\u{2584}\u{2593}\u{258c}    ",
        " _\u{2590}\u{2591}\u{2593}\u{2591}\u{258c}_   ",
        "  :#\u{2588}#:_   ",
    ],
];

const GLITCH_TINY: &[Template; 3] = &[
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "     \u{2593}     ",
        "     \u{2588}     ",
        "     \u{2591}     ",
    ],
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    \u{2593}\u{2591}\u{2593}    ",
        "    \u{2588}\u{2580}\u{2588}    ",
        "    \u{2588}\u{2584}\u{2588}    ",
        "    \u{2591} \u{2591}    ",
    ],
    [
        "           ",
        "           ",
        "           ",
        "   \u{2593}\u{2591}\u{2592}\u{2591}\u{2593}   ",
        "   \u{2588}\u{2580}\u{2580}\u{2580}\u{2588}   ",
        "   \u{2588}{eyes}\u{2588}   ",
        "   \u{2588}\u{2584}{mouth}\u{2584}\u{2588}   ",
        "   \u{2591}{pattern}\u{2591}   ",
    ],
];

// ── Crystal ───────────────────────────────────────────────────────
// Filled/shaded crystals using block density (\u{2588} \u{2593} \u{2592} \u{2591})
// as facet shading. Higher stages cluster a dominant crystal with satellites.
const CRYSTAL_PUP: &[Template] = &[[
    "           ",
    "     /\\    ",
    "    /\u{2588}\u{2588}\\   ",
    "   /{eyes}\\   ",
    " /\\ \\{mouth}/ /\\ ",
    " \u{2593}\u{2593}\\{pattern}/\u{2593}\u{2593} ",
    " \\/ \\{accent}/ \\/ ",
    "    \\/     ",
]];

const CRYSTAL_ADULT: &[Template] = &[
    // Morph 0 — single dominant gem (S4 default, S5/S6 morph 0).
    [
        "    /\\     ",
        "   /\u{2593}\u{2588}\\    ",
        "  /\u{2593}\u{2588}\u{2588}\u{2588}\\   ",
        " /\u{2593}\u{2588}\u{2588}{eyes}\u{2588}\\ ",
        " \\\u{2592}\u{2588}\u{2588}{mouth}\u{2588}\u{2588}\u{2588}/ ",
        "  \\\u{2592}{pattern}\u{2588}/  ",
        "   \\\u{2592}{accent}\u{2588}/   ",
        "    \\/     ",
    ],
    // Morph 1 — dominant crystal flanked by two satellites at the base.
    [
        "    /\\     ",
        "   /\u{2593}\u{2588}\\    ",
        "  /\u{2593}\u{2588}\u{2588}\u{2588}\\   ",
        " /\u{2593}\u{2588}\u{2588}{eyes}\u{2588}\\ ",
        " \\\u{2592}\u{2588}\u{2588}{mouth}\u{2588}\u{2588}\u{2588}/ ",
        "/\\\\\u{2592}{pattern}\u{2588}//\\",
        "\u{2593}\u{2593} \\\u{2592}{accent}\u{2588}/ \u{2593}\u{2593}",
        "\\/   \u{25bc}   \\/",
    ],
    // Morph 2 — tall asymmetric geode with one side shard.
    [
        "    /\\     ",
        "   /\u{2588}\u{2588}\\    ",
        "  /\u{2593}\u{2588}\u{2588}\\    ",
        " /\u{2588}\u{2588}{eyes}\u{2588}\\/\\",
        " \\\u{2592}\u{2588}{mouth}\u{2588}\u{2593}/ \\/",
        "  \\\u{2592}{pattern}/   ",
        "   \\\u{2592}{accent}/    ",
        "    \\/     ",
    ],
];

const CRYSTAL_TINY: &[Template; 3] = &[
    // S0 grain — a tiny seed sparkle.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "     \u{2726}     ",
        "    \u{25c6}\u{25c7}\u{25c6}    ",
        "     \u{b7}     ",
    ],
    // S1 shard — first solid shard.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    /\\     ",
        "   /\u{2588}\u{2588}\\    ",
        "   \\\u{2593}\u{2592}/    ",
        "    \\/     ",
    ],
    // S2 facet — eyes and mouth visible, gem proportions.
    [
        "           ",
        "           ",
        "           ",
        "    /\\     ",
        "   /\u{2588}\u{2588}\\    ",
        "  /\u{2593}{eyes}\u{2593}\\  ",
        "  \\\u{2593}\u{2593}{mouth}\u{2593}\u{2593}/  ",
        "   \\\u{2593}\u{2593}/    ",
    ],
];

// ── Mech ──────────────────────────────────────────────────────────
// Chunky industrial chassis using \u{2588}/\u{2592} for armor plating and
// double-line \u{2550}/\u{2551} chrome. Elder morphs upgrade the chassis:
// heavier wide frame, hovering drone, or bracketed titan plating.
// S3 drone: an articulated chassis with bolt-shoulders (\u{2534}), a hip plate
// (\u{252c}), and split legs (\u{2575}) — reads as a little robot, not a plain box.
const MECH_PUP: &[Template] = &[[
    "           ",
    "    _._    ",
    "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
    "   \u{2502}{eyes}\u{2502}   ",
    "  \u{250c}\u{2534}\u{2500}{mouth}\u{2500}\u{2534}\u{2510}  ",
    "  \u{2502}\u{2591}{pattern}\u{2591}\u{2502}  ",
    "  \u{2514}\u{252c}\u{2500}{accent}\u{2500}\u{252c}\u{2518}  ",
    "   \u{2575}   \u{2575}   ",
]];

const MECH_ADULT: &[Template] = &[
    // Morph 0 — S4 mech: humanoid build with bolt-shoulders and split feet.
    [
        "    /\u{b7}\\    ",
        "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
        "   \u{2502}{eyes}\u{2502}   ",
        "   \u{2502}\u{b7}{mouth}\u{b7}\u{2502}   ",
        " \u{250c}\u{2500}\u{2534}\u{2500}\u{2500}\u{2500}\u{2534}\u{2500}\u{2510} ",
        " \u{2502}\u{2588}\u{2591}{pattern}\u{2591}\u{2588}\u{2502} ",
        " \u{2502}\u{2588}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2588}\u{2502} ",
        " \u{2514}\u{2500}\u{2518}\u{203e}\u{203e}\u{203e}\u{2514}\u{2500}\u{2518} ",
    ],
    // Morph 1 — sentinel: crossed-mast antenna, full-width platform base.
    [
        "  \\\\__//   ",
        "  \u{250c}\u{2567}\u{2550}\u{2567}\u{2550}\u{2567}\u{2510}  ",
        "  \u{2502}\u{2593}{eyes}\u{2593}\u{2502}  ",
        "  \u{2502}\u{2593}\u{b7}{mouth}\u{b7}\u{2593}\u{2502}  ",
        "\u{250c}\u{2500}\u{2534}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2534}\u{2500}\u{2510}",
        "\u{2502}\u{2591}\u{2591}\u{2591}{pattern}\u{2591}\u{2591}\u{2591}\u{2502}",
        "\u{2502}\u{2591}\u{2591}\u{2591}\u{2591}{accent}\u{2591}\u{2591}\u{2591}\u{2591}\u{2502}",
        "\u{2514}\u{2500}\u{2510}\u{2568}\u{2550}\u{2550}\u{2550}\u{2568}\u{250c}\u{2500}\u{2518}",
    ],
    // Morph 2 — drone: sensor halo, narrowed neck, hover exhaust trail.
    [
        " \u{b0} \\\u{b7}/ \u{b0}   ",
        "   \u{2553}\u{2500}\u{2500}\u{2500}\u{2556}   ",
        "   \u{2551}{eyes}\u{2551}   ",
        "   \u{2551}\u{b7}{mouth}\u{b7}\u{2551}   ",
        "    \u{2559}\u{2565}\u{255c}    ",
        "  \u{2503}\u{2592}{pattern}\u{2592}\u{2503}  ",
        "  \u{2517}\u{2501}\u{2501}{accent}\u{2501}\u{2501}\u{251b}  ",
        "  \u{2591} \u{2591} \u{2591} \u{2591}  ",
    ],
    // Morph 3 — bio-mech: heavy chrome, exposed core indicators, bolted ends.
    [
        "   __\u{2588}__   ",
        "   \u{250f}\u{2501}\u{2501}\u{2501}\u{2513}   ",
        "   \u{2503}{eyes}\u{2503}   ",
        "   \u{2503}\u{b7}{mouth}\u{b7}\u{2503}   ",
        " \u{250f}\u{2501}\u{2567}\u{2550}\u{2550}\u{2550}\u{2567}\u{2501}\u{2513} ",
        " \u{2503}\u{2592}\u{2591}{pattern}\u{2591}\u{2592}\u{2503} ",
        " \u{2503}\u{2592}\u{25c9}\u{2591}{accent}\u{2591}\u{25c9}\u{2592}\u{2503} ",
        " \u{2517}\u{2501}\u{2568}\u{2550}\u{2550}\u{2550}\u{2568}\u{2501}\u{251b} ",
    ],
];

const MECH_TINY: &[Template; 3] = &[
    // S0 chip — tiny printed-circuit chip with one indicator.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "    \u{250c}\u{2500}\u{2510}    ",
        "    \u{2502}\u{2588}\u{2502}    ",
        "    \u{2514}\u{2500}\u{2518}    ",
    ],
    // S1 bolt — small mech head, blinking indicator.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    _._    ",
        "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
        "   \u{2502}o o\u{2502}   ",
        "   \u{2514}\u{2500}\u{2500}\u{2500}\u{2518}   ",
    ],
    // S2 rivet — small forming robot with eyes, pattern, mouth slot.
    [
        "           ",
        "           ",
        "           ",
        "    _._    ",
        "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
        "   \u{2502}{eyes}\u{2502}   ",
        "   \u{2502}{pattern}\u{2502}   ",
        "   \u{2514}\u{2500}{mouth}\u{2500}\u{2518}   ",
    ],
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
    fn apply_interior_texture_is_identity_in_phase_one() {
        // Phase 1 ships the texture hook as a passthrough: the rendered lines equal
        // the base template (slots still unresolved {} markers) regardless of seed.
        for species in Species::all() {
            for stage in ALL_STAGES {
                let base = stage_base_template(species, stage);
                for seed in [0u64, 1, 7, 99, 360, u64::from(u16::MAX)] {
                    let textured = apply_interior_texture(base, species, stage, seed);
                    for (row, (a, b)) in base.iter().zip(textured.iter()).enumerate() {
                        assert_eq!(
                            *a,
                            b.as_str(),
                            "{species:?} {stage:?} seed={seed} row={row} must be unchanged"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn stage_template_lines_matches_base_after_slot_widths() {
        // stage_template_lines feeds render.rs; in Phase 1 it equals the base.
        for species in Species::all() {
            for stage in ALL_STAGES {
                let base = stage_base_template(species, stage);
                let lines = stage_template_lines(species, stage, 42);
                assert_eq!(lines.len(), 8);
                for (a, b) in base.iter().zip(lines.iter()) {
                    assert_eq!(*a, b.as_str());
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
