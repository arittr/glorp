use crate::game::habitat::{
    HabitatPetLayer, HabitatPropKind, CODEX_SIGNAL_LAMP, FIRST_ENSEMBLE_DAY, HEAVY_SESSION_PLANTER,
    RETURN_SPROUT, TOKEN_AURORA_500M, TOKEN_BONSAI_100M, TOKEN_CONSTELLATION_250M,
    TOKEN_FRIENDLY_CLOUD_750K, TOKEN_GEODE_50M, TOKEN_HANGING_VINE_25M, TOKEN_LANTERN_10M,
    TOKEN_MOON_1B, TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M, TOKEN_PEBBLE_25K, TOKEN_REEDS_5M,
    TOKEN_SHARD_1M, TOKEN_SHELL_100K, TOKEN_SPARK_500K, TOKEN_TREASURE_CHEST_2M,
    WILT_RECOVERY_SPROUT,
};
use crate::pet::generation::Species;
use crate::presentation::target::SurfaceTargetId;
use crate::tui::component::habitat_props::{HabitatPropCell, HabitatPropPlacement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropVisualState {
    pub(crate) species: Species,
    pub(crate) sprite_phase: Option<u8>,
    pub(crate) twinkle_active: Option<bool>,
    pub(crate) chest_lid_open: Option<bool>,
    pub(crate) bloom_active: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropSpriteCell {
    pub(crate) dx: i8,
    pub(crate) dy: i8,
    pub(crate) glyph: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropFootprint {
    pub(crate) min_dx: i8,
    pub(crate) max_dx: i8,
    pub(crate) min_dy: i8,
    pub(crate) max_dy: i8,
}

pub(crate) fn presentation_unknown_prop_sprite(
    kind: HabitatPropKind,
) -> Vec<PresentationPropSpriteCell> {
    match kind {
        HabitatPropKind::Trophy => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
        ],
        HabitatPropKind::Accent => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' }]
        }
    }
}

pub(crate) fn presentation_prop_sprite(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<Vec<PresentationPropSpriteCell>> {
    let first_sprite_phase = state.sprite_phase == Some(0);
    let bloomed = state.bloom_active == Some(true);
    let twinkle = state.twinkle_active == Some(true);
    let sprite = match catalog_id {
        TOKEN_PEBBLE_25K => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▲' }],
        TOKEN_SHELL_100K => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◌' }],
        TOKEN_SPARK_500K => vec![PresentationPropSpriteCell {
            dx: 0,
            dy: 0,
            glyph: if twinkle { '✦' } else { '·' },
        }],
        TOKEN_SHARD_1M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '#' }]
        }
        TOKEN_SHARD_1M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◆' }],
        TOKEN_ORBIT_5M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: ']' }]
        }
        TOKEN_ORBIT_5M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '°' }],
        TOKEN_LANTERN_10M if matches!(state.species, Species::Glitch) && twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '_' }]
        }
        TOKEN_LANTERN_10M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: ':' }]
        }
        TOKEN_LANTERN_10M if matches!(state.species, Species::Crystal) && twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' }]
        }
        TOKEN_LANTERN_10M if twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '☼' }]
        }
        TOKEN_LANTERN_10M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '○' }],
        // Bloomed plants flower once matured in the tank. The blossoms (*)
        // twinkle between phases while paint adapters retain blossom coloring.
        TOKEN_MOSS_TUFT_250K if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▂' },
        ],
        TOKEN_MOSS_TUFT_250K if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▃' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▂' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▃' },
        ],
        TOKEN_HANGING_VINE_25M if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╲' },
        ],
        TOKEN_HANGING_VINE_25M if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╱' },
        ],
        HEAVY_SESSION_PLANTER if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        HEAVY_SESSION_PLANTER if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        TOKEN_MOSS_TUFT_250K if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▂' },
        ],
        TOKEN_MOSS_TUFT_250K => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▃' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▂' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▃' },
        ],
        TOKEN_FRIENDLY_CLOUD_750K if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '☁' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◦' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◡' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◦' },
        ],
        TOKEN_FRIENDLY_CLOUD_750K => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '☁' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '˙' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◡' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '˙' },
        ],
        // Treasure chest: lid open (gap in the lid + a ✦ glint) during the
        // bubble cycle's open window, closed (solid lid + ◆) otherwise.
        TOKEN_TREASURE_CHEST_2M if state.chest_lid_open == Some(true) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▣' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▣' },
        ],
        TOKEN_TREASURE_CHEST_2M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╭' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '─' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╮' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▣' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▣' },
        ],
        TOKEN_HANGING_VINE_25M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╲' },
        ],
        TOKEN_HANGING_VINE_25M => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╱' },
        ],
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Glitch) && first_sprite_phase => {
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
                PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '#' },
                PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '_' },
            ]
        }
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Glitch) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '_' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: ':' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Crystal) && first_sprite_phase => {
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
                PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◆' },
                PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
            ]
        }
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Crystal) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◉' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '○' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        HEAVY_SESSION_PLANTER if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: 'ѱ' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        HEAVY_SESSION_PLANTER => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: 'ѱ' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        WILT_RECOVERY_SPROUT if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
        ],
        WILT_RECOVERY_SPROUT => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
        ],
        // Amethyst geode: a 3×3 facet cluster in a rock cradle that shimmers.
        TOKEN_GEODE_50M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◈' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '◣' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▼' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '◢' },
        ],
        TOKEN_GEODE_50M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '◣' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▼' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '◢' },
        ],
        TOKEN_BONSAI_100M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '▂' },
        ],
        TOKEN_BONSAI_100M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '▂' },
        ],
        TOKEN_CONSTELLATION_250M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '✦' },
        ],
        TOKEN_CONSTELLATION_250M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '·' },
        ],
        TOKEN_AURORA_500M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 4, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 4, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 4, dy: 2, glyph: '┊' },
        ],
        TOKEN_AURORA_500M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 4, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 4, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 4, dy: 2, glyph: '┊' },
        ],
        TOKEN_MOON_1B if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◑' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 3, dy: 1, glyph: '✦' },
        ],
        TOKEN_MOON_1B => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◑' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 3, dy: 1, glyph: '·' },
        ],
        TOKEN_REEDS_5M if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M if bloomed => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╵' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╵' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        FIRST_ENSEMBLE_DAY | RETURN_SPROUT => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
        ],
        _ => return None,
    };
    Some(sprite)
}

