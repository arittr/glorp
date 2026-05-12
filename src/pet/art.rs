use crate::game::evolution::Stage;
use crate::pet::generation::Species;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageKey {
    S0,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
}

impl StageKey {
    pub(crate) fn index(self) -> usize {
        match self {
            StageKey::S0 => 0,
            StageKey::S1 => 1,
            StageKey::S2 => 2,
            StageKey::S3 => 3,
            StageKey::S4 => 4,
            StageKey::S5 => 5,
            StageKey::S6 => 6,
        }
    }
}

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
        Species::Mech => ["chip", "bolt", "rivet", "drone", "mech", "warmech", "titan"],
    };
    labels[stage_key(stage).index()]
}

pub fn morph_count(species: Species, stage: Stage) -> usize {
    let key = stage_key(stage);
    match key {
        StageKey::S0 | StageKey::S1 | StageKey::S2 => 1,
        StageKey::S3 => pup_templates(species).len(),
        StageKey::S4 | StageKey::S5 | StageKey::S6 => adult_templates(species).len(),
    }
}

pub(crate) fn template_lines(
    species: Species,
    stage: StageKey,
    morph_index: usize,
    morph_pup_index: usize,
) -> Vec<&'static str> {
    match stage {
        StageKey::S0 => tiny_template(species, 0).to_vec(),
        StageKey::S1 => tiny_template(species, 1).to_vec(),
        StageKey::S2 => tiny_template(species, 2).to_vec(),
        StageKey::S3 => {
            let templates = pup_templates(species);
            templates[morph_pup_index % templates.len()].to_vec()
        }
        StageKey::S4 => adult_templates(species)[0].to_vec(),
        StageKey::S5 => {
            let templates = adult_templates(species);
            templates[elder_morph_index(species, morph_index, templates.len())].to_vec()
        }
        StageKey::S6 => {
            let templates = adult_templates(species);
            let body = templates[elder_morph_index(species, morph_index, templates.len())];
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

// Some species visibly transform at S5+: Crystal pets cluster, Ghost pets grow
// longer tentacles, Blob pets bubble and bud co-blobs, Fuzz pets grow a tail.
// Skip the singleton morph 0 at elder stages for those species so every elder
// pet reads as the evolved form while still preserving morph-driven variation
// across the remaining morphs.
fn elder_morph_index(species: Species, morph_index: usize, len: usize) -> usize {
    let skip_first = matches!(
        species,
        Species::Crystal | Species::Ghost | Species::Blob | Species::Fuzz
    ) && len > 1;
    if skip_first {
        1 + (morph_index % (len - 1))
    } else {
        morph_index % len
    }
}

pub(crate) fn stage_key(stage: Stage) -> StageKey {
    match stage {
        Stage::S0 => StageKey::S0,
        Stage::S1 => StageKey::S1,
        Stage::S2 => StageKey::S2,
        Stage::S3 => StageKey::S3,
        Stage::S4 => StageKey::S4,
        Stage::S5 => StageKey::S5,
        Stage::S6 => StageKey::S6,
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
// that grows at elder stages. Morph 0 is the tail-less S4 form; morphs 1-3
// (S5+ via elder_morph_index) each sport a distinct tail.
const FUZZ_PUP: &[Template] = &[[
    "           ",
    "           ",
    "   /\\_/\\   ",
    "  (\u{2591}{eyes}\u{2591})  ",
    "   ='{mouth}'=   ",
    " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\ ",
    "  \\\u{2591}{accent}\u{2591}/   ",
    "    d b    ",
]];

const FUZZ_ADULT: &[Template] = &[
    // Morph 0 — S4 fuzz: chunky cat-body with no tail yet.
    [
        "           ",
        "   /\\_/\\   ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "   ='{mouth}'=   ",
        " /\u{2591}\u{2592}{pattern}\u{2592}\u{2591}\\ ",
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591}) ",
        "  \\\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}/  ",
        "   d   b   ",
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
        " (\u{2591}\u{2592}\u{2591}{accent}\u{2591}\u{2592}\u{2591}}",
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
// Chunky filled gelatinous bodies using ( ) rounded walls + \u{2591}/\u{2592}
// density for jelly translucency. Variants differ in cap silhouette, body
// width, and trailing bubble pattern.
const BLOB_PUP: &[Template] = &[[
    "           ",
    "   .---.   ",
    "  /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\  ",
    " (\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}) ",
    " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
    " (\u{2591}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2591}) ",
    "  \\\u{2591}{pattern}\u{2591}/  ",
    "   '_{accent}_'   ",
]];

const BLOB_ADULT: &[Template] = &[
    // Morph 0 — classic round chunky blob with trailing dribbles.
    [
        "   .---.   ",
        "  /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\  ",
        " (\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}) ",
        " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}{pattern}\u{2592}\u{2591}) ",
        "  \\\u{2591}\u{2592}{accent}\u{2592}\u{2591}/  ",
        "   \u{b0} o \u{b0}   ",
    ],
    // Morph 1 — wobble blob with bubbles drifting up off the top.
    [
        "  \u{b0} . o .  ",
        "  .-._.-.  ",
        " (\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}{eyes}\u{2592}\u{2591}) ",
        " (\u{2591}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2591}{pattern}\u{2591}\u{2591}) ",
        "  \\\u{2591}\u{2591}{accent}\u{2591}\u{2591}/  ",
        "   \u{b0} \u{b0} \u{b0}   ",
    ],
    // Morph 2 — mega-blob: full-width body extends to the edges, wide cap.
    [
        "           ",
        "  .\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}.  ",
        " /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\ ",
        "(\u{2591}\u{2592}\u{2591}{eyes}\u{2591}\u{2592}\u{2591})",
        "(\u{2591}\u{2592}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2592}\u{2591})",
        "(\u{2591}\u{2592}\u{2591}{pattern}\u{2591}\u{2592}\u{2591})",
        "  \\\u{2591}\u{2591}{accent}\u{2591}\u{2591}/  ",
        " ' . \u{b0} . ' ",
    ],
    // Morph 3 — twin blob: main body with two tiny co-blobs budding at base.
    [
        "   .---.   ",
        "  /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\  ",
        " (\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2591}{eyes}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2591}) ",
        " (\u{2591}\u{2592}{pattern}\u{2592}\u{2591}) ",
        "  \\\u{2591}\u{2591}{accent}\u{2591}\u{2591}/  ",
        " (o) \u{b0} (o) ",
    ],
];

const BLOB_TINY: &[Template; 3] = &[
    // S0 droplet — single drop forming.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "     .     ",
        "    o.o    ",
        "    '.'    ",
    ],
    // S1 blip — small chunky droplet.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    .-.    ",
        "   /\u{2591}\u{2591}\u{2591}\\   ",
        "   (\u{2591}\u{2592}\u{2591})   ",
        "    '_'    ",
    ],
    // S2 globule — small round blob with eyes and mouth.
    [
        "           ",
        "           ",
        "           ",
        "   .---.   ",
        "  /\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\\  ",
        "  (\u{2591}{eyes}\u{2591})  ",
        "  (\u{2591}{mouth}\u{2591})  ",
        "   '___'   ",
    ],
];

// ── Ghost ─────────────────────────────────────────────────────────
// Chunky filled bodies (\u{2588} outline + \u{2592}/\u{2591} two-tone interior
// shimmer) with dangling tentacles below. Each morph varies head silhouette
// AND tentacle style+count, so pets at the same stage read as distinct
// specters rather than recolored siblings.
const GHOST_PUP: &[Template] = &[[
    "           ",
    "   _____   ",
    "  /\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\\  ",
    " |\u{2588}\u{2591}{eyes}\u{2591}\u{2588}| ",
    " |\u{2588}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2588}| ",
    "  \\\u{2588}{pattern}\u{2588}/  ",
    "   \\\u{2588}{accent}\u{2588}/   ",
    "    |~|    ",
]];

const GHOST_ADULT: &[Template] = &[
    // Morph 0 — classic S4 ghost: rounded cap, full chunky body, four
    // short tentacle stubs with sharp drip tips.
    [
        "   _____   ",
        "  /\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\\  ",
        " |\u{2588}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2588}| ",
        " |\u{2588}\u{2591}{eyes}\u{2591}\u{2588}| ",
        " |\u{2588}\u{2591}\u{2591}{mouth}\u{2591}\u{2591}\u{2588}| ",
        " |\u{2588}\u{2592}{pattern}\u{2592}\u{2588}| ",
        "  | |{accent}| |  ",
        "  ' ' ' '  ",
    ],
    // Morph 1 — hooded wraith: peaked _-^-_ hood, compact body, three
    // curled tendrils meeting in a wavy drip.
    [
        "   _-^-_   ",
        "  /\u{2588}\u{2592}\u{2588}\u{2592}\u{2588}\\  ",
        " |\u{2588}\u{2592}{eyes}\u{2592}\u{2588}| ",
        " |\u{2588}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2588}| ",
        " |\u{2588}\u{2592}{pattern}\u{2592}\u{2588}| ",
        "   | | |   ",
        "   ) {accent} (   ",
        "   ' ~ '   ",
    ],
    // Morph 2 — tattered specter: jagged ~v~v~ torn top, lighter dappled
    // body, four wavy tendrils staggered for asymmetric drip.
    [
        "   ~v~v~   ",
        "  \u{2591}\u{2588}\u{2591}\u{2588}\u{2591}\u{2588}\u{2591}  ",
        " |\u{2591}\u{2592}{eyes}\u{2592}\u{2591}| ",
        " |\u{2591}\u{2592}\u{2591}{mouth}\u{2591}\u{2592}\u{2591}| ",
        " |\u{2591}\u{2592}{pattern}\u{2592}\u{2591}| ",
        "  \u{2307} \u{2307}{accent}\u{2307} \u{2307}  ",
        "   \u{2307}  \u{2307}   ",
        "  \u{2307}  \u{2307}  \u{2307}  ",
    ],
    // Morph 3 — chaotic phantom: torn jagged shroud, dense ▓░ dappled body,
    // V-pattern tendrils that tangle as they fall.
    [
        "   \\v/v\\   ",
        "  /\u{2593}\u{2591}\u{2593}\u{2591}\u{2593}\\  ",
        " |\u{2593}\u{2591}{eyes}\u{2591}\u{2593}| ",
        " |\u{2593}\u{2591}\u{2592}{mouth}\u{2592}\u{2591}\u{2593}| ",
        " |\u{2593}\u{2591}{pattern}\u{2591}\u{2593}| ",
        "  / \\{accent}/ \\  ",
        "   \\ v /   ",
        "    \\/    ",
    ],
];

const GHOST_TINY: &[Template; 3] = &[
    // S0 whisper — just a wispy mark drifting.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "     '     ",
        "    . .    ",
        "     ~     ",
    ],
    // S1 wisp — small forming shape, two-tone shimmer.
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "    ___    ",
        "   /\u{2588}\u{2588}\u{2588}\\   ",
        "   \\\u{2591}\u{2592}\u{2591}/   ",
        "    ~ ~    ",
    ],
    // S2 shade — small chunky ghost with eyes and mouth.
    [
        "           ",
        "           ",
        "           ",
        "    ___    ",
        "   /\u{2588}\u{2588}\u{2588}\\   ",
        "  |\u{2588}{eyes}\u{2588}|  ",
        "  |\u{2588}\u{2591}{mouth}\u{2591}\u{2588}|  ",
        "   \\\u{2588}\u{2592}\u{2588}/   ",
    ],
];

