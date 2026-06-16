use glorp::presentation::room::PresentationRoom;
use glorp::tui::room::derive_room_life_profile;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn presentation_room_preserves_profile_identity_without_placement() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let profile = derive_room_life_profile(&vm, datetime!(2026-06-15 12:00 UTC));

    let room = PresentationRoom::from_profile(&profile);

    assert_eq!(room.primary_biome, format!("{:?}", profile.biome.primary));
    assert_eq!(
        room.secondary_biome,
        profile.biome.secondary.map(|tag| format!("{tag:?}"))
    );
    assert_eq!(room.species_dialect, profile.species_dialect.key.as_str());
    assert_eq!(room.dialect_status, profile.species_dialect.status.as_str());
    assert_eq!(room.room_weather, format!("{:?}", profile.room_weather));
    assert_eq!(
        room.prop_landmarks,
        profile
            .identity_prop_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect::<Vec<_>>()
    );
    assert!(!room.glyph_vocabulary.is_empty());
    assert!(room
        .glyph_vocabulary
        .iter()
        .all(|glyph| glyph.chars().count() == 1));
    assert!(
        room.placements.is_empty(),
        "placement stays outside this plan"
    );
}
