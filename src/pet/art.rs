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
            "lint seed",
            "whisk sprout",
            "pawling",
            "tuftkin",
            "ruff",
            "mantlefur",
            "elder fuzz",
        ],
        Species::Blob => [
            "dew bead",
            "puddle bud",
            "globlet",
            "squishling",
            "ooze chum",
            "moss blob",
            "elder ooze",
        ],
        Species::Ghost => [
            "thin veil",
            "candle wisp",
            "small haunt",
            "driftling",
            "veilshade",
            "moon phant",
            "old echo",
        ],
        Species::Glitch => [
            "bit spark",
            "pixel split",
            "jitling",
            "scan stray",
            "fault bloom",
            "packet sage",
            "root anomaly",
        ],
        Species::Crystal => [
            "mica chip",
            "facet bud",
            "shardling",
            "prism kin",
            "geode heart",
            "lumen spire",
            "old facet",
        ],
        Species::Mech => [
            "loose bolt",
            "gear bud",
            "servo tot",
            "tin rover",
            "brass walker",
            "relay knight",
            "old engine",
        ],
    };
    labels[stage_key(stage).index()]
}

pub fn morph_count(species: Species, stage: Stage) -> usize {
    templates(species, stage_key(stage)).len()
}

pub(crate) fn template_for(
    species: Species,
    stage: StageKey,
    morph_index: usize,
    compact: bool,
) -> &'static [&'static str] {
    let templates = templates(species, stage);
    let selected = templates[morph_index % templates.len()];
    if compact {
        selected.compact
    } else {
        selected.full
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

#[derive(Clone, Copy)]
struct TemplateSet {
    full: &'static [&'static str],
    compact: &'static [&'static str],
}

fn templates(species: Species, stage: StageKey) -> &'static [TemplateSet] {
    match species {
        Species::Fuzz => fuzz_templates(stage),
        Species::Blob => blob_templates(stage),
        Species::Ghost => ghost_templates(stage),
        Species::Glitch => glitch_templates(stage),
        Species::Crystal => crystal_templates(stage),
        Species::Mech => mech_templates(stage),
    }
}

macro_rules! set {
    ($full:expr, $compact:expr) => {
        TemplateSet {
            full: $full,
            compact: $compact,
        }
    };
}

