// pet.jsx — species + seeded generation system.
//
// Each species has a body template (array of equal-width lines) with slot
// markers that get filled per-pet:
//   "EEE" → eyes (3 chars)
//   "PPP" → body pattern (3 chars)
// The seed deterministically picks species, eyes palette, pattern, hue, and
// accent, so the same seed string always grows the same pet.

// Each species has multiple morph variants. The seed picks species + morph
// index, so two ghost pets can have different silhouettes. Templates use
// slots: EEE (3-char eyes), PPP (3-char pattern), M (1-char mouth),
// A (1-char body accent).
const SPECIES = {
  fuzz: {
    label: "fuzz",
    blurb: "fluffy, eared, whiskered",
    pup: [[
      "           ",
      "           ",
      "           ",
      "   /\\_/\\   ",
      "  ( EEE )  ",
      "    .M.    ",
      "   /PPP\\   ",
      "   d A b   ",
    ]],
    adult: [[
      "           ",
      "   /\\_/\\   ",
      "  ( EEE )  ",
      " ='  M  '= ",
      "  / PPP \\  ",
      " (   A   ) ",
      "  \\_____/  ",
      "   d   b   ",
    ], [
      "  /\\---/\\  ",
      " /  EEE  \\ ",
      " \\   M   / ",
      "  / PPP \\  ",
      " (   A   ) ",
      " /       \\ ",
      " \\_______/ ",
      "   ʼ   ʼ   ",
    ], [
      "           ",
      "           ",
      "  /\\   /\\  ",
      " |  EEE  | ",
      "  \\_PPP_/  ",
      "    M M    ",
      "  __ A __  ",
      "  d/   \\b  ",
    ]],
  },
  blob: {
    label: "blob",
    blurb: "round, gooey, dripping",
    pup: [[
      "           ",
      "           ",
      "           ",
      "   .---.   ",
      "  ( EEE )  ",
      "  ( PPP )  ",
      "   '.M.'   ",
      "   o A o   ",
    ]],
    adult: [[
      "           ",
      "   .---.   ",
      "  / EEE \\  ",
      " ( PPP   ) ",
      " (   M   ) ",
      "  '.___.'  ",
      "  . . A .  ",
      "   ° o °   ",
    ], [
      "           ",
      "  ._____.  ",
      " ( EEE   ) ",
      " ( . M . ) ",
      " (-_____-) ",
      " ( PPP   ) ",
      "  '. A .'  ",
      "   ° ° °   ",
    ], [
      "     o     ",
      "    o.o    ",
      "   .---.   ",
      "  ( EEE )  ",
      "  (  M  )  ",
      "  ( PPP )  ",
      "   '___'   ",
      "   o A o   ",
    ]],
  },
  ghost: {
    label: "ghost",
    blurb: "wispy, floating, hollow",
    pup: [[
      "           ",
      "           ",
      "    ___    ",
      "   /   \\   ",
      "  | EEE |  ",
      "  |  M  |  ",
      "   \\PPP/   ",
      "   ~ A ~   ",
    ]],
    adult: [[
      "    ___    ",
      "   /   \\   ",
      "  / EEE \\  ",
      " |  .M.  | ",
      " |  PPP  | ",
      "  \\  A  /  ",
      "   \\___/   ",
      "  ~ . ~ .  ",
    ], [
      "     _     ",
      "    / \\    ",
      "   /EEE\\   ",
      "  | .M. |  ",
      "  | PPP |  ",
      "  \\\\ A //  ",
      "   \\___/   ",
      "  ~ . . ~  ",
    ], [
      "    ___    ",
      "   /   \\   ",
      "  |     |  ",
      "  | EEE |  ",
      "  |  M  |  ",
      "   \\PPP/   ",
      "   ~ A ~   ",
      "    ~ ~    ",
    ]],
  },
  glitch: {
    label: "glitch",
    blurb: "corrupted, pixelated",
    pup: [[
      "           ",
      "           ",
      "   ▓░▒░▓   ",
      "  █▀▀▀▀▀█  ",
      "  █ EEE █  ",
      "  █▓ M ▓█  ",
      "  █ PPP █  ",
      "   ░ A ░   ",
    ]],
    adult: [[
      "   ▓░▒░▓   ",
      "  █▀▀▀▀▀█  ",
      "  █▒EEE▒█  ",
      "  ░ PPP ░  ",
      "  █▓ M ▓█  ",
      "  █▄▄▄▄▄█  ",
      "   ▒░A░▒   ",
      "  ░ ░ ░ ░  ",
    ], [
      "  ░░▓▒░▓░  ",
      "  █▀▀▀▀▀█  ",
      " ▓█ EEE █▓ ",
      " ▒█  M  █▒ ",
      " ░█ PPP █░ ",
      "  █▓░A░▓█  ",
      "  █▄▄▄▄▄█  ",
      "  ░ ░░░ ░  ",
    ], [
      "  ▓░▓░▓░▓  ",
      "  █▀▀▀▀▀█  ",
      "  █ EEE █  ",
      "  █  M  █  ",
      "  █ PPP █  ",
      "  █▄▄A▄▄█  ",
      "  ▓ ░ ▓ ░  ",
      "  ░ ▓ ░ ▓  ",
    ]],
  },
  crystal: {
    label: "crystal",
    blurb: "angular, faceted gem",
    pup: [[
      "           ",
      "           ",
      "    /\\     ",
      "   /  \\    ",
      "  / EEE \\  ",
      "  \\  M  /  ",
      "   \\PPP/   ",
      "    \\A/    ",
    ]],
    adult: [[
      "     ◆     ",
      "    /\\     ",
      "   /  \\    ",
      "  / EEE \\  ",
      " /  ◇M◇  \\ ",
      " \\  PPP  / ",
      "  \\  A  /  ",
      "   \\___/   ",
    ], [
      "    /\\     ",
      "   /\\/\\    ",
      "  / EEE \\  ",
      " /  ◇M◇  \\ ",
      " \\  PPP  / ",
      "  \\\\ A //  ",
      "   \\\\ //   ",
      "    \\/     ",
    ], [
      "   /\\ /\\   ",
      "  /  ◇  \\  ",
      " / EEE   \\ ",
      " \\  M    / ",
      "  \\PPP  /  ",
      "   \\.A./   ",
      "    \\◇/    ",
      "     v     ",
    ]],
  },
  mech: {
    label: "mech",
    blurb: "boxy, riveted, antenna",
    pup: [[
      "           ",
      "           ",
      "    _._    ",
      "   ┌───┐   ",
      "   │EEE│   ",
      "   │.M.│   ",
      "   │PPP│   ",
      "   └─A─┘   ",
    ]],
    adult: [[
      "    /·\\    ",
      "   ┌───┐   ",
      "  ┌┘EEE└┐  ",
      "  │ .M. │  ",
      "  │ PPP │  ",
      "  └──A──┘  ",
      "  /‗‗‗‗‗\\  ",
      "  ‾‾   ‾‾  ",
    ], [
      "  | /·\\ |  ",
      "  ┌─────┐  ",
      "  │ EEE │  ",
      " -│ .M. │- ",
      " -│ PPP │- ",
      "  └──A──┘  ",
      "  ┌──┴──┐  ",
      "  ‾‾   ‾‾  ",
    ], [
      "   _.A._   ",
      "  ┌─────┐  ",
      "  │ EEE │  ",
      "  │  M  │  ",
      "  │ PPP │  ",
      "  └┬───┬┘  ",
      "  ╔═════╗  ",
      "  ╚═════╝  ",
    ]],
  },
};

