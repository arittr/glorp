pub mod frame;
pub mod input;

pub use frame::{PixelBounds, PixelFrame, PixelViewport, Rgba8};
pub use input::{
    PixelActivity, PixelPetIdentity, PixelPetInput, PixelPulseState, PixelSleepState,
    PixelVariationKey,
};