const FUZZ_S0: &[TemplateSet] = &[set!(
    &["  /\\_/\\", " ( {eyes} )", "  \\_{mouth}_/"],
    &["/\\_/\\", "({eyes})", "\\_{mouth}_/"]
)];
const FUZZ_S1: &[TemplateSet] = &[
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            " /| {mouth} |\\",
            "   {pattern}{accent}"
        ],
        &["/\\_/\\", "({eyes})", "|{mouth}|"]
    ),
    set!(
        &["  (\\_/)", " ( {eyes} )", " /  {mouth} \\", "  '{pattern}'"],
        &["(\\_/)", "({eyes})", "/{mouth}\\"]
    ),
];
const FUZZ_ADULT: &[TemplateSet] = &[
    set!(
        &[
            " /\\   /\\",
            "(  {eyes}  )",
            " \\  {mouth}  /",
            " /|{pattern}{accent}{pattern}|\\",
            "  /   \\"
        ],
        &["/\\ /\\", "({eyes})", "\\ {mouth}/", "/| |\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            " <  {mouth}  >",
            " /{pattern}___{pattern}\\",
            "  tail {accent}"
        ],
        &["/\\_/\\", "({eyes})", "<{mouth}>", "/___\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ({accent}{eyes}{accent})",
            " /  {mouth}  \\",
            "(_{pattern}___{pattern}_)",
            "  paw paw"
        ],
        &["/\\_/\\", "({eyes})", "/ {mouth}\\", "(___)"]
    ),
];
const FUZZ_S2: &[TemplateSet] = &[
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            " /  {mouth}  \\",
            "  {pattern} paws"
        ],
        &["/\\_/\\", "({eyes})", "/{mouth}\\", "paws"]
    ),
    set!(
        &[
            "  (\\_/)",
            " ( {eyes} )",
            " / {mouth} \\",
            "  {accent} fluff"
        ],
        &["(\\_/)", "({eyes})", "/{mouth}\\", "fluf"]
    ),
    set!(
        &[
            "  /\\ /\\",
            " ( {eyes})",
            " / {mouth} \\",
            "  {pattern}{accent}"
        ],
        &["/\\ /\\", "({eyes})", "/{mouth}\\", "{pattern}{accent}"]
    ),
];
const FUZZ_S3: &[TemplateSet] = &[
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            "<   {mouth}  >",
            " /{pattern}___\\"
        ],
        &["/\\_/\\", "({eyes})", "<{mouth}>", "/___"]
    ),
    set!(
        &[
            " /\\   /\\",
            "( {eyes} )",
            " \\ {mouth} /",
            " /{accent}___{accent}\\"
        ],
        &["/\\ /\\", "({eyes})", "\\{mouth}/", "/__\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ({accent}{eyes}{accent})",
            " /  {mouth}  \\",
            "(_{pattern}_)"
        ],
        &["/\\_/\\", "({eyes})", "/{mouth}\\", "(_)"]
    ),
];
const FUZZ_S4: &[TemplateSet] = &[
    set!(
        &[
            " /\\   /\\",
            "(  {eyes}  )",
            " \\  {mouth}  /",
            " /|{pattern}_{accent}|\\",
            "  / \\"
        ],
        &["/\\ /\\", "({eyes})", "\\ {mouth}/", "/|\\ "]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            " <  {mouth}  >",
            " /{pattern}___{pattern}\\",
            "  tail"
        ],
        &["/\\_/\\", "({eyes})", "<{mouth}>", "/___\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ({accent}{eyes}{accent})",
            " /  {mouth}  \\",
            "(_{pattern}___{pattern}_)",
            "  paw"
        ],
        &["/\\_/\\", "({eyes})", "/ {mouth}\\", "(___)"]
    ),
];
const FUZZ_S5: &[TemplateSet] = &[
    set!(
        &[
            " /\\___/\\",
            "(  {eyes}  )",
            " \\  {mouth}  /",
            " /|{pattern}{accent}{pattern}|\\",
            " /_   _\\"
        ],
        &["/\\_/\\", "({eyes})", "\\ {mouth}/", "/|_|\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ( {eyes} )",
            "<<  {mouth} >>",
            " /{pattern}___{pattern}\\",
            " tail {accent}"
        ],
        &["/\\_/\\", "({eyes})", "<<{mouth}", "/___\\"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " ({accent}{eyes}{accent})",
            "/   {mouth}   \\",
            "(_{pattern}___{pattern}_)",
            " paw paw"
        ],
        &["/\\_/\\", "({eyes})", "/ {mouth}\\", "(___)"]
    ),
];
const FUZZ_S6: &[TemplateSet] = FUZZ_ADULT;