const SPECIES_LIST = Object.keys(SPECIES);

// ── Per-species 7-stage evolution arcs ────────────────────────────────────
// Index 0..6. Robots don't hatch from generic eggs — each species has its
// own early-stage forms.
const SPECIES_ARCS = {
  fuzz:    ["fluff",   "fuzzling", "kit",     "pup",        "fuzz",    "archfuzz", "mythic-fuzz"],
  blob:    ["droplet", "blip",     "globule", "wee-blob",   "blob",    "mega-blob","primordial"],
  ghost:   ["whisper", "wisp",     "shade",   "phantom-pup","ghost",   "wraith",   "revenant"],
  glitch:  ["bit",     "byte",     "packet",  "thread",     "glitch",  "daemon",   "kernel"],
  crystal: ["grain",   "shard",    "facet",   "cluster",    "crystal", "spire",    "lodestar"],
  mech:    ["chip",    "bolt",     "rivet",   "drone",      "mech",    "warmech",  "titan"],
};

// Tiny templates for stages 0,1,2 — species-specific so the early arc looks
// distinct. Stages 3 (pup), 4-5 (adult morphs), 6 (sage) reuse main templates.
const SPECIES_TINY = {
  fuzz: [
    ["           ","           ","           ","           ","           ","           ","    ʚʚʚ    ","    ___    "],
    ["           ","           ","           ","           ","    ___    ","   /EEE\\   ","   \\___/   ","    ʼ ʼ    "],
    ["           ","           ","           ","   /\\_/\\   ","  ( EEE )  ","    .M.    ","   /PPP\\   ","    | |    "],
  ],
  blob: [
    ["           ","           ","           ","           ","           ","     o     ","    ° °    ","     .     "],
    ["           ","           ","           ","           ","   .---.   ","  ( EEE )  ","   '._.'   ","    ° °    "],
    ["           ","           ","           ","   .---.   ","  ( EEE )  ","   . M .   ","  ( PPP )  ","   ° o °   "],
  ],
  ghost: [
    ["           ","           ","           ","           ","    . .    ","     '     ","    . .    ","           "],
    ["           ","           ","           ","    ___    ","   /   \\   ","  | EEE |  ","   \\_M_/   ","    ~ ~    "],
    ["           ","           ","           ","    ___    ","   /   \\   ","  | EEE |  ","  | PPP |  ","   ~ M ~   "],
  ],
  glitch: [
    ["           ","           ","           ","           ","           ","     ▓     ","     █     ","     ░     "],
    ["           ","           ","           ","           ","    ▓░▓    ","    █▀█    ","    █▄█    ","    ░ ░    "],
    ["           ","           ","           ","   ▓░▒░▓   ","   █▀▀▀█   ","   █EEE█   ","   █▄M▄█   ","   ░PPP░   "],
  ],
  crystal: [
    ["           ","           ","           ","           ","           ","     ·     ","    ◆◇◆    ","     ·     "],
    ["           ","           ","           ","           ","    /\\     ","   /  \\    ","   \\◇◇/    ","    \\/     "],
    ["           ","           ","           ","    /\\     ","   /  \\    ","  /EEE \\   ","  \\.M./    ","   \\PPP/   "],
  ],
  mech: [
    ["           ","           ","           ","           ","           ","    ┌─┐    ","    │·│    ","    └─┘    "],
    ["           ","           ","           ","           ","    _._    ","   ┌───┐   ","   │o o│   ","   └───┘   "],
    ["           ","           ","           ","    _._    ","   ┌───┐   ","   │EEE│   ","   │PPP│   ","   └─M─┘   "],
  ],
};

