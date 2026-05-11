// examples/pet_gallery.rs
//
// Phase 0 spike: prints 50 procedurally-generated pets in a grid so the
// aesthetic-validation gate (zero broken pets, >=85% read as intentional
// creatures) can be checked by visual inspection.
//
// This file is standalone — no glorp internals — until Phase 1 promotes the
// generator into src/pet/generate.rs.

fn main() {
    println!("pet_gallery — Phase 0 procedural pet spike");
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilhouetteParams {
    pub width_px: u8,         // even — Braille is 2 wide per cell
    pub height_px: u8,        // multiple of 4 — Braille is 4 tall per cell
    pub roundness: f32,       // 0..1 — Gaussian envelope tightness
    pub taper: f32,           // 0..1 — corner falloff strength
    pub body_density: f32,    // 0..1 — overall fill probability
    pub asymmetry_seed: u32,
    pub head_zone_ratio: f32, // 0..1 — fraction of top grid reserved as head
    pub ornament_density: f32,
}

// Mirror of production `crate::game::evolution::Stage` so the spike has no
// glorp dependency. Production has S0..=S6; Phase 1 promotes to the real enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage { S0, S1, S2, S3, S4, S5, S6 }

// Mirror of production `crate::pet::generation::Species`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpeciesK {
    Fuzz, Blob, Ghost, Glitch, Crystal, Mech,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MutationVector {
    pub d_roundness: f32,
    pub d_taper: f32,
    pub d_body_density: f32,
    pub d_ornament_density: f32,
    pub d_head_zone_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct PetBlueprint {
    pub species: SpeciesK,
    pub stage: Stage,
    pub silhouette: SilhouetteParams,
    pub mutation_vector: MutationVector,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silhouette_params_round_trips() {
        let p = SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.5, taper: 0.5,
            body_density: 0.5, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        };
        assert_eq!(p, p.clone());
    }
}
