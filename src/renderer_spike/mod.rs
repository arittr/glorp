//! Experimental renderer-decision harness.
//!
//! This module is deliberately feature-gated and benchmark-specific. Production
//! presentation and renderer modules must not depend on these DTOs.

pub mod artifacts;
pub mod fixture;
#[cfg(target_os = "macos")]
mod macos;
pub mod privacy;
#[cfg(target_os = "macos")]
mod smooth;
pub mod software;
#[cfg(target_os = "macos")]
mod software_host;
#[cfg(all(target_os = "macos", feature = "renderer-spike-wgpu"))]
mod wgpu;

use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::{GlorpError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum RendererSpikeCandidate {
    Smooth,
    Wgpu,
    Software,
}

impl RendererSpikeCandidate {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Smooth => "smooth",
            Self::Wgpu => "wgpu",
            Self::Software => "software",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum RendererSpikeTrack {
    Static,
    Ambient,
    Active,
    Dynamic,
    Resize,
    Occlusion,
    Capture,
}

impl RendererSpikeTrack {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Ambient => "ambient",
            Self::Active => "active",
            Self::Dynamic => "dynamic",
            Self::Resize => "resize",
            Self::Occlusion => "occlusion",
            Self::Capture => "capture",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum RendererSpikeFault {
    CallbackPanic,
    CaptureTimeout,
    SurfaceUnavailable,
}

impl RendererSpikeFault {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallbackPanic => "callback-panic",
            Self::CaptureTimeout => "capture-timeout",
            Self::SurfaceUnavailable => "surface-unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererSpikeOptions {
    pub candidate: RendererSpikeCandidate,
    pub track: RendererSpikeTrack,
    pub logical_size: u16,
    pub duration_ms: u64,
    pub out: PathBuf,
    pub inject_fault: Option<RendererSpikeFault>,
    pub runner_entry_micros: Option<u64>,
}

pub fn parse_logical_size(value: &str) -> std::result::Result<u16, String> {
    let size = value
        .parse::<u16>()
        .map_err(|_| "logical size must be 360 or 720".to_string())?;
    match size {
        360 | 720 => Ok(size),
        _ => Err("logical size must be 360 or 720".to_string()),
    }
}

pub fn run(options: RendererSpikeOptions) -> Result<()> {
    if options.duration_ms == 0 {
        return Err(GlorpError::Message(
            "renderer spike duration must be greater than zero".into(),
        ));
    }
    if options.candidate == RendererSpikeCandidate::Wgpu && !cfg!(feature = "renderer-spike-wgpu") {
        return Err(GlorpError::Message(
            "wgpu renderer spike requires --features renderer-spike-wgpu".into(),
        ));
    }
    match options.candidate {
        RendererSpikeCandidate::Smooth => {
            #[cfg(target_os = "macos")]
            {
                macos::run_smooth(options)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(GlorpError::Message(
                    "renderer spike native candidates are only available on macOS".into(),
                ))
            }
        }
        RendererSpikeCandidate::Wgpu => {
            #[cfg(all(target_os = "macos", feature = "renderer-spike-wgpu"))]
            {
                wgpu::run(options)
            }
            #[cfg(not(all(target_os = "macos", feature = "renderer-spike-wgpu")))]
            {
                Err(GlorpError::Message(
                    "wgpu renderer spike requires macOS and --features renderer-spike-wgpu".into(),
                ))
            }
        }
        RendererSpikeCandidate::Software => {
            #[cfg(target_os = "macos")]
            {
                software_host::run(options)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(GlorpError::Message(
                    "software renderer spike requires macOS".into(),
                ))
            }
        }
    }
}