const BLOB_S0: &[TemplateSet] = &[set!(
    &["  .---.", " / {eyes} \\", " \\  {mouth} /", "  `---'"],
    &[".---.", "|{eyes}|", "`{mouth}-'"]
)];
const BLOB_S1: &[TemplateSet] = &[
    set!(
        &["  .-.", " ( {eyes} )", " (  {mouth})", "  `~'"],
        &[".-.", "({eyes})", "({mouth})"]
    ),
    set!(
        &[" .---.", "( {eyes} )", "( {mouth} {pattern})", " `..'"],
        &[".---.", "{eyes}", "({mouth})"]
    ),
];
const BLOB_ADULT: &[TemplateSet] = &[
    set!(
        &[
            "  .----.",
            " / {eyes}  \\",
            "|   {mouth}  |",
            "| {pattern}  {accent} |",
            " `-..-'"
        ],
        &[".----.", "|{eyes}|", "| {mouth} |", "`..-'"]
    ),
    set!(
        &[
            " .------.",
            "(  {eyes}  )",
            "(   {mouth}   )",
            " \\ {pattern}_{accent} /",
            "  `----'"
        ],
        &[".----.", "({eyes})", "( {mouth})", "`--'"]
    ),
    set!(
        &[
            "  .-~~-.",
            " / {eyes}  \\",
            "(  {mouth} {pattern} )",
            " \\__{accent}__/",
            "   drip"
        ],
        &[".-~~-.", "{eyes}", "({mouth})", " drip"]
    ),
];
const BLOB_S2: &[TemplateSet] = &[
    set!(
        &["  .---.", " / {eyes} \\", "(   {mouth})", " `.{pattern}.'"],
        &[".---.", "{eyes}", "({mouth})", "`.'"]
    ),
    set!(
        &[" .---.", "( {eyes} )", "( {mouth} {accent})", " `~'"],
        &[".---.", "{eyes}", "({mouth})", "`~'"]
    ),
    set!(
        &["  .-.", " ( {eyes})", " ( {mouth} )", "  drip"],
        &[".-.", "{eyes}", "{mouth}", "drip"]
    ),
];
const BLOB_S3: &[TemplateSet] = &[
    set!(
        &[
            "  .----.",
            " / {eyes}  \\",
            "|   {mouth} |",
            " `-{pattern}-'"
        ],
        &[".----.", "|{eyes}|", "|{mouth}|", "`-'"]
    ),
    set!(
        &[" .----.", "( {eyes}  )", "(  {mouth}  )", " \\ {accent} /"],
        &[".----", "({eyes})", "({mouth})", "\\_/"]
    ),
    set!(
        &[" .-~~-.", "/ {eyes} \\", "( {mouth}{pattern} )", " `~~'"],
        &[".-~~", "{eyes}", "{mouth}", "~~"]
    ),
];
const BLOB_S4: &[TemplateSet] = &[
    set!(
        &[
            "  .----.",
            " / {eyes}  \\",
            "|   {mouth}  |",
            "| {pattern}  {accent} |",
            " `-..-'"
        ],
        &[".----.", "|{eyes}|", "| {mouth} |", "`..-'"]
    ),
    set!(
        &[
            " .------.",
            "(  {eyes}  )",
            "(   {mouth}   )",
            " \\ {pattern}_{accent} /",
            "  `--'"
        ],
        &[".----.", "({eyes})", "( {mouth})", "`--'"]
    ),
    set!(
        &[
            "  .-~~-.",
            " / {eyes}  \\",
            "(  {mouth} {pattern} )",
            " \\__{accent}__/",
            "  drip"
        ],
        &[".-~~-.", "{eyes}", "({mouth})", "drip"]
    ),
];
const BLOB_S5: &[TemplateSet] = &[
    set!(
        &[
            " .------.",
            "/  {eyes}  \\",
            "|   {mouth}   |",
            "| {pattern}  {accent} |",
            " `-....-'"
        ],
        &[".----.", "|{eyes}|", "| {mouth}|", "`..-'"]
    ),
    set!(
        &[
            " .------.",
            "(  {eyes}  )",
            "(   {mouth}   )",
            " \\ {pattern}_{accent}_{pattern} /",
            "  `----'"
        ],
        &[".----.", "({eyes})", "({mouth})", "`--'"]
    ),
    set!(
        &[
            " .-~~~~-.",
            "/  {eyes}  \\",
            "(  {mouth} {pattern} )",
            " \\___{accent}___/",
            "   drip"
        ],
        &[".-~~-.", "{eyes}", "({mouth})", "drip"]
    ),
];
const BLOB_S6: &[TemplateSet] = BLOB_ADULT;

