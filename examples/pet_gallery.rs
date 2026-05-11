//! Aesthetic-tuning binary for the production parts generator.
//!
//! Generates 50 procedurally-composed pets across all 6 species × 3 stages
//! and prints them in a grid for visual inspection of the parts catalogs.

use glorp::game::evolution::Stage;
use glorp::pet::generate::generate_pet_lines;
use glorp::pet::generation::Species;

fn main() {
    println!("parts_gallery — production parts generator\n");
    let species = [
        Species::Fuzz,
        Species::Blob,
        Species::Ghost,
        Species::Glitch,
        Species::Crystal,
        Species::Mech,
    ];
    let stages: [Stage; 3] = [Stage::S0, Stage::S1, Stage::S2];

    let mut count: u32 = 0;
    for sp in species {
        for st in stages {
            for seed_base in 0u64..3 {
                if count >= 50 {
                    return;
                }
                let seed = (sp as u64).wrapping_mul(0x9e37)
                    ^ (st as u64).wrapping_mul(0x7c15)
                    ^ (seed_base * 137);
                println!("--- #{count:02}  {sp:?}  {st:?}  seed={seed} ---");
                for line in generate_pet_lines(sp, st, seed) {
                    println!("    {line}");
                }
                println!();
                count += 1;
            }
        }
    }
}
