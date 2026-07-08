use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::Species;
use crate::presentation::surface::ResolvedColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelVariationKey(pub u16);

impl PixelVariationKey {
    pub fn from_seed(seed: &str) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in seed.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self((hash ^ (hash >> 32)) as u16)
    }

    pub const fn bucket(self, modulo: u16) -> u16 {
        if modulo == 0 {
            0
        } else {
            self.0 % modulo
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPetIdentity {
    pub species: Species,
    pub stage: Stage,
    pub variation_key: PixelVariationKey,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelActivity {
    pub level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSleepState {
    pub asleep: bool,
    pub calm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPulseState {
    pub active: bool,
    pub age_ms: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelPetInput {
    pub identity: PixelPetIdentity,
    pub mood: Mood,
    pub palette: ResolvedColors,
    pub activity: PixelActivity,
    pub sleep: PixelSleepState,
    pub pulse: PixelPulseState,
}
