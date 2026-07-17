pub const COMPANION_STATISTICS_Z: f32 = 0.72;
pub const COMPANION_GAUGE_PACE_Z: f32 = 1.55;
pub const COMPANION_GAUGE_DAILY_Z: f32 = 1.65;
pub const COMPANION_GAUGE_XP_Z: f32 = 1.75;
pub const COMPANION_PET_MIN_Z: f32 = -1.0;
pub const COMPANION_STATISTICS_INTERACTION_START_Z: f32 = 0.64;
pub const COMPANION_STATISTICS_ECHO_Z: f32 = COMPANION_STATISTICS_INTERACTION_START_Z;
pub const COMPANION_PET_MAX_Z: f32 = 1.0;
pub const COMPANION_CAMERA_NEAR_Z: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompanionGaugeLane {
    Pace,
    Daily,
    Xp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PetStatisticsOrder {
    BehindStatistics,
    InFrontOfStatistics,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionGaugeDepthPlanes {
    pub pace: f32,
    pub daily: f32,
    pub xp: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StatisticsDepthInteraction {
    pub start_z: f32,
    pub plane_z: f32,
    pub reveal_mix: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionDepthComposition {
    pub pet_effective_z: f32,
    pub statistics_z: f32,
    pub statistics_interaction: StatisticsDepthInteraction,
    pub gauges: CompanionGaugeDepthPlanes,
    pub pet_statistics_order: PetStatisticsOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionDepthCompositionError {
    InvalidEffectiveDepth,
    InvalidPlaneOrder,
}

impl CompanionGaugeLane {
    pub const fn scene_z(self) -> f32 {
        match self {
            Self::Pace => COMPANION_GAUGE_PACE_Z,
            Self::Daily => COMPANION_GAUGE_DAILY_Z,
            Self::Xp => COMPANION_GAUGE_XP_Z,
        }
    }
}

impl StatisticsDepthInteraction {
    pub fn resolve(pet_effective_z: f32) -> Result<Self, CompanionDepthCompositionError> {
        if !pet_effective_z.is_finite()
            || !(COMPANION_PET_MIN_Z..=COMPANION_PET_MAX_Z).contains(&pet_effective_z)
        {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        if !(COMPANION_PET_MIN_Z < COMPANION_STATISTICS_INTERACTION_START_Z
            && COMPANION_STATISTICS_INTERACTION_START_Z < COMPANION_STATISTICS_Z
            && COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z)
        {
            return Err(CompanionDepthCompositionError::InvalidPlaneOrder);
        }
        let linear = ((pet_effective_z - COMPANION_STATISTICS_INTERACTION_START_Z)
            / (COMPANION_STATISTICS_Z - COMPANION_STATISTICS_INTERACTION_START_Z))
            .clamp(0.0, 1.0);
        let reveal_mix = linear * linear * (3.0 - 2.0 * linear);
        if !reveal_mix.is_finite() || !(0.0..=1.0).contains(&reveal_mix) {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        Ok(Self {
            start_z: COMPANION_STATISTICS_INTERACTION_START_Z,
            plane_z: COMPANION_STATISTICS_Z,
            reveal_mix,
        })
    }
}

impl CompanionDepthComposition {
    pub fn resolve(pet_effective_z: f32) -> Result<Self, CompanionDepthCompositionError> {
        if !pet_effective_z.is_finite()
            || !(COMPANION_PET_MIN_Z..=COMPANION_PET_MAX_Z).contains(&pet_effective_z)
        {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        if !(COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z
            && COMPANION_PET_MAX_Z < COMPANION_GAUGE_PACE_Z
            && COMPANION_GAUGE_PACE_Z < COMPANION_GAUGE_DAILY_Z
            && COMPANION_GAUGE_DAILY_Z < COMPANION_GAUGE_XP_Z
            && COMPANION_GAUGE_XP_Z < COMPANION_CAMERA_NEAR_Z)
        {
            return Err(CompanionDepthCompositionError::InvalidPlaneOrder);
        }
        let statistics_interaction = StatisticsDepthInteraction::resolve(pet_effective_z)?;
        Ok(Self {
            pet_effective_z,
            statistics_z: COMPANION_STATISTICS_Z,
            statistics_interaction,
            gauges: CompanionGaugeDepthPlanes {
                pace: COMPANION_GAUGE_PACE_Z,
                daily: COMPANION_GAUGE_DAILY_Z,
                xp: COMPANION_GAUGE_XP_Z,
            },
            pet_statistics_order: if pet_effective_z > COMPANION_STATISTICS_Z {
                PetStatisticsOrder::InFrontOfStatistics
            } else {
                PetStatisticsOrder::BehindStatistics
            },
        })
    }
}

/// The pet keeps a visible front-to-back excursion without reading as though it
/// crosses a room-sized volume. Shadow separation carries the remaining Z cue.
pub const SMOOTH_PET_FAR_SCALE: f32 = 0.97;
pub const SMOOTH_PET_NEAR_SCALE: f32 = 1.035;
pub const SMOOTH_PERSPECTIVE_Y_MAX: f32 = 0.10;

/// The far plane keeps most of its ink in the deliberately shallow tank; the
/// neutral and near planes remain fully present.
pub const SMOOTH_FAR_ATMOSPHERE: f32 = 0.93;

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
    crate::presentation::companion_effects::depth_lifecycle_scale(asleep, calm)
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

    let effective_z =
        crate::presentation::companion_effects::effective_depth(raw_z, lifecycle_scale);
    let (scale, perspective_y, atmosphere) = canonical_smooth_depth_cues(effective_z);
    validate_smooth_depth_sample(SmoothDepthSample {
        raw_z,
        effective_z,
        scale,
        perspective_y,
        atmosphere,
    })
}

pub(crate) fn validate_smooth_depth_sample(
    sample: SmoothDepthSample,
) -> Result<SmoothDepthSample, SmoothDepthError> {
    let (expected_scale, expected_perspective_y, expected_atmosphere) =
        canonical_smooth_depth_cues(sample.effective_z);
    let clamped_raw_z = sample.raw_z.clamp(-1.0, 1.0);
    let effective_depth_is_reachable = if clamped_raw_z >= 0.0 {
        (0.0..=clamped_raw_z).contains(&sample.effective_z)
    } else {
        (clamped_raw_z..=0.0).contains(&sample.effective_z)
    };
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
        || sample.scale != expected_scale
        || sample.perspective_y != expected_perspective_y
        || sample.atmosphere != expected_atmosphere
        || !effective_depth_is_reachable
    {
        return Err(SmoothDepthError::InvalidOutput);
    }
    Ok(sample)
}

fn canonical_smooth_depth_cues(effective_z: f32) -> (f32, f32, f32) {
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
    (scale, perspective_y, atmosphere)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_depth_planes_are_exact_and_strictly_ordered() {
        assert_eq!(COMPANION_STATISTICS_Z, 0.72);
        assert_eq!(COMPANION_GAUGE_PACE_Z, 1.55);
        assert_eq!(COMPANION_GAUGE_DAILY_Z, 1.65);
        assert_eq!(COMPANION_GAUGE_XP_Z, 1.75);
        assert!(COMPANION_STATISTICS_Z < 1.0);
        assert!(1.0 < COMPANION_GAUGE_PACE_Z);
        assert!(COMPANION_GAUGE_PACE_Z < COMPANION_GAUGE_DAILY_Z);
        assert!(COMPANION_GAUGE_DAILY_Z < COMPANION_GAUGE_XP_Z);
        assert!(COMPANION_GAUGE_XP_Z < 2.0);
    }

    #[test]
    fn statistics_crossing_is_strictly_after_the_plane() {
        let boundary = CompanionDepthComposition::resolve(COMPANION_STATISTICS_Z).unwrap();
        assert_eq!(
            boundary.pet_statistics_order,
            PetStatisticsOrder::BehindStatistics
        );

        let just_crossed = CompanionDepthComposition::resolve(f32::from_bits(
            COMPANION_STATISTICS_Z.to_bits() + 1,
        ))
        .unwrap();
        assert_eq!(
            just_crossed.pet_statistics_order,
            PetStatisticsOrder::InFrontOfStatistics
        );
    }

    #[test]
    fn statistics_interaction_uses_the_exact_one_sided_band() {
        for (depth, expected) in [
            (COMPANION_STATISTICS_INTERACTION_START_Z, 0.0),
            (0.68, 0.5),
            (COMPANION_STATISTICS_Z, 1.0),
            (f32::from_bits(COMPANION_STATISTICS_Z.to_bits() + 1), 1.0),
        ] {
            let interaction = StatisticsDepthInteraction::resolve(depth).unwrap();
            assert_eq!(interaction.start_z, 0.64);
            assert_eq!(interaction.plane_z, 0.72);
            assert!((interaction.reveal_mix - expected).abs() <= f32::EPSILON * 8.0);
        }
    }

    #[test]
    fn statistics_interaction_is_reversible_and_uses_effective_depth() {
        let approaching = StatisticsDepthInteraction::resolve(0.68).unwrap();
        let retreating = StatisticsDepthInteraction::resolve(0.68).unwrap();
        assert_eq!(approaching, retreating);

        let asleep_sample = resolve_smooth_depth(1.0, 0.68).unwrap();
        let asleep = CompanionDepthComposition::resolve(asleep_sample.effective_z).unwrap();
        assert!((asleep.statistics_interaction.reveal_mix - 0.5).abs() <= f32::EPSILON * 8.0);
    }

    #[test]
    fn statistics_interaction_rejects_non_finite_or_invalid_plane_order() {
        assert_eq!(
            StatisticsDepthInteraction::resolve(f32::NAN),
            Err(CompanionDepthCompositionError::InvalidEffectiveDepth)
        );
        assert!(COMPANION_PET_MIN_Z < COMPANION_STATISTICS_INTERACTION_START_Z);
        assert!(COMPANION_STATISTICS_INTERACTION_START_Z < COMPANION_STATISTICS_Z);
        assert!(COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z);
    }

    #[test]
    fn companion_depth_composition_rejects_invalid_effective_depth() {
        for depth in [f32::NAN, f32::INFINITY, -1.01, 1.01] {
            assert!(CompanionDepthComposition::resolve(depth).is_err());
        }
    }

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
