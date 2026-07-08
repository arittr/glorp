pub mod animator;
pub mod art_reference;
pub mod frame;
pub mod input;
pub mod raster;
pub mod scene;

pub use animator::{render_pixel_frame, PixelRendererState, PixelRendererTick};
pub use art_reference::{
    PixelArtCell, PixelArtPoseKey, PixelArtReferenceProvider, PixelArtReferenceRequest,
    PixelArtRole, PixelCanonicalAnimationInputs, PixelCellBounds, PixelCueCoverage,
    PixelFootContact, PixelPetArtReference, PixelProtectedRegion, PixelReferenceChecksum,
};
pub use frame::{pixel_runs, PixelBounds, PixelFrame, PixelRun, PixelViewport, Rgba8};
pub use input::{
    PixelActivity, PixelPetIdentity, PixelPetInput, PixelPulseState, PixelSleepState,
    PixelVariationKey,
};
pub use scene::PixelPetScene;
