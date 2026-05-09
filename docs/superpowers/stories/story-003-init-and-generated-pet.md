---
id: story-003
title: Init And Generated Pet
status: ready
tags: init, pet-generation, seed, naming
depends_on: [story-002]
---

As a user, I want `glorp init` to create one distinctive generated pet from a seed so that my first interaction is a short, personal hatching moment.

## Acceptance Criteria

- Running `glorp init` with no existing pet creates one new pet state under `~/.config/glorp/`.
- The pet is generated from a seed, and the same seed deterministically produces the same species, visual trait selections, palette selection, animation phase offsets, and generated name.
- MVP species include fuzz, blob, ghost, glitch, crystal, and mech.
- Generated names are species-aware, so different species use different name grammars or vocabularies.
- Init presents the generated name and lets the user accept it or provide a replacement name.
- There is no litter picker or multi-pet selection flow in MVP.
- Existing pet state blocks accidental re-init unless the user confirms reset/reinit.
- `glorp reset` performs a confirmed full reset of Glorp pet state; usage can be re-read from ccusage after the next init/watch.

## Implementation Notes

- The seed should be stored, but derived traits should be recomputable so the stored state stays small.
- If the user renames the pet, preserve the seed-derived pet traits and only override the display name.
- Reset is not death, retirement, or graveyard behavior. It is a deliberate start-over operation.

## Verification

- Seed fixtures produce stable species, names, and trait selections.
- Different species fixtures produce visibly different name styles.
- Init refuses to overwrite existing state without confirmation.
- Reset removes pet state in the test config directory and does not touch usage source logs.
