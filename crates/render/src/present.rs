//! The game's screen resolution and how its frame is shown in the window. GPU-free.
//!
//! The game draws every frame into a render texture of its `ScreenSize` option
//! (`FScreen.renderTexture`, 1366x768 by default), shown 1:1 on a full-screen UI image
//! in a window of that same size (`Screen.SetResolution`). In full-screen mode Unity
//! scales that window to the display, keeping its aspect with black bars
//! (`rainworld-re/notes/room-rendering.md`, "Resolution and scaling").
//!
//! Mirrors: FScreen.cs

use std::fmt;
use std::str::FromStr;

/// One of the game's screen sizes (`Options.screenResolutions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    /// `Options.screenResolutions`, in the game's order (4:3, 16:9, 16:9, 5:3, 16:10).
    pub const CHOICES: [Self; 5] = [
        Self::new(1024, 768),
        Self::new(1366, 768),
        Self::new(1360, 768),
        Self::new(1280, 768),
        Self::new(1229, 768),
    ];

    /// The game's default (`resolution = 1`; 4 on a Steam Deck).
    pub const DEFAULT: Self = Self::CHOICES[1];

    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub fn size(self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The size in pixels as floats (the room camera's `sSize`).
    #[must_use]
    pub fn size_f32(self) -> [f32; 2] {
        [self.width as f32, self.height as f32]
    }
}

impl Default for Resolution {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl FromStr for Resolution {
    type Err = String;

    /// `WIDTHxHEIGHT`, one of [`Resolution::CHOICES`].
    fn from_str(s: &str) -> Result<Self, String> {
        let parsed = s
            .split_once('x')
            .and_then(|(w, h)| Some(Self::new(w.parse().ok()?, h.parse().ok()?)));
        match parsed {
            Some(r) if Self::CHOICES.contains(&r) => Ok(r),
            _ => {
                let choices: Vec<String> = Self::CHOICES.iter().map(ToString::to_string).collect();
                Err(format!(
                    "invalid resolution {s:?}; the game offers {}",
                    choices.join(", ")
                ))
            }
        }
    }
}

/// How the frame's pixels are sampled when shown in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentFilter {
    /// Shown 1:1 (the game's window at its own size; Futile's point-filtered texture).
    Nearest,
    /// Scaled (Unity's full-screen-window scaling; the filter is inferred).
    Linear,
}

/// Where the frame lands in a window: the largest rectangle of the frame's aspect that
/// fits, at any (fractional) scale, centred, with black bars around it. Window pixels,
/// origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub filter: PresentFilter,
}

impl PresentRect {
    /// `None` when either size is zero.
    #[must_use]
    pub fn fit(frame: (u32, u32), window: (u32, u32)) -> Option<Self> {
        if frame.0 == 0 || frame.1 == 0 || window.0 == 0 || window.1 == 0 {
            return None;
        }
        let scale = (window.0 as f64 / frame.0 as f64).min(window.1 as f64 / frame.1 as f64);
        let width = ((frame.0 as f64 * scale).round() as u32).clamp(1, window.0);
        let height = ((frame.1 as f64 * scale).round() as u32).clamp(1, window.1);
        let filter = if (width, height) == frame {
            PresentFilter::Nearest
        } else {
            PresentFilter::Linear
        };
        Some(Self {
            x: (window.0 - width) / 2,
            y: (window.1 - height) / 2,
            width,
            height,
            filter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u32, y: u32, width: u32, height: u32, filter: PresentFilter) -> PresentRect {
        PresentRect {
            x,
            y,
            width,
            height,
            filter,
        }
    }

    const FRAME: (u32, u32) = (1366, 768);

    #[test]
    fn same_size_window_shows_the_frame_one_to_one() {
        assert_eq!(
            PresentRect::fit(FRAME, FRAME),
            Some(rect(0, 0, 1366, 768, PresentFilter::Nearest))
        );
    }

    #[test]
    fn integer_scale_fills_the_window() {
        assert_eq!(
            PresentRect::fit(FRAME, (2732, 1536)),
            Some(rect(0, 0, 2732, 1536, PresentFilter::Linear))
        );
    }

    #[test]
    fn fractional_scales_keep_the_aspect_with_bars() {
        // 1080p: 1920 / 1366 = 1.4056 < 1080 / 768 = 1.40625, width-limited; 1079.47 px
        // tall, so one row of bar at the bottom.
        assert_eq!(
            PresentRect::fit(FRAME, (1920, 1080)),
            Some(rect(0, 0, 1920, 1079, PresentFilter::Linear))
        );
        // 16:10 2560x1600: width-limited at 1.874, bars above and below.
        assert_eq!(
            PresentRect::fit(FRAME, (2560, 1600)),
            Some(rect(0, 80, 2560, 1439, PresentFilter::Linear))
        );
        // 4:3 1024x768: width-limited at 0.75 (downscaled), bars above and below.
        assert_eq!(
            PresentRect::fit(FRAME, (1024, 768)),
            Some(rect(0, 96, 1024, 576, PresentFilter::Linear))
        );
        // Ultrawide 3440x1440: height-limited at 1.875, bars left and right.
        assert_eq!(
            PresentRect::fit(FRAME, (3440, 1440)),
            Some(rect(439, 0, 2561, 1440, PresentFilter::Linear))
        );
    }

    #[test]
    fn a_four_by_three_frame_is_pillarboxed_on_a_wide_window() {
        assert_eq!(
            PresentRect::fit((1024, 768), (1920, 1080)),
            Some(rect(240, 0, 1440, 1080, PresentFilter::Linear))
        );
    }

    #[test]
    fn empty_sizes_have_no_rect() {
        assert_eq!(PresentRect::fit(FRAME, (0, 768)), None);
        assert_eq!(PresentRect::fit((0, 0), FRAME), None);
    }

    #[test]
    fn resolution_parses_only_the_games_choices() {
        assert_eq!("1366x768".parse(), Ok(Resolution::DEFAULT));
        assert_eq!("1024x768".parse(), Ok(Resolution::new(1024, 768)));
        assert!("1920x1080".parse::<Resolution>().is_err());
        assert!("1366".parse::<Resolution>().is_err());
        assert_eq!(Resolution::DEFAULT.to_string(), "1366x768");
    }
}