const GHOST_S0: &[TemplateSet] = &[set!(
    &["   .-.", "  ( {eyes})", "   \\{mouth}/", "    ~"],
    &[".-.", "{eyes}", "\\{mouth}/"]
)];
const GHOST_S1: &[TemplateSet] = &[
    set!(
        &["  .-.", " ( {eyes} )", "  \\ {mouth}/", "   {pattern}~"],
        &[".-.", "({eyes})", "\\{mouth}/"]
    ),
    set!(
        &["   .--.", "  ( {eyes})", "  / {mouth}/", "  ~~{accent}"],
        &[".--.", "{eyes}", "/{mouth}/"]
    ),
];
const GHOST_ADULT: &[TemplateSet] = &[
    set!(
        &[
            "   .----.",
            "  / {eyes} \\",
            " |   {mouth} |",
            " | {pattern}  {accent}|",
            "  \\_~~_/"
        ],
        &[".----.", "|{eyes}|", "| {mouth}|", "\\~~/"]
    ),
    set!(
        &[
            "  .-~~-.",
            " / {eyes}  \\",
            "|   {mouth}  |",
            " \\ {pattern}  /",
            "  `~~{accent}"
        ],
        &[".-~~-.", "{eyes}", "|{mouth}|", "`~~"]
    ),
    set!(
        &[
            "   .---.",
            "  ( {eyes} )",
            "  /  {mouth} \\",
            " /_{pattern}_{accent}_\\",
            "   wisp"
        ],
        &[".---.", "({eyes})", "/ {mouth}\\", "wisp"]
    ),
];
const GHOST_S2: &[TemplateSet] = &[
    set!(
        &["   .-.", "  ( {eyes})", "  / {mouth}\\", "   {pattern}~"],
        &[".-.", "{eyes}", "/{mouth}\\", "~"]
    ),
    set!(
        &["  .--.", " ( {eyes})", " / {mouth}/", "  ~{accent}"],
        &[".--.", "{eyes}", "/{mouth}", "~"]
    ),
    set!(
        &["  .-.", " ( {eyes} )", "  \\ {mouth}/", "  ~~"],
        &[".-.", "({eyes})", "\\{mouth}", "~~"]
    ),
];
const GHOST_S3: &[TemplateSet] = &[
    set!(
        &[
            "   .---.",
            "  / {eyes}\\",
            " |   {mouth}|",
            "  \\_{pattern}~/"
        ],
        &[".---.", "|{eyes}|", "|{mouth}|", "~~"]
    ),
    set!(
        &["  .-~~-.", " / {eyes} \\", "|  {mouth} |", " `~{accent}"],
        &[".-~~", "{eyes}", "|{mouth}|", "`~"]
    ),
    set!(
        &["   .--.", "  ( {eyes})", "  / {mouth}\\", " /_{pattern}_\\"],
        &[".--.", "{eyes}", "/{mouth}\\", "_"]
    ),
];
const GHOST_S4: &[TemplateSet] = GHOST_ADULT;
const GHOST_S5: &[TemplateSet] = &[
    set!(
        &[
            "   .----.",
            "  / {eyes} \\",
            " |   {mouth} |",
            " | {pattern}  {accent}|",
            "  \\_~~_/"
        ],
        &[".----.", "|{eyes}|", "| {mouth}|", "\\~~/"]
    ),
    set!(
        &[
            "  .-~~-.",
            " / {eyes}  \\",
            "|   {mouth}  |",
            " \\ {pattern} {accent}/",
            "  `~~~"
        ],
        &[".-~~-.", "{eyes}", "|{mouth}|", "`~~"]
    ),
    set!(
        &[
            "   .---.",
            "  ( {eyes} )",
            " /   {mouth} \\",
            "/__{pattern}_{accent}__\\",
            "  wisp"
        ],
        &[".---.", "({eyes})", "/ {mouth}\\", "wisp"]
    ),
];
const GHOST_S6: &[TemplateSet] = &[
    set!(
        &[
            "   .----.",
            "  / {eyes} \\",
            " |   {mouth} |",
            " | {pattern}  {accent}|",
            "  \\_~~_/",
            "   echo"
        ],
        &[".----.", "|{eyes}|", "| {mouth}|", "\\~~/"]
    ),
    set!(
        &[
            "  .-~~-.",
            " / {eyes}  \\",
            "|   {mouth}  |",
            " \\ {pattern}  /",
            "  `~~{accent}",
            " old"
        ],
        &[".-~~-.", "{eyes}", "|{mouth}|", "`~~"]
    ),
    set!(
        &[
            "   .---.",
            "  ( {eyes} )",
            "  /  {mouth} \\",
            " /_{pattern}_{accent}_\\",
            "   wisp",
            "   ~"
        ],
        &[".---.", "({eyes})", "/ {mouth}\\", "wisp"]
    ),
];

