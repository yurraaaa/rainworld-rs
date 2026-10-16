//! How a body's condition changes its debug drawing.

use crate::Color;

/// Condition of a creature, as drawn by [`Renderer::draw_object`](crate::Renderer::draw_object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BodyStatus {
    pub stunned: bool,
    pub dead: bool,
}

/// Colours of one body for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyStyle {
    /// Chunk fills, cycled by chunk index (head, hips, ...).
    pub fill: [Color; 2],
    pub edge: Color,
    pub edge_width: f32,
    pub rod: Color,
    pub contact: Color,
    /// An X drawn over each chunk, for dead bodies.
    pub cross: Option<Color>,
}

/// Stun outline pulse rate, Hz.
const STUN_PULSE_HZ: f32 = 3.0;

impl BodyStatus {
    pub const ALIVE: Self = Self {
        stunned: false,
        dead: false,
    };

    /// Style at `time` seconds (any running clock; only the stun pulse uses it).
    /// Dead wins over stunned; stunned dims the fill and pulses the outline.
    #[must_use]
    pub fn style(self, time: f32) -> BodyStyle {
        let normal = BodyStyle {
            fill: [
                Color::rgba(0.95, 0.95, 1.0, 0.35),
                Color::rgba(0.75, 0.85, 1.0, 0.30),
            ],
            edge: Color::rgb(0.95, 0.95, 1.0),
            edge_width: 1.5,
            rod: Color::rgb(1.0, 0.55, 0.25),
            contact: Color::rgb(0.3, 1.0, 0.5),
            cross: None,
        };
        if self.dead {
            let grey = Color::rgb(0.5, 0.5, 0.52);
            BodyStyle {
                fill: [Color::rgba(0.45, 0.45, 0.47, 0.35); 2],
                edge: grey,
                rod: grey,
                contact: grey,
                cross: Some(Color::rgb(0.95, 0.25, 0.25)),
                ..normal
            }
        } else if self.stunned {
            let pulse = 0.5 + 0.5 * (std::f32::consts::TAU * STUN_PULSE_HZ * time).sin();
            BodyStyle {
                fill: normal.fill.map(|c| c.with_alpha(c.0[3] * 0.5)),
                edge: Color::rgb(0.45, 0.42, 0.3).lerp(Color::rgb(1.0, 0.85, 0.2), pulse),
                edge_width: 1.5 + 1.5 * pulse,
                ..normal
            }
        } else {
            normal
        }
    }
}

impl BodyStyle {
    /// The fill of a living body in `color` (a dead one keeps its grey).
    #[must_use]
    pub fn tinted(self, color: Color, dead: bool) -> Self {
        if dead {
            return self;
        }
        Self {
            fill: [color.with_alpha(0.35), color.with_alpha(0.25)],
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alive_is_the_plain_style() {
        let s = BodyStatus::ALIVE.style(0.3);
        assert_eq!(s, BodyStatus::default().style(0.0));
        assert!(s.cross.is_none());
    }

    #[test]
    fn stun_dims_and_pulses() {
        let alive = BodyStatus::ALIVE.style(0.0);
        let stunned = BodyStatus {
            stunned: true,
            dead: false,
        };
        let a = stunned.style(0.0);
        assert!(a.fill[0].0[3] < alive.fill[0].0[3]);
        // A quarter period apart: crest vs midpoint.
        let crest = stunned.style(1.0 / (4.0 * STUN_PULSE_HZ));
        assert!(crest.edge_width > a.edge_width);
        assert_ne!(crest.edge, a.edge);
    }

    #[test]
    fn dead_wins_and_is_crossed() {
        let s = BodyStatus {
            stunned: true,
            dead: true,
        }
        .style(0.1);
        assert!(s.cross.is_some());
        assert_eq!(s.fill[0], s.fill[1]);
    }
}