// ── Slot palettes ─────────────────────────────────────────────────────────
// Eyes are 3 chars wide. Mood-driven entries override seeded ones; default
// is the seeded variant. All mouths are baked into species templates.
const EYES_BY_MOOD = {
  happy:    "^.^",
  ecstatic: "*o*",
  hungry:   "u.u",
  sad:      "T.T",
  sleepy:   "-.-",
  wilted:   ",_,",
};
const MOUTHS_BY_MOOD = {
  happy:    "ω",
  ecstatic: "▽",
  hungry:   "o",
  sad:      "︵",
  sleepy:   "-",
  wilted:   "_",
};

const EYES_SEEDED = [
  "o.o", "0.0", "•.•", "@.@", "ø.ø", "·.·", "O.O", "◉.◉",
  "ʘ.ʘ", "◐.◑", "ó.ò", "˘.˘", "▼.▼", "*.*", "ò.ó", "+.+",
];
const PATTERNS_SEEDED = [
  "   ", " . ", " * ", " ◇ ", " + ", " ~ ", "...", " v ", " ‧ ",
  " ✦ ", "·.·", " ◘ ", "‡ ‡", " ❍ ", " ◢ ", "‹·›",
];
const MOUTHS_SEEDED = [
  "v", "_", "o", "ω", "u", "⌣", "︵", "ᴗ", "-", "0",
  "x", "*", "‿", "◡", "○", "ᗢ",
];
const ACCENTS_SEEDED = [
  "·", ":", "✦", "*", "+", "○", "◇", "♥", "✿", "▪",
  "◦", "✧", "■", "◈", "❍",
];