fn presentation_prop_visual_states(catalog_id: &str) -> Option<Vec<PresentationPropVisualState>> {
    crate::game::habitat::catalog_prop_by_str(catalog_id)?;
    let supports_sprite_phase = crate::game::habitat::habitat_prop_animation_state(
        catalog_id,
        time::OffsetDateTime::UNIX_EPOCH,
    )
    .sprite_phase
    .is_some();
    let phases = if supports_sprite_phase {
        &[Some(0), Some(1)][..]
    } else {
        &[None][..]
    };
    let mut states = Vec::with_capacity(Species::all().len() * phases.len() * 8);
    for species in Species::all() {
        for &sprite_phase in phases {
            for twinkle_active in [false, true] {
                for chest_lid_open in [false, true] {
                    for bloom_active in [false, true] {
                        states.push(PresentationPropVisualState {
                            species,
                            sprite_phase,
                            twinkle_active: Some(twinkle_active),
                            chest_lid_open: Some(chest_lid_open),
                            bloom_active: Some(bloom_active),
                        });
                    }
                }
            }
        }
    }
    Some(states)
}

pub(crate) fn presentation_prop_max_footprint(
    catalog_id: &str,
) -> Option<PresentationPropFootprint> {
    let mut footprint = None::<PresentationPropFootprint>;
    for state in presentation_prop_visual_states(catalog_id)? {
        for cell in presentation_prop_sprite(catalog_id, state)? {
            footprint = Some(match footprint {
                Some(current) => PresentationPropFootprint {
                    min_dx: current.min_dx.min(cell.dx),
                    max_dx: current.max_dx.max(cell.dx),
                    min_dy: current.min_dy.min(cell.dy),
                    max_dy: current.max_dy.max(cell.dy),
                },
                None => PresentationPropFootprint {
                    min_dx: cell.dx,
                    max_dx: cell.dx,
                    min_dy: cell.dy,
                    max_dy: cell.dy,
                },
            });
        }
    }
    footprint
}

pub(crate) fn presentation_prop_occupied_offsets(catalog_id: &str) -> Option<Vec<[i8; 2]>> {
    let mut offsets = Vec::new();
    for state in presentation_prop_visual_states(catalog_id)? {
        for cell in presentation_prop_sprite(catalog_id, state)? {
            let offset = [cell.dx, cell.dy];
            if !offsets.contains(&offset) {
                offsets.push(offset);
            }
        }
    }
    offsets.sort_unstable();
    Some(offsets)
}

