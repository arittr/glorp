use crate::tui::room::{biome_symbols, RoomLifeProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRoom {
    pub primary_biome: String,
    pub secondary_biome: Option<String>,
    pub species_dialect: String,
    pub dialect_status: String,
    pub room_weather: String,
    pub prop_landmarks: Vec<String>,
    pub glyph_vocabulary: Vec<String>,
    pub placements: Vec<PresentationRoomPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRoomPlacement {
    pub target_id: String,
    pub glyph: String,
}

impl PresentationRoom {
    pub fn from_profile(profile: &RoomLifeProfile) -> Self {
        Self {
            primary_biome: format!("{:?}", profile.biome.primary),
            secondary_biome: profile.biome.secondary.map(|tag| format!("{tag:?}")),
            species_dialect: profile.species_dialect.key.as_str().to_string(),
            dialect_status: profile.species_dialect.status.as_str().to_string(),
            room_weather: format!("{:?}", profile.room_weather),
            prop_landmarks: profile
                .identity_prop_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            glyph_vocabulary: biome_symbols(profile.biome.primary, profile.species_dialect)
                .iter()
                .map(|ch| ch.to_string())
                .collect(),
            placements: Vec::new(),
        }
    }

    pub(crate) fn debug_assert_matches_profile(&self, profile: &RoomLifeProfile) {
        debug_assert_eq!(self.primary_biome, format!("{:?}", profile.biome.primary));
        debug_assert_eq!(self.species_dialect, profile.species_dialect.key.as_str());
    }
}
