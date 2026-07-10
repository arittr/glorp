/// The tank is deliberately shallow: the pet never recedes far enough to get
/// lost in the murk, and the wall shadow's detachment carries the Z story
/// across the small excursion.
pub const SMOOTH_PET_FAR_SCALE: f32 = 0.92;
pub const SMOOTH_PET_NEAR_SCALE: f32 = 1.12;
pub const SMOOTH_PERSPECTIVE_Y_MAX: f32 = 0.30;

/// Atmospheric perspective: things seen through more water lose contrast to it.
/// The far plane keeps this fraction of its ink; the near plane is fully present.
/// Size alone is a weak depth cue at a 12% excursion — this is what sells it.
pub const SMOOTH_FAR_ATMOSPHERE: f32 = 0.82;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothDepthSample {
    pub raw_z: f32,
    pub effective_z: f32,
    pub scale: f32,
    pub perspective_y: f32,
    /// Opacity multiplier for the pet's attached layers at this depth.
    pub atmosphere: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothDepthError {
    NonFiniteRawDepth,
    InvalidLifecycleScale,
    InvalidOutput,
}

impl std::fmt::Display for SmoothDepthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteRawDepth => f.write_str("smooth depth input must be finite"),
            Self::InvalidLifecycleScale => {
                f.write_str("smooth depth lifecycle scale must be finite and within [0, 1]")
            }
            Self::InvalidOutput => f.write_str("smooth depth output is outside finite bounds"),
        }
    }
}

impl std::error::Error for SmoothDepthError {}

pub const fn depth_lifecycle_scale(asleep: bool, calm: bool) -> f32 {
    if asleep {
        0.25
    } else if calm {
        0.5
    } else {
        1.0
    }
}

pub fn resolve_smooth_depth(
    raw_z: f32,
    lifecycle_scale: f32,
) -> Result<SmoothDepthSample, SmoothDepthError> {
    if !raw_z.is_finite() {
        return Err(SmoothDepthError::NonFiniteRawDepth);
    }
    if !lifecycle_scale.is_finite() || !(0.0..=1.0).contains(&lifecycle_scale) {
        return Err(SmoothDepthError::InvalidLifecycleScale);
    }

    let effective_z = raw_z.clamp(-1.0, 1.0) * lifecycle_scale;
    // The neutral plane renders the art at exactly 1.0 for Classic parity, so the
    // asymmetric excursion is mapped piecewise around it: a shallow back half and
    // a slightly deeper front half.
    let scale = if effective_z >= 0.0 {
        1.0 + effective_z * (SMOOTH_PET_NEAR_SCALE - 1.0)
    } else {
        1.0 + effective_z * (1.0 - SMOOTH_PET_FAR_SCALE)
    };
    let perspective_y = effective_z * SMOOTH_PERSPECTIVE_Y_MAX;
    // Only the back half of the tank carries murk: from neutral to the glass the
    // pet is fully present.
    let atmosphere = 1.0 + effective_z.min(0.0) * (1.0 - SMOOTH_FAR_ATMOSPHERE);
    validate_smooth_depth_sample(SmoothDepthSample {
        raw_z,
        effective_z,
        scale,
        perspective_y,
        atmosphere,
    })
}

fn validate_smooth_depth_sample(
    sample: SmoothDepthSample,
) -> Result<SmoothDepthSample, SmoothDepthError> {
    if !sample.raw_z.is_finite()
        || !sample.effective_z.is_finite()
        || !(-1.0..=1.0).contains(&sample.effective_z)
        || !sample.scale.is_finite()
        || !(SMOOTH_PET_FAR_SCALE..=SMOOTH_PET_NEAR_SCALE).contains(&sample.scale)
        || !sample.perspective_y.is_finite()
        || sample.perspective_y.abs() > SMOOTH_PERSPECTIVE_Y_MAX
        || sample.perspective_y.abs() >= 1.0
        || !sample.atmosphere.is_finite()
        || !(SMOOTH_FAR_ATMOSPHERE..=1.0).contains(&sample.atmosphere)
    {
        return Err(SmoothDepthError::InvalidOutput);
    }
    Ok(sample)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_depth_rejects_invalid_output_samples() {
        let valid = SmoothDepthSample {
            raw_z: 0.0,
            effective_z: 0.0,
            scale: 1.0,
            perspective_y: 0.0,
            atmosphere: 1.0,
        };
        assert_eq!(validate_smooth_depth_sample(valid), Ok(valid));

        for invalid in [
            SmoothDepthSample { scale: f32::NAN, ..valid },
            SmoothDepthSample {
                scale: SMOOTH_PET_FAR_SCALE - 0.01,
                ..valid
            },
            SmoothDepthSample {
                scale: SMOOTH_PET_NEAR_SCALE + 0.01,
                ..valid
            },
            SmoothDepthSample { effective_z: 1.01, ..valid },
            SmoothDepthSample { perspective_y: 1.0, ..valid },
            SmoothDepthSample {
                atmosphere: SMOOTH_FAR_ATMOSPHERE - 0.01,
                ..valid
            },
            SmoothDepthSample { atmosphere: 1.01, ..valid },
        ] {
            assert_eq!(
                validate_smooth_depth_sample(invalid),
                Err(SmoothDepthError::InvalidOutput)
            );
        }
    }
}