pub(crate) fn presentation_prop_footprint(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<PresentationPropFootprint> {
    let sprite = presentation_prop_sprite(catalog_id, state)?;
    let mut cells = sprite.into_iter();
    let first = cells.next()?;
    Some(cells.fold(
        PresentationPropFootprint {
            min_dx: first.dx,
            max_dx: first.dx,
            min_dy: first.dy,
            max_dy: first.dy,
        },
        |footprint, cell| PresentationPropFootprint {
            min_dx: footprint.min_dx.min(cell.dx),
            max_dx: footprint.max_dx.max(cell.dx),
            min_dy: footprint.min_dy.min(cell.dy),
            max_dy: footprint.max_dy.max(cell.dy),
        },
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationPropPlacement {
    pub prop_id: String,
    pub layer: PresentationPropLayer,
    pub bounds: PresentationRect,
    pub cells: Vec<PresentationPropCell>,
    pub effect_target: Option<SurfaceTargetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationPropLayer {
    Background,
    Behind,
    Foreground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationPropCell {
    pub x: u16,
    pub y: u16,
    pub glyph: char,
}

impl PresentationPropPlacement {
    pub fn from_habitat_placement(placement: &HabitatPropPlacement) -> Self {
        Self {
            prop_id: placement.prop_id.as_str().to_string(),
            layer: presentation_layer(placement.pet_layer),
            bounds: PresentationRect {
                x: placement.bounds.x,
                y: placement.bounds.y,
                width: placement.bounds.width,
                height: placement.bounds.height,
            },
            cells: placement.cells.iter().map(presentation_cell).collect(),
            effect_target: placement.target_id.map(|target| {
                let raw = target.as_str();
                let neutral = raw.strip_prefix("watch.").unwrap_or(raw);
                SurfaceTargetId::new(neutral.to_string())
            }),
        }
    }
}

fn presentation_layer(layer: HabitatPetLayer) -> PresentationPropLayer {
    match layer {
        HabitatPetLayer::Background => PresentationPropLayer::Background,
        HabitatPetLayer::Behind => PresentationPropLayer::Behind,
        HabitatPetLayer::Foreground => PresentationPropLayer::Foreground,
    }
}

fn presentation_cell(cell: &HabitatPropCell) -> PresentationPropCell {
    PresentationPropCell {
        x: cell.col,
        y: cell.row,
        glyph: cell.glyph,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::*;
    use crate::pet::generation::Species;

    #[test]
    fn unknown_prop_fallback_art_is_owned_by_presentation() {
        assert_eq!(
            presentation_unknown_prop_sprite(HabitatPropKind::Trophy),
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
                PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
            ]
        );
        assert_eq!(
            presentation_unknown_prop_sprite(HabitatPropKind::Accent),
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' }]
        );
    }

    #[test]
    fn canonical_prop_states_fit_their_frozen_footprints() {
        let frozen_footprint = |catalog_id| match catalog_id {
            TOKEN_PEBBLE_25K | TOKEN_SHELL_100K | TOKEN_SPARK_500K | TOKEN_SHARD_1M
            | TOKEN_ORBIT_5M | TOKEN_LANTERN_10M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 0,
                min_dy: 0,
                max_dy: 0,
            },
            TOKEN_MOSS_TUFT_250K
            | TOKEN_FRIENDLY_CLOUD_750K
            | TOKEN_TREASURE_CHEST_2M
            | TOKEN_REEDS_5M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 1,
            },
            TOKEN_HANGING_VINE_25M
            | TOKEN_GEODE_50M
            | TOKEN_BONSAI_100M
            | TOKEN_CONSTELLATION_250M
            | HEAVY_SESSION_PLANTER => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 2,
            },
            TOKEN_AURORA_500M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 4,
                min_dy: 0,
                max_dy: 2,
            },
            TOKEN_MOON_1B => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 3,
                min_dy: 0,
                max_dy: 2,
            },
            CODEX_SIGNAL_LAMP => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 0,
                min_dy: 0,
                max_dy: 2,
            },
            WILT_RECOVERY_SPROUT => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 1,
            },
            FIRST_ENSEMBLE_DAY | RETURN_SPROUT => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 1,
                min_dy: 0,
                max_dy: 1,
            },
            other => panic!("missing frozen footprint for {other}"),
        };

        for spec in HABITAT_PROP_CATALOG {
            let footprint = presentation_prop_max_footprint(spec.id)
                .unwrap_or_else(|| panic!("{} must have a canonical footprint", spec.id));
            assert_eq!(
                footprint,
                frozen_footprint(spec.id),
                "{} footprint",
                spec.id
            );

            let states = presentation_prop_visual_states(spec.id)
                .unwrap_or_else(|| panic!("{} must enumerate visual states", spec.id));
            assert!(
                !states.is_empty(),
                "{} must enumerate visual states",
                spec.id
            );
            for species in Species::all() {
                assert!(states.iter().any(|state| state.species == species));
            }
            for state in states {
                let active_footprint = presentation_prop_footprint(spec.id, state)
                    .unwrap_or_else(|| panic!("{} must have an active footprint", spec.id));
                assert_eq!(active_footprint.min_dx, 0, "{} state {state:?}", spec.id);
                assert_eq!(active_footprint.min_dy, 0, "{} state {state:?}", spec.id);
                let sprite = presentation_prop_sprite(spec.id, state)
                    .unwrap_or_else(|| panic!("{} must have a canonical sprite", spec.id));
                assert!(
                    !sprite.is_empty(),
                    "{} must have a nonempty sprite",
                    spec.id
                );
                for cell in sprite {
                    assert!(
                        footprint.min_dx <= cell.dx && cell.dx <= footprint.max_dx,
                        "{} state {state:?} cell {cell:?} exceeds horizontal footprint {footprint:?}",
                        spec.id
                    );
                    assert!(
                        footprint.min_dy <= cell.dy && cell.dy <= footprint.max_dy,
                        "{} state {state:?} cell {cell:?} exceeds vertical footprint {footprint:?}",
                        spec.id
                    );
                }
            }
        }
    }

    #[test]
    fn vine_occupied_offsets_cover_every_animation_state() {
        assert_eq!(
            presentation_prop_occupied_offsets(TOKEN_HANGING_VINE_25M).unwrap(),
            vec![[0, 0], [0, 2], [1, 0], [1, 1], [1, 2], [2, 0], [2, 2]],
        );
    }
}