// ── Glitch ────────────────────────────────────────────────────────
const GLITCH_PUP: &[Template] = &[[
    "           ",
    "           ",
    "   \u{2593}\u{2591}\u{2592}\u{2591}\u{2593}   ",
    "  \u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2588}  ",
    "  \u{2588} {eyes} \u{2588}  ",
    "  \u{2588}\u{2593} {mouth} \u{2593}\u{2588}  ",
    "  \u{2588} {pattern} \u{2588}  ",
    "   \u{2591} {accent} \u{2591}   ",
]];

const GLITCH_ADULT: &[Template] = &[
    [
        "   \u{2593}\u{2591}\u{2592}\u{2591}\u{2593}   ",
        "  \u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2588}  ",
        "  \u{2588}\u{2592}{eyes}\u{2592}\u{2588}  ",
        "  \u{2591} {pattern} \u{2591}  ",
        "  \u{2588}\u{2593} {mouth} \u{2593}\u{2588}  ",
        "  \u{2588}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2588}  ",
        "   \u{2592}\u{2591}{accent}\u{2591}\u{2592}   ",
        "  \u{2591} \u{2591} \u{2591} \u{2591}  ",
    ],
    [
        "  \u{2591}\u{2591}\u{2593}\u{2592}\u{2591}\u{2593}\u{2591}  ",
        "  \u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2588}  ",
        " \u{2593}\u{2588} {eyes} \u{2588}\u{2593} ",
        " \u{2592}\u{2588}  {mouth}  \u{2588}\u{2592} ",
        " \u{2591}\u{2588} {pattern} \u{2588}\u{2591} ",
        "  \u{2588}\u{2593}\u{2591}{accent}\u{2591}\u{2593}\u{2588}  ",
        "  \u{2588}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2588}  ",
        "  \u{2591} \u{2591}\u{2591}\u{2591} \u{2591}  ",
    ],
    [
        "  \u{2593}\u{2591}\u{2593}\u{2591}\u{2593}\u{2591}\u{2593}  ",
        "  \u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2588}  ",
        "  \u{2588} {eyes} \u{2588}  ",
        "  \u{2588}  {mouth}  \u{2588}  ",
        "  \u{2588} {pattern} \u{2588}  ",
        "  \u{2588}\u{2584}\u{2584}{accent}\u{2584}\u{2584}\u{2588}  ",
        "  \u{2593} \u{2591} \u{2593} \u{2591}  ",
        "  \u{2591} \u{2593} \u{2591} \u{2593}  ",
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
        "  \\\u{2592}{pattern}/    ",
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
const MECH_PUP: &[Template] = &[[
    "           ",
    "           ",
    "    _._    ",
    "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
    "   \u{2502}{eyes}\u{2502}   ",
    "   \u{2502}.{mouth}.\u{2502}   ",
    "   \u{2502}{pattern}\u{2502}   ",
    "   \u{2514}\u{2500}{accent}\u{2500}\u{2518}   ",
]];

const MECH_ADULT: &[Template] = &[
    [
        "    /\u{b7}\\    ",
        "   \u{250c}\u{2500}\u{2500}\u{2500}\u{2510}   ",
        "  \u{250c}\u{2518}{eyes}\u{2514}\u{2510}  ",
        "  \u{2502} .{mouth}. \u{2502}  ",
        "  \u{2502} {pattern} \u{2502}  ",
        "  \u{2514}\u{2500}\u{2500}{accent}\u{2500}\u{2500}\u{2518}  ",
        "  /\u{2017}\u{2017}\u{2017}\u{2017}\u{2017}\\  ",
        "  \u{203e}\u{203e}   \u{203e}\u{203e}  ",
    ],
    [
        "  | /\u{b7}\\ |  ",
        "  \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}  ",
        "  \u{2502} {eyes} \u{2502}  ",
        " -\u{2502} .{mouth}. \u{2502}- ",
        " -\u{2502} {pattern} \u{2502}- ",
        "  \u{2514}\u{2500}\u{2500}{accent}\u{2500}\u{2500}\u{2518}  ",
        "  \u{250c}\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}\u{2510}  ",
        "  \u{203e}\u{203e}   \u{203e}\u{203e}  ",
    ],
    [
        "   _.{accent}._   ",
        "  \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}  ",
        "  \u{2502} {eyes} \u{2502}  ",
        "  \u{2502}  {mouth}  \u{2502}  ",
        "  \u{2502} {pattern} \u{2502}  ",
        "  \u{2514}\u{252c}\u{2500}\u{2500}\u{2500}\u{252c}\u{2518}  ",
        "  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}  ",
        "  \u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}  ",
    ],
];

const MECH_TINY: &[Template; 3] = &[
    [
        "           ",
        "           ",
        "           ",
        "           ",
        "           ",
        "    \u{250c}\u{2500}\u{2510}    ",
        "    \u{2502}\u{b7}\u{2502}    ",
        "    \u{2514}\u{2500}\u{2518}    ",
    ],
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