// ── Hash + RNG ─────────────────────────────────────────────────────────────
function hashSeed(str) {
  let h = 2166136261 >>> 0;
  const s = String(str || "anon");
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}
function mulberry32(a) {
  return function () {
    a = (a + 0x6d2b79f5) | 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const NAME_PARTS_A = ["mo", "pip", "lu", "kib", "noo", "fen", "ori", "tam", "vex", "zin", "rok", "cal", "ari", "puff", "biz"];
const NAME_PARTS_B = ["chi", "py", "na", "bit", "dle", "ka", "go", "mi", "lo", "x", "by", "tz", "sh", "ru"];

function generatePet(seed) {
  const r = mulberry32(hashSeed(seed));
  const pick = (arr) => arr[Math.floor(r() * arr.length)];
  const species = pick(SPECIES_LIST);
  const eyes = pick(EYES_SEEDED);
  const pattern = pick(PATTERNS_SEEDED);
  const mouth = pick(MOUTHS_SEEDED);
  const accent = pick(ACCENTS_SEEDED);
  const sp = SPECIES[species];
  const morph = Math.floor(r() * sp.adult.length);
  const morphPup = Math.floor(r() * sp.pup.length);
  const hue = Math.floor(r() * 360);
  const sat = 0.5 + r() * 0.9;
  const name = pick(NAME_PARTS_A) + pick(NAME_PARTS_B);
  return {
    seed: String(seed),
    species, eyes, pattern, mouth, accent,
    morph, morphPup,
    hue, sat, name,
  };
}

// ── Render ────────────────────────────────────────────────────────────────
function fillTemplate(lines, pet, eyesOverride, mouthOverride) {
  const eyes = eyesOverride || pet.eyes;
  const mouth = mouthOverride || pet.mouth || "v";
  return lines.map((l) =>
    l
      .replace("EEE", eyes)
      .replace("PPP", pet.pattern)
      .replace("M", mouth)
      .replace("A", pet.accent || "·")
  );
}

// Sage stage = adult + sparkle frame on top + bottom.
const SAGE_TOP = " *  .  *   ";
const SAGE_BOT = " ✦ ✧ ✦ ✧ ✦ ";

function petFrame(pet, stage) {
  const sp = SPECIES[pet.species] || SPECIES.fuzz;
  const idx = STAGES.indexOf(stage);
  // Stages 0,1,2 = species-specific tiny templates so a robot never hatches
  // from a generic egg.
  if (idx >= 0 && idx <= 2) {
    const tinySet = SPECIES_TINY[pet.species] || SPECIES_TINY.fuzz;
    return fillTemplate(tinySet[idx], pet);
  }
  if (idx === 3) {
    const m = (pet.morphPup || 0) % sp.pup.length;
    return fillTemplate(sp.pup[m], pet);
  }
  if (idx === 4) {
    return fillTemplate(sp.adult[0], pet);
  }
  if (idx === 5) {
    const m = (pet.morph || 0) % sp.adult.length;
    return fillTemplate(sp.adult[m], pet);
  }
  // s6 = sage: adult body framed by sparkle aura
  const m = (pet.morph || 0) % sp.adult.length;
  const body = fillTemplate(sp.adult[m], pet);
  return [SAGE_TOP, ...body.slice(1, -1), SAGE_BOT];
}

// Mood-aware frame: swaps eyes + mouth for mood-appropriate set.
function moodFrame(pet, stage, mood) {
  const eyes = EYES_BY_MOOD[mood] || pet.eyes;
  const mouth = MOUTHS_BY_MOOD[mood] || pet.mouth;
  return petFrame({ ...pet, eyes, mouth }, stage);
}

// ── Stage thresholds (auto-calibrated by personal token pace) ─────────────
// 7 stages: s0 (newborn) → s6 (mythic). Each stage is gated by lifetime
// tokens, scaled to the user's daily pace so a 500M/day user evolves on the
// same wall-clock arc as a 50k/day user.
const STAGES = ["s0", "s1", "s2", "s3", "s4", "s5", "s6"];
const DEFAULT_DAILY_PACE = 100_000;
// Days of activity at pace required to reach each stage.
const STAGE_DAYS = [0, 0.04, 0.25, 1, 4, 14, 60];
function stageFromTokens(lifetime, pace = DEFAULT_DAILY_PACE) {
  let idx = 0;
  for (let i = 1; i < STAGE_DAYS.length; i++) {
    if (lifetime >= STAGE_DAYS[i] * pace) idx = i;
  }
  return STAGES[idx];
}
function nextStageInfo(lifetime, pace = DEFAULT_DAILY_PACE) {
  const cur = stageFromTokens(lifetime, pace);
  const idx = STAGES.indexOf(cur);
  if (idx === STAGES.length - 1) return { next: null, threshold: null };
  return { next: STAGES[idx + 1], threshold: STAGE_DAYS[idx + 1] * pace };
}
// Compatibility shim: legacy code referenced STAGE_THRESHOLDS as an object.
const STAGE_THRESHOLDS = STAGES.reduce((acc, s, i) => {
  acc[s] = STAGE_DAYS[i] * DEFAULT_DAILY_PACE;
  return acc;
}, {});

// Species-aware stage label — "chip", "daemon", "mythic-fuzz" etc.
function stageLabel(pet, stage) {
  const arc = SPECIES_ARCS[pet.species] || SPECIES_ARCS.fuzz;
  const idx = STAGES.indexOf(stage);
  return idx >= 0 ? arc[idx] : stage;
}

// ── Components ────────────────────────────────────────────────────────────
function SpeechBubble({ text }) {
  if (!text) return null;
  const w = Math.max(text.length + 2, 8);
  const top = "╭" + "─".repeat(w) + "╮";
  const mid = "│ " + text + " ".repeat(w - text.length - 1) + "│";
  const bot = "╰" + "─".repeat(w - 4) + "┬───╯";
  const tail = " ".repeat(w - 4) + "╲";
  return (
    <pre className="bubble">
      {top + "\n" + mid + "\n" + bot + "\n" + tail}
    </pre>
  );
}

function petStyle(pet) {
  // Per-pet color derived from hue + saturation, applied as the foreground
  // for the ASCII frame. Falls back to inherited --fg if no pet.
  if (!pet) return undefined;
  const { hue, sat } = pet;
  return {
    color: `oklch(0.84 ${(0.10 * sat).toFixed(3)} ${hue})`,
    // Subtle drop-shadow in same hue gives the pet a "lit" feel without
    // a heavy glow that would muddy the terminal.
    textShadow: `0 0 8px oklch(0.78 ${(0.14 * sat).toFixed(3)} ${hue} / 0.35)`,
  };
}

// ── Per-role colors ───────────────────────────────────────────────
// Each glyph type pulls a different hue offset so two same-species pets read
// distinctly. Body stays at base hue; eyes/mouth/accent/pattern shift around
// the wheel by fixed degrees.
const ROLE_HUE_OFFSET = { eye: 180, mouth: 30, accent: 90, pattern: 150 };
function roleColor(pet, role) {
  if (!pet || role === "body") return null;
  const h = (pet.hue + (ROLE_HUE_OFFSET[role] || 0)) % 360;
  const s = pet.sat || 1;
  // Eyes: bright + saturated. Pattern: slightly muted. Mouth/accent: between.
  // Tuned: eye lightness/chroma now match body so the palette reads as a
  // family, not a neon ad. Hue shift alone supplies the variation.
  const lightness =
    role === "eye"     ? 0.84 :
    role === "pattern" ? 0.76 :
    role === "accent"  ? 0.82 :
                         0.84;
  const chroma =
    role === "eye"     ? (0.13 * s) :
    role === "pattern" ? (0.06 * s) :
    role === "accent"  ? (0.11 * s) :
                         (0.10 * s);
  return `oklch(${lightness} ${chroma.toFixed(3)} ${h})`;
}

// Raw template lines for a stage — placeholders intact (EEE / PPP / M / A).
function petTemplate(pet, stage) {
  const sp = SPECIES[pet.species] || SPECIES.fuzz;
  const idx = STAGES.indexOf(stage);
  if (idx >= 0 && idx <= 2) {
    const tinySet = SPECIES_TINY[pet.species] || SPECIES_TINY.fuzz;
    return tinySet[idx];
  }
  if (idx === 3) {
    const m = (pet.morphPup || 0) % sp.pup.length;
    return sp.pup[m];
  }
  if (idx === 4) return sp.adult[0];
  if (idx === 5) {
    const m = (pet.morph || 0) % sp.adult.length;
    return sp.adult[m];
  }
  const m = (pet.morph || 0) % sp.adult.length;
  return [SAGE_TOP, ...sp.adult[m].slice(1, -1), SAGE_BOT];
}

// Tokenize a template into colored segments. Returns array (per line) of
// {text, role} runs so the renderer can wrap each role in its own span.
function tokenizeTemplate(rawLines, pet, eyesOverride, mouthOverride) {
  const eyes = eyesOverride || pet.eyes;
  const mouth = mouthOverride || pet.mouth || "v";
  const pattern = pet.pattern || "···";
  const accent = pet.accent || "·";
  return rawLines.map((line) => {
    const out = [];
    const push = (text, role) => {
      const last = out[out.length - 1];
      if (last && last.role === role) last.text += text;
      else out.push({ text, role });
    };
    let i = 0;
    while (i < line.length) {
      if (line.substr(i, 3) === "EEE") { push(eyes, "eye"); i += 3; }
      else if (line.substr(i, 3) === "PPP") { push(pattern, "pattern"); i += 3; }
      else if (line[i] === "M") { push(mouth, "mouth"); i += 1; }
      else if (line[i] === "A") { push(accent, "accent"); i += 1; }
      else { push(line[i], "body"); i += 1; }
    }
    return out;
  });
}

// Render tokenized lines as colored React spans.
function renderTokens(pet, tokenLines) {
  return tokenLines.map((segs, i) => (
    <React.Fragment key={i}>
      {segs.map((s, j) => {
        const c = roleColor(pet, s.role);
        return (
          <span key={j} className={`seg seg-${s.role}`} style={c ? { color: c } : undefined}>
            {s.text}
          </span>
        );
      })}
      {i < tokenLines.length - 1 ? "\n" : null}
    </React.Fragment>
  ));
}

// ── Per-species animation profiles ─────────────────────────────────
// Each channel (breath, blink, particle) runs on its OWN period and OWN seeded
// phase offset so they never align. Periods are coprime-ish to avoid lockstep.
// All values are in fast-ticks (~150-220ms each).
const SPECIES_ANIM = {
  fuzz:    { breathPeriod: 16, breathHold: 4, blinkAvg: 32, blinkJitter: 12, particles: "fuzz"   },
  blob:    { breathPeriod: 13, breathHold: 5, blinkAvg: 40, blinkJitter: 14, particles: "blob"   },
  ghost:   { breathPeriod: 11, breathHold: 3, blinkAvg: 50, blinkJitter: 18, particles: "ghost"  },
  glitch:  { breathPeriod:  9, breathHold: 2, blinkAvg: 24, blinkJitter:  8, particles: "glitch" },
  crystal: { breathPeriod: 19, breathHold: 6, blinkAvg: 60, blinkJitter: 22, particles: "crystal"},
  mech:    { breathPeriod: 17, breathHold: 4, blinkAvg: 22, blinkJitter:  6, particles: "mech"   },
};

// Closed-eyes glyph for blink — 3 chars wide, species-flavored.
const BLINK_EYES = {
  fuzz:    "- -",
  blob:    "- -",
  ghost:   "— —",
  glitch:  "▒▒▒",
  crystal: "◇ ◇",
  mech:    "= =",
};

// Mood overrides blink — ecstatic/asleep/sick keep their distinctive eye glyph.
const BLINK_BLOCKED_MOODS = new Set(["ecstatic", "asleep", "sick", "sad", "wilted"]);

// Optional 1-char glitch corruption — rare, for glitch species only.
const GLITCH_NOISE = "▒░▓▀▄▌▐";
function glitchCorrupt(lines, tick) {
  if (tick % 37 !== 0) return lines;
  const row = (tick * 7) % lines.length;
  const line = lines[row];
  if (!line || line.trim() === "") return lines;
  const col = (tick * 11) % line.length;
  const ch = line[col];
  if (ch === " ") return lines;
  const noise = GLITCH_NOISE[(tick * 3) % GLITCH_NOISE.length];
  const out = lines.slice();
  out[row] = line.slice(0, col) + noise + line.slice(col + 1);
  return out;
}

function Pet({ pet, stage, mood, bubble, speed = 1500 }) {
  // Fast tick drives all sub-animations; speed prop scales it.
  const fastMs = Math.max(120, Math.round(speed / 7));
  const [tick, setTick] = React.useState(0);
  React.useEffect(() => {
    const id = setInterval(() => setTick((t) => (t + 1) % 100000), fastMs);
    return () => clearInterval(id);
  }, [fastMs]);

  const anim = SPECIES_ANIM[pet.species] || SPECIES_ANIM.fuzz;

  // Seeded phase offsets so each pet's rhythm is unique — a fuzz named "mochi"
  // breathes on a different beat than a fuzz named "pixel".
  const seedH = React.useMemo(
    () => hashSeed((pet.name || "x") + ":" + pet.species),
    [pet.name, pet.species]
  );
  const breathOffset = seedH % anim.breathPeriod;
  const blinkSeedOffset = (seedH * 13) % anim.blinkAvg;

  // Breath: long hold + slow swap. Wobble holds for breathHold ticks then
  // releases for breathHold ticks; the rest of the cycle it sits at rest.
  const breathPhase = (tick + breathOffset) % anim.breathPeriod;
  const wobble = breathPhase < anim.breathHold ? 1 : 0;

  // Blink: schedule next blink with random jitter so consecutive blinks land
  // on irregular intervals. Lives across renders via ref; one-tick window.
  const nextBlinkRef = React.useRef(null);
  const lastSeedRef = React.useRef(seedH);
  if (nextBlinkRef.current === null || lastSeedRef.current !== seedH) {
    lastSeedRef.current = seedH;
    nextBlinkRef.current = tick + blinkSeedOffset;
  }
  // Mood-transition blink suppression: when mood changes, skip blink for a
  // few ticks so we don't flash a blink frame mid-mood-swap.
  const lastMoodRef = React.useRef(mood);
  const moodChangedAtRef = React.useRef(0);
  if (lastMoodRef.current !== mood) {
    lastMoodRef.current = mood;
    moodChangedAtRef.current = tick;
  }
  const moodSettled = tick - moodChangedAtRef.current > 4;

  const blinking =
    moodSettled &&
    !BLINK_BLOCKED_MOODS.has(mood) &&
    tick === nextBlinkRef.current;
  React.useEffect(() => {
    if (!blinking) return;
    const jitter = Math.floor((Math.random() - 0.5) * anim.blinkJitter * 2);
    nextBlinkRef.current = tick + anim.blinkAvg + jitter;
  }, [blinking, tick, anim.blinkAvg, anim.blinkJitter]);

  const eyesOverride = blinking ? (BLINK_EYES[pet.species] || "- -") : null;

  // Build colored token stream for the current frame. Apply blink override on
  // top of mood-derived eyes/mouth.
  const moodEyes = EYES_BY_MOOD[mood];
  const moodMouth = MOUTHS_BY_MOOD[mood];
  const finalEyes = eyesOverride || moodEyes || pet.eyes;
  const finalMouth = moodMouth || pet.mouth || "v";
  const tmpl = petTemplate(pet, stage);
  const tokens = tokenizeTemplate(tmpl, pet, finalEyes, finalMouth);

  // Glitch species: occasional rare character corruption applied at the body
  // segment level (post-tokenization). Cheaper than re-tokenizing each tick.
  const corruptedTokens =
    pet.species === "glitch" ? glitchCorruptTokens(tokens, tick) : tokens;

  return (
    <div className={`pet pet-${pet.species} stage-${stage} mood-${mood}`}>
      <SpeechBubble text={bubble} />
      <div className="pet-aura" aria-hidden="true">
        <ParticleLayer kind={anim.particles} pet={pet} stage={stage} tick={tick} />
        <pre
          className="pet-frame"
          style={{
            ...petStyle(pet),
            transform: `translateY(${wobble}px)`,
            transition: "transform 0.4s ease-in-out",
          }}
        >
          {renderTokens(pet, corruptedTokens)}
        </pre>
      </div>
    </div>
  );
}

// Glitch corruption operating on tokenized lines: pick a body segment, swap
// one char with a noise glyph. Rare cadence so it reads as a real artifact.
function glitchCorruptTokens(tokenLines, tick) {
  if (tick % 37 !== 0) return tokenLines;
  const row = (tick * 7) % tokenLines.length;
  const segs = tokenLines[row];
  if (!segs || !segs.length) return tokenLines;
  // Find a body segment with a non-space char.
  for (let s = 0; s < segs.length; s++) {
    if (segs[s].role !== "body") continue;
    const text = segs[s].text;
    const col = (tick * 11) % text.length;
    if (text[col] === " ") continue;
    const noise = GLITCH_NOISE[(tick * 3) % GLITCH_NOISE.length];
    const out = tokenLines.slice();
    const newSegs = segs.slice();
    newSegs[s] = { ...segs[s], text: text.slice(0, col) + noise + text.slice(col + 1) };
    out[row] = newSegs;
    return out;
  }
  return tokenLines;
}

// ── Per-species particle/aura layer ─────────────────────────────────
// Static span layout, animated entirely in CSS so React isn't re-rendering
// every particle. The class hooks live in tokenpet.html.
function ParticleLayer({ kind, pet, stage, tick }) {
  // Suppress particles in earliest stages — newborns shouldn't have full auras.
  const idx = STAGES.indexOf(stage);
  if (idx < 2) return null;

  if (kind === "fuzz") {
    // Tail flick — occasional swish below the pet.
    const swish = tick % 23 < 3;
    return (
      <div className="pl pl-fuzz">
        <span className={`pl-tail ${swish ? "swish" : ""}`}>{swish ? "~" : " "}</span>
      </div>
    );
  }
  if (kind === "blob") {
    return (
      <div className="pl pl-blob">
        <span className="pl-drop pl-drop-1">.</span>
        <span className="pl-drop pl-drop-2">°</span>
        <span className="pl-drop pl-drop-3">.</span>
      </div>
    );
  }
  if (kind === "ghost") {
    return (
      <div className="pl pl-ghost">
        <span className="pl-wisp pl-wisp-1">~</span>
        <span className="pl-wisp pl-wisp-2">·</span>
        <span className="pl-wisp pl-wisp-3">'</span>
        <span className="pl-wisp pl-wisp-4">.</span>
      </div>
    );
  }
  if (kind === "glitch") {
    return (
      <div className="pl pl-glitch">
        <span className="pl-scan" />
        <span className="pl-noise pl-noise-1">▒</span>
        <span className="pl-noise pl-noise-2">░</span>
        <span className="pl-noise pl-noise-3">▓</span>
      </div>
    );
  }
  if (kind === "crystal") {
    return (
      <div className="pl pl-crystal">
        <span className="pl-spark pl-spark-1">✧</span>
        <span className="pl-spark pl-spark-2">✦</span>
        <span className="pl-spark pl-spark-3">✧</span>
        <span className="pl-spark pl-spark-4">·</span>
        <span className="pl-spark pl-spark-5">✧</span>
      </div>
    );
  }
  if (kind === "mech") {
    const led = tick % 4 < 2;
    return (
      <div className="pl pl-mech">
        <span className={`pl-led ${led ? "on" : "off"}`}>●</span>
        <span className="pl-steam pl-steam-1">~</span>
        <span className="pl-steam pl-steam-2">°</span>
      </div>
    );
  }
  return null;
}

// Mini pet — used in the gallery row.
function MiniPet({ pet, stage = "adult" }) {
  const lines = petFrame(pet, stage);
  return (
    <div className="mini-pet">
      <pre className="mini-frame" style={petStyle(pet)} aria-hidden="true">
        {lines.join("\n")}
      </pre>
      <div className="mini-meta">
        <span className="mini-name">{pet.name}</span>
        <span className="mini-species">{SPECIES[pet.species].label}</span>
      </div>
    </div>
  );
}

Object.assign(window, {
  SPECIES, SPECIES_LIST, EYES_SEEDED, PATTERNS_SEEDED,
  STAGES, STAGE_THRESHOLDS, stageFromTokens, nextStageInfo,
  generatePet, hashSeed, petFrame, moodFrame, stageLabel,
  Pet, MiniPet,
});
