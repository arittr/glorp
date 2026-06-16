use glorp::pet::render::{PaletteRoleName, StyledSegment};
use glorp::presentation::pet::{role_for_cell, role_names, PetTextBlock};

#[test]
fn presentation_pet_role_lookup_uses_character_indices() {
    let block = PetTextBlock::new(
        vec!["ab界d".to_string()],
        vec![
            StyledSegment {
                line: 0,
                start: 0,
                end: 2,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Accent,
            },
        ],
    );

    assert_eq!(role_for_cell(&block, 0, 0), PaletteRoleName::Eye);
    assert_eq!(role_for_cell(&block, 0, 1), PaletteRoleName::Eye);
    assert_eq!(role_for_cell(&block, 0, 2), PaletteRoleName::Accent);
    assert_eq!(role_for_cell(&block, 0, 3), PaletteRoleName::Body);
}

#[test]
fn presentation_pet_role_names_are_stable_and_deduped() {
    let roles = role_names(&[
        StyledSegment {
            line: 0,
            start: 0,
            end: 1,
            role: PaletteRoleName::Eye,
        },
        StyledSegment {
            line: 1,
            start: 0,
            end: 1,
            role: PaletteRoleName::Eye,
        },
        StyledSegment {
            line: 1,
            start: 1,
            end: 2,
            role: PaletteRoleName::Pattern,
        },
    ]);

    assert_eq!(roles, vec!["eye".to_string(), "pattern".to_string()]);
}
