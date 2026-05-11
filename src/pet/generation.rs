use crate::game::evolution::Stage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Species {
    Fuzz,
    Blob,
    Ghost,
    Glitch,
    Crystal,
    Mech,
}

impl Species {
    pub const fn all() -> [Species; 6] {
        [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
            Species::Mech,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Species::Fuzz => "fuzz",
            Species::Blob => "blob",
            Species::Ghost => "ghost",
            Species::Glitch => "glitch",
            Species::Crystal => "crystal",
            Species::Mech => "mech",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedPet {
    pub seed: String,
    pub species: Species,
    pub generated_name: String,
    pub traits: VisibleTraits,
    pub animation_phase: AnimationPhase,
}

impl GeneratedPet {
    pub fn with_species_for_test(mut self, species: Species) -> Self {
        self.species = species;
        let hash = fnv1a64(&format!("{}:{}", self.seed, species.as_str()));
        let mut rng = StableRng::new(hash);
        self.generated_name = generated_name(species, &mut rng);
        self.traits = visible_traits(species, &mut rng);
        self.animation_phase = animation_phase(&mut rng);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleTraits {
    pub eyes: String,
    pub mouth: String,
    pub pattern: String,
    pub accent: String,
    pub palette_index: usize,
    pub morph_index: usize,
    #[serde(default)]
    pub morph_pup_index: usize,
    pub seed_hue: u16,
    pub saturation_percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationPhase {
    pub breath: u8,
    pub blink: u8,
    pub flavor: u8,
}

pub fn generate_pet(seed: &str) -> GeneratedPet {
    let hash = fnv1a64(seed);
    let species = Species::all()[(hash as usize) % Species::all().len()];
    let mut rng = StableRng::new(hash ^ 0x9e37_79b9_7f4a_7c15);
    GeneratedPet {
        seed: seed.to_string(),
        species,
        generated_name: generated_name(species, &mut rng),
        traits: visible_traits(species, &mut rng),
        animation_phase: animation_phase(&mut rng),
    }
}

pub fn resolve_accepted_name(generated_name: &str, replacement: Option<&str>) -> String {
    replacement
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(generated_name)
        .to_string()
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

pub fn morph_count(_species: Species, stage: Stage) -> usize {
    let key = stage_key(stage);
    match key {
        StageKey::S0 | StageKey::S1 | StageKey::S2 => 1,
        StageKey::S3 => 1, // pup_templates(species).len() is always 1
        StageKey::S4 | StageKey::S5 | StageKey::S6 => 3, // adult_templates(species).len() is always 3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageKey {
    S0,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
}

impl StageKey {
    fn index(self) -> usize {
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

fn stage_key(stage: Stage) -> StageKey {
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

fn generated_name(species: Species, rng: &mut StableRng) -> String {
    match species {
        Species::Fuzz => join_parts(rng, &["mo", "puff", "kib", "lu", "chi", "lo", "mi"], 2),
        Species::Blob => join_parts(rng, &["bub", "glo", "plip", "momo", "squ", "lo", "gum"], 2),
        Species::Ghost => join_parts(rng, &["wisp", "veil", "noct", "oma", "hush", "lume"], 2),
        Species::Glitch => {
            let left = pick(rng, &["bit", "hex", "vex", "0x", "mux", "irq"]);
            let right = pick(rng, &["loop", "zap", "404", "jit", "xor"]);
            format!("{left}-{right}")
        }
        Species::Crystal => join_parts(rng, &["ori", "shard", "lux", "facet", "mica", "opal"], 2),
        Species::Mech => {
            let left = pick(rng, &["axl", "bolt", "rivet", "cog", "servo", "unit"]);
            let serial = rng.next_usize(90) + 10;
            format!("{left}-{serial}")
        }
    }
}

fn visible_traits(species: Species, rng: &mut StableRng) -> VisibleTraits {
    let eyes = match species {
        Species::Fuzz => pick(rng, &["o o", "^ ^", "* *", "u u"]),
        Species::Blob => pick(rng, &["o o", ". .", "O O", "~ ~"]),
        Species::Ghost => pick(rng, &["o o", ". .", "* *", "v v"]),
        Species::Glitch => pick(rng, &["0 0", "x x", "# #", "1 1"]),
        Species::Crystal => pick(rng, &["o o", "< >", "* *", "v v"]),
        Species::Mech => pick(rng, &["o o", "[ ]", "= =", "0 0"]),
    };
    let mouth = match species {
        Species::Fuzz | Species::Blob => pick(rng, &["w", "u", "v", "3"]),
        Species::Ghost => pick(rng, &["o", "~", "_", "."]),
        Species::Glitch => pick(rng, &["_", "!", "/", "\\"]),
        Species::Crystal => pick(rng, &["v", "^", "_", "."]),
        Species::Mech => pick(rng, &["-", "_", "=", "v"]),
    };
    // 3-char patterns match the `{pattern}` slot width in pet.jsx templates.
    let pattern = pick(
        rng,
        &[
            "   ",
            " . ",
            " * ",
            " \u{25c7} ",
            " + ",
            " ~ ",
            "...",
            " v ",
            " \u{2027} ",
            " \u{2726} ",
            "\u{b7}.\u{b7}",
            " \u{25d8} ",
            "\u{2021} \u{2021}",
            " \u{274d} ",
            " \u{25e2} ",
            "\u{2039}\u{b7}\u{203a}",
        ],
    )
    .to_string();
    let accent = pick(
        rng,
        &[
            "\u{b7}", ":", "\u{2726}", "*", "+", "\u{25cb}", "\u{25c7}", "\u{2665}", "\u{273f}",
            "\u{25aa}", "\u{25e6}", "\u{2727}", "\u{25a0}", "\u{25c8}", "\u{274d}",
        ],
    )
    .to_string();
    let palette_index = rng.next_usize(8);
    let morph_index = rng.next_usize(4);
    let morph_pup_index = rng.next_usize(4);
    let seed_hue = rng.next_usize(360) as u16;
    let saturation_percent = 82 + rng.next_usize(19) as u8;

    VisibleTraits {
        eyes: eyes.to_string(),
        mouth: mouth.to_string(),
        pattern,
        accent,
        palette_index,
        morph_index,
        morph_pup_index,
        seed_hue,
        saturation_percent,
    }
}

fn animation_phase(rng: &mut StableRng) -> AnimationPhase {
    let breath = rng.next_usize(64) as u8;
    let mut blink = rng.next_usize(64) as u8;
    if blink == breath {
        blink = (blink + 17) % 64;
    }
    AnimationPhase {
        breath,
        blink,
        flavor: rng.next_usize(64) as u8,
    }
}

fn join_parts(rng: &mut StableRng, parts: &[&'static str], count: usize) -> String {
    (0..count)
        .map(|_| pick(rng, parts))
        .collect::<Vec<_>>()
        .join("")
}

fn pick<'a>(rng: &mut StableRng, values: &'a [&'static str]) -> &'a str {
    values[rng.next_usize(values.len())]
}

pub(crate) fn fnv1a64(input: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Clone)]
pub(crate) struct StableRng {
    state: u64,
}

impl StableRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub(crate) fn next_usize(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() as usize) % upper
    }

    pub(crate) fn next_f32_unit(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / (1u32 << 24) as f32
    }

    pub(crate) fn next_bias(&mut self, p: f32) -> bool {
        self.next_f32_unit() < p
    }

    pub(crate) fn next_signed_unit(&mut self) -> f32 {
        self.next_f32_unit() * 2.0 - 1.0
    }
}

#[cfg(test)]
mod stable_rng_tests {
    use super::*;
    #[test]
    fn next_f32_unit_in_range() {
        let mut rng = StableRng::new(7);
        for _ in 0..10_000 {
            let f = rng.next_f32_unit();
            assert!((0.0..1.0).contains(&f), "out of bounds: {f}");
        }
    }
    #[test]
    fn next_bias_obeys_distribution() {
        let mut rng = StableRng::new(13);
        let n = 10_000;
        let hits = (0..n).filter(|_| rng.next_bias(0.30)).count() as f32 / n as f32;
        assert!((0.27..0.33).contains(&hits), "p=0.30 should land near 0.30, got {hits}");
    }
    #[test]
    fn next_signed_unit_in_signed_range() {
        let mut rng = StableRng::new(21);
        for _ in 0..1_000 {
            let v = rng.next_signed_unit();
            assert!((-1.0..=1.0).contains(&v));
        }
    }
}