const GLITCH_S0: &[TemplateSet] = &[set!(
    &["  [#{accent}]", " <{eyes}>", "  [{mouth}]"],
    &["[#{accent}]", "<{eyes}>", "[{mouth}]"]
)];
const GLITCH_S1: &[TemplateSet] = &[
    set!(
        &["  []{}", " [{eyes}]", " /{mouth}{pattern}\\", "  err"],
        &["[]{}", "[{eyes}]", "{mouth}{pattern}"]
    ),
    set!(
        &["  <##>", " [{eyes}]", " |{mouth}{accent}|", "  01"],
        &["<##>", "[{eyes}]", "|{mouth}|"]
    ),
];
const GLITCH_ADULT: &[TemplateSet] = &[
    set!(
        &[
            " []==[]",
            "[ {eyes} ]",
            "|  {mouth}  |",
            "|{pattern}__{accent}|",
            " /_/\\_\\"
        ],
        &["[]=[]", "[{eyes}]", "|{mouth}|", "/_\\"]
    ),
    set!(
        &[
            " <//>",
            "[{accent}{eyes}{accent}]",
            "|  {mouth} |",
            "|_{pattern}_|",
            " 0101"
        ],
        &["<//>", "[{eyes}]", "|{mouth}|", "0101"]
    ),
    set!(
        &[
            " [####]",
            "< {eyes} >",
            "[  {mouth}  ]",
            "[{pattern}=={accent}]",
            "  //"
        ],
        &["[###]", "<{eyes}>", "[{mouth}]", "//"]
    ),
];
const GLITCH_S2: &[TemplateSet] = &[
    set!(
        &[" []{}", "[{eyes}]", "/{mouth}{pattern}\\", " err"],
        &["[]{}", "[{eyes}]", "{mouth}{pattern}", "err"]
    ),
    set!(
        &[" <##>", "[{eyes}]", "|{mouth}{accent}|", " 01"],
        &["<##>", "[{eyes}]", "|{mouth}|", "01"]
    ),
    set!(
        &[" [::]", "<{eyes}>", "[{mouth}]", " {pattern}"],
        &["[::]", "<{eyes}>", "[{mouth}]", "{pattern}"]
    ),
];
const GLITCH_S3: &[TemplateSet] = &[
    set!(
        &[" []==", "[ {eyes} ]", "| {mouth} |", "|{pattern}_{accent}|"],
        &["[]=", "[{eyes}]", "|{mouth}|", "|_|"]
    ),
    set!(
        &[" <//>", "[{eyes}]", "| {mouth}|", "|_{pattern}|"],
        &["<//>", "[{eyes}]", "|{mouth}|", "_"]
    ),
    set!(
        &[" [###]", "< {eyes}>", "[ {mouth} ]", "[{accent}]"],
        &["[###]", "<{eyes}>", "[{mouth}]", "{accent}"]
    ),
];
const GLITCH_S4: &[TemplateSet] = GLITCH_ADULT;
const GLITCH_S5: &[TemplateSet] = &[
    set!(
        &[
            " []==[]",
            "[ {eyes} ]",
            "|  {mouth}  |",
            "|{pattern}__{accent}|",
            " /_/\\_\\"
        ],
        &["[]=[]", "[{eyes}]", "|{mouth}|", "/_\\"]
    ),
    set!(
        &[
            " <///>",
            "[{accent}{eyes}{accent}]",
            "|  {mouth} |",
            "|_{pattern}_|",
            " 0101"
        ],
        &["<//>", "[{eyes}]", "|{mouth}|", "0101"]
    ),
    set!(
        &[
            " [#####]",
            "< {eyes} >",
            "[  {mouth}  ]",
            "[{pattern}=={accent}]",
            "  ///"
        ],
        &["[###]", "<{eyes}>", "[{mouth}]", "//"]
    ),
];
const GLITCH_S6: &[TemplateSet] = &[
    set!(
        &[
            " []==[]",
            "[ {eyes} ]",
            "|  {mouth}  |",
            "|{pattern}__{accent}|",
            " /_/\\_\\",
            " root"
        ],
        &["[]=[]", "[{eyes}]", "|{mouth}|", "/_\\"]
    ),
    set!(
        &[
            " <///>",
            "[{accent}{eyes}{accent}]",
            "|  {mouth} |",
            "|_{pattern}_|",
            " 0101",
            " sys"
        ],
        &["<//>", "[{eyes}]", "|{mouth}|", "0101"]
    ),
    set!(
        &[
            " [#####]",
            "< {eyes} >",
            "[  {mouth}  ]",
            "[{pattern}=={accent}]",
            "  ///",
            " 0x"
        ],
        &["[###]", "<{eyes}>", "[{mouth}]", "//"]
    ),
];

