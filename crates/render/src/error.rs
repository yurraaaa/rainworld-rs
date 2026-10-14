//! Renderer errors.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Error from creating or using a renderer.
#[derive(Debug)]
#[non_exhaustive]
pub enum RenderError {
    /// Backend initialisation failed at `stage` (surface, adapter, device, ...).
    Init {
        stage: &'static str,
        source: Box<dyn Error + Send + Sync>,
    },
    /// The surface supports no usable texture format.
    NoSurfaceFormat,
    /// The surface reported a validation error while acquiring a frame.
    SurfaceValidation,
    /// The surface cannot be copied from, so frames cannot be captured.
    CaptureUnsupported,
    /// Saving a captured frame failed.
    Capture { path: PathBuf, source: io::Error },
    /// Reading a frame back from the GPU failed.
    Readback(io::Error),
}

impl RenderError {
    pub(crate) fn init(stage: &'static str, source: impl Error + Send + Sync + 'static) -> Self {
        Self::Init {
            stage,
            source: Box::new(source),
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Init { stage, .. } => write!(f, "renderer initialisation failed: {stage}"),
            Self::NoSurfaceFormat => write!(f, "surface supports no texture format"),
            Self::SurfaceValidation => write!(f, "surface validation error on frame acquire"),
            Self::CaptureUnsupported => write!(f, "frame capture is not supported by this surface"),
            Self::Capture { path, .. } => write!(f, "failed to save frame to {}", path.display()),
            Self::Readback(_) => write!(f, "failed to read the frame back"),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Init { source, .. } => Some(source.as_ref()),
            Self::Capture { source, .. } | Self::Readback(source) => Some(source),
            Self::NoSurfaceFormat | Self::SurfaceValidation | Self::CaptureUnsupported => None,
        }
    }
}
