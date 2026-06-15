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

pub fn morph_count(species: Species, stage: Stage) -> usize {
    match stage {
        Stage::S0 | Stage::S1 | Stage::S2 => 1,
        Stage::S3 => pup_templates(species).len(),
        Stage::S4 | Stage::S5 | Stage::S6 => adult_templates(species).len(),
    }
}

pub(crate) fn template_lines(
    species: Species,
    stage: Stage,
    morph_index: usize,
    morph_pup_index: usize,
) -> Vec<&'static str> {
    match stage {
        Stage::S0 => tiny_template(species, 0).to_vec(),
        Stage::S1 => tiny_template(species, 1).to_vec(),
        Stage::S2 => tiny_template(species, 2).to_vec(),
        Stage::S3 => {
            let templates = pup_templates(species);
            templates[morph_pup_index % templates.len()].to_vec()
        }
        Stage::S4 => adult_templates(species)[0].to_vec(),
        Stage::S5 => {
            let templates = adult_templates(species);
            templates[elder_morph_index(species, morph_index, templates)].to_vec()
        }
        Stage::S6 => {
            let templates = adult_templates(species);
            let body = templates[elder_morph_index(species, morph_index, templates)];
            // Sage frame: top + body[1..body.len()-1] + bottom.
            let mut framed: Vec<&'static str> = Vec::with_capacity(body.len());
            framed.push(SAGE_TOP);
            for line in &body[1..body.len() - 1] {
                framed.push(line);
            }
            framed.push(SAGE_BOT);
            framed
        }
    }
}

// Most species visibly transform at S5+: Crystal pets cluster, Ghost pets grow
// longer tentacles, Blob pets bubble and bud co-blobs, Fuzz pets grow a tail,
// Glitch pets become denser packets, Mech pets upgrade chassis. Skip the
// singleton morph 0 at elder stages so
// every elder pet reads as the evolved form while still preserving
// morph-driven variation across the remaining morphs.
fn elder_morph_index(species: Species, morph_index: usize, templates: &[Template]) -> usize {
    debug_assert!(!templates.is_empty(), "adult templates must be non-empty");
    let len = templates.len();
    let skip_first = matches!(
        species,
        Species::Crystal
            | Species::Ghost
            | Species::Blob
            | Species::Fuzz
            | Species::Glitch
            | Species::Mech
    ) && len > 1;
    if skip_first {
        1 + (morph_index % (len - 1))
    } else {
        morph_index % len
    }
}

// Sage stage = adult + sparkle frame on top + bottom (per pet.jsx).
pub(crate) const SAGE_TOP: &str = " *  .  *   ";
pub(crate) const SAGE_BOT: &str = " \u{2726} \u{2727} \u{2726} \u{2727} \u{2726} ";

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
        "\u{2590}\u{2580}\u{2580}\u{2580} \u{2580}\u{2580}\u{258c}   ",
        "\u{2590} {eyes} \u{258c}    ",
        "\u{2590}  {mouth}  \u{258c}    ",
        "\u{2590}{pattern} \u{258c}     ",
        " \u{2580}\u{2584}{accent}\u{2584}\u{2580}     ",
        " _\u{258c}\u{2591}\u{2591}\u{2590}_    ",
        " :_#\u{2591}#_:   ",
    ],
    [
        "  \u{2591}\u{2593}\u{2591}#\u{2591}    ",
        " \u{258c}\u{2580}\u{2580}\u{2580}\u{2580}\u{2590}_   ",
        " \u{258c} {eyes}\u{2590}#   ",
        " \u{258c}  {mouth} \u{2590}    ",
        " \u{258c}{pattern}\u{2590}     ",
        " \u{2580}\u{2584}{accent}\u{2584}\u{258c}     ",
        " _\u{2590}\u{2591} \u{2591}\u{258c}_   ",
        "  :#_#:    ",
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
        // The art renderer assumes each template line is exactly 11 chars;
        // see the `Template` type alias and `frame_with_particles` in
        // `src/pet/render.rs`. Drift in either direction breaks alignment.
        for species in Species::all() {
            for stage in ALL_STAGES {
                for morph_index in 0..6 {
                    for morph_pup_index in 0..6 {
                        let lines = template_lines(species, stage, morph_index, morph_pup_index);
                        for (row, line) in lines.iter().enumerate() {
                            let rendered = substitute_slots(line);
                            let width = rendered.chars().count();
                            assert_eq!(
                                width, 11,
                                "template width != 11 for species={species:?} stage={stage:?} \
                                 morph={morph_index} pup_morph={morph_pup_index} row={row}: \
                                 {rendered:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_template_line_is_eleven_display_columns() {
        use unicode_width::UnicodeWidthStr;
        // Terminal columns, not code points. Policy: art uses only glyphs that
        // are width-1 under the ambiguous=narrow assumption (unicode-width's
        // default `width()`); this catches any always-width-2 (CJK/emoji) glyph
        // that `.chars().count()` would miss.
        for species in Species::all() {
            for stage in ALL_STAGES {
                for morph_index in 0..6 {
                    for morph_pup_index in 0..6 {
                        let lines = template_lines(species, stage, morph_index, morph_pup_index);
                        for (row, line) in lines.iter().enumerate() {
                            let rendered = substitute_slots(line);
                            let columns = UnicodeWidthStr::width(rendered.as_str());
                            assert_eq!(
                                columns, 11,
                                "display width != 11 for species={species:?} stage={stage:?} \
                                 morph={morph_index} pup_morph={morph_pup_index} row={row}: \
                                 {rendered:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_template_is_eight_lines() {
        // frame_with_particles overlays exactly 8 art rows (render.rs `.take(8)`);
        // a template with a different line count would silently clip or shift.
        for species in Species::all() {
            for stage in ALL_STAGES {
                for morph_index in 0..6 {
                    for morph_pup_index in 0..6 {
                        let lines = template_lines(species, stage, morph_index, morph_pup_index);
                        assert_eq!(
                            lines.len(),
                            8,
                            "template height != 8 for species={species:?} stage={stage:?} \
                             morph={morph_index} pup_morph={morph_pup_index}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn elder_morph_skips_singleton_for_carved_species() {
        // Pets in this set evolve a distinct elder silhouette; the singleton
        // morph 0 is the S4 form and must never be reused at S5/S6.
        let carved = [
            Species::Crystal,
            Species::Ghost,
            Species::Blob,
            Species::Fuzz,
            Species::Glitch,
            Species::Mech,
        ];
        for species in carved {
            let templates = adult_templates(species);
            assert!(
                templates.len() > 1,
                "{species:?} needs at least 2 adult morphs for elder skip to apply"
            );
            for morph_index in 0..6 {
                let idx = elder_morph_index(species, morph_index, templates);
                assert!(
                    idx >= 1,
                    "{species:?} elder_morph_index returned 0 for morph={morph_index}"
                );
            }
        }
    }

    #[test]
    fn elder_morph_skips_singleton_for_glitch() {
        let templates = adult_templates(Species::Glitch);
        assert!(
            templates.len() > 1,
            "Glitch needs at least 2 adult morphs for elder skip to apply"
        );
        for morph_index in 0..6 {
            let idx = elder_morph_index(Species::Glitch, morph_index, templates);
            assert!(
                idx >= 1,
                "Glitch elder stage reused the weaker S4 morph for morph={morph_index}"
            );
        }
    }
}