const CRYSTAL_S0: &[TemplateSet] = &[set!(
    &["   /\\", "  /{eyes}\\", "  \\{mouth}/", "   \\/"],
    &[" /\\", "/{eyes}\\", "\\{mouth}/"]
)];
const CRYSTAL_S1: &[TemplateSet] = &[
    set!(
        &["   /\\", "  /{eyes}\\", " / {mouth} \\", " \\_{pattern}/"],
        &[" /\\", "/{eyes}\\", "\\{mouth}/"]
    ),
    set!(
        &["  /\\", " /{accent}{eyes}\\", "<  {mouth}>", " \\/"],
        &["/\\", "{eyes}", "<{mouth}>"]
    ),
];
const CRYSTAL_ADULT: &[TemplateSet] = &[
    set!(
        &[
            "    /\\",
            "  /{accent}{eyes}\\",
            " /  {mouth}  \\",
            "/_{pattern}__{pattern}_\\",
            "  /__\\"
        ],
        &[" /\\", "/{eyes}\\", "|{mouth}|", "/__\\"]
    ),
    set!(
        &[
            "   /\\",
            "  <{eyes}>",
            " /  {mouth}\\",
            "<_{pattern}{accent}_>",
            "   \\/"
        ],
        &["/\\", "<{eyes}>", "/{mouth}\\", "<_>"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " / {eyes} \\",
            "<   {mouth}  >",
            " \\{pattern}_{accent}/",
            "  \\_/"
        ],
        &["/\\_/\\", "{eyes}", "<{mouth}>", "\\_/"]
    ),
];
const CRYSTAL_S2: &[TemplateSet] = &[
    set!(
        &["   /\\", "  /{eyes}\\", " / {mouth} \\", " \\_{pattern}/"],
        &[" /\\", "/{eyes}\\", "\\{mouth}/", "{pattern}"]
    ),
    set!(
        &["  /\\", " /{accent}{eyes}\\", "< {mouth}>", " \\/"],
        &["/\\", "{eyes}", "<{mouth}>", "\\/"]
    ),
    set!(
        &["  /\\", " <{eyes}>", " /{mouth}\\", " \\/"],
        &["/\\", "<{eyes}>", "{mouth}", "\\/"]
    ),
];
const CRYSTAL_S3: &[TemplateSet] = &[
    set!(
        &[
            "    /\\",
            "  /{eyes}\\",
            " /  {mouth}\\",
            "/_{pattern}_\\",
            "  \\/"
        ],
        &[" /\\", "/{eyes}\\", "|{mouth}|", "\\/"]
    ),
    set!(
        &[
            "   /\\",
            "  <{eyes}>",
            " / {mouth}\\",
            "<_{accent}_>",
            "  \\/"
        ],
        &["/\\", "<{eyes}>", "/{mouth}", "<_>"]
    ),
    set!(
        &["  /\\_/\\", " / {eyes}\\", "<  {mouth}>", " \\{pattern}/"],
        &["/\\_/\\", "{eyes}", "<{mouth}>", "\\/"]
    ),
];
const CRYSTAL_S4: &[TemplateSet] = CRYSTAL_ADULT;
const CRYSTAL_S5: &[TemplateSet] = &[
    set!(
        &[
            "    /\\",
            "  /{accent}{eyes}\\",
            " /   {mouth}  \\",
            "/_{pattern}__{pattern}_\\",
            "  /__\\"
        ],
        &[" /\\", "/{eyes}\\", "|{mouth}|", "/__\\"]
    ),
    set!(
        &[
            "   /\\",
            "  <{eyes}>",
            " /  {mouth}\\",
            "<_{pattern}{accent}_{pattern}>",
            "   \\/"
        ],
        &["/\\", "<{eyes}>", "/{mouth}\\", "<_>"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " / {eyes} \\",
            "<   {mouth}  >",
            " \\{pattern}_{accent}/",
            "  \\_/"
        ],
        &["/\\_/\\", "{eyes}", "<{mouth}>", "\\_/"]
    ),
];
const CRYSTAL_S6: &[TemplateSet] = &[
    set!(
        &[
            "    /\\",
            "  /{accent}{eyes}\\",
            " /  {mouth}  \\",
            "/_{pattern}__{pattern}_\\",
            "  /__\\",
            " facet"
        ],
        &[" /\\", "/{eyes}\\", "|{mouth}|", "/__\\"]
    ),
    set!(
        &[
            "   /\\",
            "  <{eyes}>",
            " /  {mouth}\\",
            "<_{pattern}{accent}_>",
            "   \\/",
            " glow"
        ],
        &["/\\", "<{eyes}>", "/{mouth}\\", "<_>"]
    ),
    set!(
        &[
            "  /\\_/\\",
            " / {eyes} \\",
            "<   {mouth}  >",
            " \\{pattern}_{accent}/",
            "  \\_/",
            " old"
        ],
        &["/\\_/\\", "{eyes}", "<{mouth}>", "\\_/"]
    ),
];

