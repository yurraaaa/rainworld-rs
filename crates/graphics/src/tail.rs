//! `TailSegment`: a body part kept within `connection_rad` of the previous segment
//! (or, for the first, of a point).

use rw_assets::Room;
use rw_sim::Vec2;

use crate::body_part::BodyPart;

/// One tail segment.
#[derive(Debug, Clone, PartialEq)]
pub struct TailSegment {
    pub part: BodyPart,
    /// Allowed distance to what it hangs on.
    pub connection_rad: f32,
    /// Share of a stretch correction applied to the previous segment instead of this one.
    pub affect_previous: f32,
    /// 1 when slack, down to 0.2 when pulled far beyond `connection_rad`.
    pub stretched: f32,
    /// `pullInPreviousPosition`: the correction also moves the previous segment (else
    /// only its velocity).
    pub pull_in_previous_position: bool,
}

/// What a segment hangs on.
pub enum TailAnchor<'a> {
    /// The first segment's `connectedPoint`.
    Point(Vec2),
    /// The previous segment (`connectedSegment`), which the correction also pulls.
    Segment(&'a mut BodyPart),
}

impl TailSegment {
    /// A base-game segment (surface friction 0.85, no air friction).
    #[must_use]
    pub fn new(rad: f32, connection_rad: f32, affect_previous: f32) -> Self {
        Self {
            part: BodyPart::new(rad, 0.85, 1.0),
            connection_rad,
            affect_previous,
            stretched: 1.0,
            pull_in_previous_position: true,
        }
    }

    /// `StretchedRad`.
    #[must_use]
    pub fn stretched_rad(&self) -> f32 {
        self.part.rad * self.stretched
    }

    /// `TailSegment.Update`: move, then pull back within `connection_rad` of the anchor
    /// (sharing the correction with a previous segment by `affect_previous`, which
    /// moves both its position and velocity), then push out of terrain.
    pub fn update(&mut self, room: &Room, anchor: TailAnchor<'_>) {
        let p = &mut self.part;
        p.last_pos = p.pos;
        p.pos += p.vel;
        p.vel *= p.air_friction;
        self.stretched = 1.0;
        match anchor {
            TailAnchor::Segment(prev) => {
                let dist = p.pos.distance(prev.pos);
                if dist >= self.connection_rad {
                    let dir = p.pos.direction_to(prev.pos);
                    let own = dir * ((self.connection_rad - dist) * (1.0 - self.affect_previous));
                    let other = dir * ((self.connection_rad - dist) * self.affect_previous);
                    p.pos -= own;
                    p.vel -= own;
                    if self.pull_in_previous_position {
                        prev.pos += other;
                    }
                    prev.vel += other;
                    self.stretched = Self::stretch_for(self.connection_rad, dist);
                }
            }
            TailAnchor::Point(point) => {
                let dist = p.pos.distance(point);
                if dist >= self.connection_rad {
                    let v = p.pos.direction_to(point) * (self.connection_rad - dist);
                    p.pos -= v;
                    p.vel -= v;
                    self.stretched = Self::stretch_for(self.connection_rad, dist);
                }
            }
        }
        self.part.push_out_of_terrain(room);
    }

    /// `stretched` for a segment found `dist` from its anchor.
    fn stretch_for(connection_rad: f32, dist: f32) -> f32 {
        ((connection_rad / (dist * 0.5) + 2.0) / 3.0).clamp(0.2, 1.0)
    }
}