const MECH_S0: &[TemplateSet] = &[set!(
    &["  .-.", " [ {eyes}]", "  |{mouth}|", "  /_\\"],
    &[".-.", "[{eyes}]", "|{mouth}|"]
)];
const MECH_S1: &[TemplateSet] = &[
    set!(
        &["  .-.", " [{eyes}]", " /|{mouth}|\\", "  treads"],
        &[".-.", "[{eyes}]", "|{mouth}|"]
    ),
    set!(
        &["  .^.", " [{eyes}]", " <|{mouth}|>", "  == "],
        &[".^.", "[{eyes}]", "<{mouth}>"]
    ),
];
const MECH_ADULT: &[TemplateSet] = &[
    set!(
        &[
            "  .---.",
            " [{eyes}]",
            "<| {mouth} |>",
            " |{pattern}__{accent}|",
            " /====\\"
        ],
        &[".---.", "[{eyes}]", "| {mouth}|", "/==\\"]
    ),
    set!(
        &[
            "  _[=]_",
            " / {eyes} \\",
            "|   {mouth} |",
            "|_{pattern}_{accent}_|",
            "  O--O"
        ],
        &["_[=]_", "{eyes}", "|{mouth}|", "O-O"]
    ),
    set!(
        &[
            "  .-^-.",
            " [{accent}{eyes}{accent}]",
            "<  {mouth}  >",
            "|{pattern}----|",
            " d====b"
        ],
        &[".-^-.", "[{eyes}]", "<{mouth}>", "d==b"]
    ),
];
const MECH_S2: &[TemplateSet] = &[
    set!(
        &["  .-.", " [{eyes}]", " /|{mouth}|\\", "  {pattern} legs"],
        &[".-.", "[{eyes}]", "|{mouth}|", "legs"]
    ),
    set!(
        &["  .^.", " [{eyes}]", " <|{mouth}|>", "  =={accent}"],
        &[".^.", "[{eyes}]", "<{mouth}>", "=="]
    ),
    set!(
        &["  [=]", " [{eyes}]", "  |{mouth}|", " /{pattern}\\"],
        &["[=]", "[{eyes}]", "|{mouth}|", "/\\"]
    ),
];
const MECH_S3: &[TemplateSet] = &[
    set!(
        &[" .---.", "[{eyes}]", "<|{mouth}|>", " |{pattern}_{accent}|"],
        &[".---", "[{eyes}]", "|{mouth}|", "|_|"]
    ),
    set!(
        &[" _[=]_", "/ {eyes}\\", "| {mouth}|", " O-O"],
        &["_[=]", "{eyes}", "|{mouth}|", "O-O"]
    ),
    set!(
        &[" .-^-.", "[{eyes}]", "< {mouth}>", " d=b"],
        &[".-^", "[{eyes}]", "<{mouth}>", "d=b"]
    ),
];
const MECH_S4: &[TemplateSet] = MECH_ADULT;
const MECH_S5: &[TemplateSet] = &[
    set!(
        &[
            "  .---.",
            " [{eyes}]",
            "<| {mouth} |>",
            " |{pattern}__{accent}|",
            " /====\\"
        ],
        &[".---.", "[{eyes}]", "| {mouth}|", "/==\\", "mk5"]
    ),
    set!(
        &[
            "  _[=]_",
            " / {eyes} \\",
            "|   {mouth} |",
            "|_{pattern}_{accent}_|",
            "  O--O"
        ],
        &["_[=]_", "{eyes}", "|{mouth}|", "O-O"]
    ),
    set!(
        &[
            "  .-^-.",
            " [{accent}{eyes}{accent}]",
            "<  {mouth}  >",
            "|{pattern}----|",
            " d====b"
        ],
        &[".-^-.", "[{eyes}]", "<{mouth}>", "d==b"]
    ),
];
const MECH_S6: &[TemplateSet] = &[
    set!(
        &[
            "  .---.",
            " [{eyes}]",
            "<| {mouth} |>",
            " |{pattern}__{accent}|",
            " /====\\",
            " relay"
        ],
        &[".---.", "[{eyes}]", "| {mouth}|", "/==\\", "relay"]
    ),
    set!(
        &[
            "  _[=]_",
            " / {eyes} \\",
            "|   {mouth} |",
            "|_{pattern}_{accent}_|",
            "  O--O",
            " gear"
        ],
        &["_[=]_", "{eyes}", "|{mouth}|", "O-O"]
    ),
    set!(
        &[
            "  .-^-.",
            " [{accent}{eyes}{accent}]",
            "<  {mouth}  >",
            "|{pattern}----|",
            " d====b",
            " old"
        ],
        &[".-^-.", "[{eyes}]", "<{mouth}>", "d==b"]
    ),
];

fn fuzz_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => FUZZ_S0,
        StageKey::S1 => FUZZ_S1,
        StageKey::S2 => FUZZ_S2,
        StageKey::S3 => FUZZ_S3,
        StageKey::S4 => FUZZ_S4,
        StageKey::S5 => FUZZ_S5,
        StageKey::S6 => FUZZ_S6,
    }
}

fn blob_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => BLOB_S0,
        StageKey::S1 => BLOB_S1,
        StageKey::S2 => BLOB_S2,
        StageKey::S3 => BLOB_S3,
        StageKey::S4 => BLOB_S4,
        StageKey::S5 => BLOB_S5,
        StageKey::S6 => BLOB_S6,
    }
}

fn ghost_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => GHOST_S0,
        StageKey::S1 => GHOST_S1,
        StageKey::S2 => GHOST_S2,
        StageKey::S3 => GHOST_S3,
        StageKey::S4 => GHOST_S4,
        StageKey::S5 => GHOST_S5,
        StageKey::S6 => GHOST_S6,
    }
}

fn glitch_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => GLITCH_S0,
        StageKey::S1 => GLITCH_S1,
        StageKey::S2 => GLITCH_S2,
        StageKey::S3 => GLITCH_S3,
        StageKey::S4 => GLITCH_S4,
        StageKey::S5 => GLITCH_S5,
        StageKey::S6 => GLITCH_S6,
    }
}

fn crystal_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => CRYSTAL_S0,
        StageKey::S1 => CRYSTAL_S1,
        StageKey::S2 => CRYSTAL_S2,
        StageKey::S3 => CRYSTAL_S3,
        StageKey::S4 => CRYSTAL_S4,
        StageKey::S5 => CRYSTAL_S5,
        StageKey::S6 => CRYSTAL_S6,
    }
}

fn mech_templates(stage: StageKey) -> &'static [TemplateSet] {
    match stage {
        StageKey::S0 => MECH_S0,
        StageKey::S1 => MECH_S1,
        StageKey::S2 => MECH_S2,
        StageKey::S3 => MECH_S3,
        StageKey::S4 => MECH_S4,
        StageKey::S5 => MECH_S5,
        StageKey::S6 => MECH_S6,
    }
}
