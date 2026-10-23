//! The game's side of the superstructure projections: `Room.Loaded`'s `ProjectedStars`
//! (a `StarMatrix` joins the update list), each projection object's turn in the room's
//! update list (the `SuperStructureProjector` and its parts, the `ProjectedScanLines` and
//! its lines, the `StarMatrix`), and the shader's `_gridOffset`. The projectors and scan
//! lines themselves are made by `Room.NowViewed` ([`crate::effects::particles::turn`]). Types:
//! [`crate::superstructure::projections::projector`], [`crate::superstructure::projections::scan_lines`], [`crate::superstructure::projections::star_matrix`].
//!
//! Mirrors: Room.cs (Loaded: ProjectedStars), SuperStructureProjector.cs (UpdateOffset), StarMatrix.cs (Update)

use crate::Vec2;
use crate::game::{RainWorldGame, RoomObject};
use crate::superstructure::projections::projector::{PartKind, ProjectorFrame, SingleGlyph};
use crate::superstructure::projections::room_projections::ProjectionRef;
use crate::superstructure::projections::star_matrix::StarMatrix;

impl RainWorldGame {
    /// `Room.Loaded`'s `new StarMatrix(placedObject)` for a `ProjectedStars` at `pos`
    /// whose handle makes `rad`.
    pub(crate) fn add_star_matrix(&mut self, index: i32, pos: Vec2, rad: f32) {
        let mut stars = StarMatrix::new(0, pos, rad, &mut self.rng);
        stars.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.projections.star_matrices.push(stars);
        }
    }

    /// Room `index`'s projections in its update list.
    pub(crate) fn room_projection_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.projections
                .objects()
                .into_iter()
                .map(|(order, p)| (order, RoomObject::Projection(p)))
                .collect()
        })
    }

    /// What the projectors read of the game at their turn in room `index`.
    fn projector_frame(&self, index: i32) -> Option<ProjectorFrame> {
        let r = self.world.realized(index)?;
        let viewed = index == self.camera_room;
        Some(ProjectorFrame {
            electric_power: r.electric_power(&self.world.rain_cycle),
            being_viewed: viewed,
            camera_position: viewed.then_some(self.camera_view.current),
        })
    }

    /// The `Update` of projection object `p` in room `index`.
    pub(crate) fn run_projection(&mut self, index: i32, p: ProjectionRef) {
        let Some(frame) = self.projector_frame(index) else {
            return;
        };
        let mut seq = self.world.update_seq();
        let rng = &mut self.rng;
        if let Some(r) = self.world.realized_mut(index) {
            let list = &mut r.projections;
            match p {
                ProjectionRef::Projector(order) => {
                    if let Some(proj) = list.projectors.iter_mut().find(|q| q.update_order == order)
                        && !proj.slated
                    {
                        proj.update(&frame, &mut seq, rng);
                    }
                }
                ProjectionRef::Part { projector, part } => {
                    if let Some(proj) = list
                        .projectors
                        .iter_mut()
                        .find(|q| q.update_order == projector)
                    {
                        proj.update_part(part, &mut seq, rng);
                    }
                }
                ProjectionRef::ScanLines(order) => {
                    if let Some(lines) =
                        list.scan_lines.iter_mut().find(|q| q.update_order == order)
                        && !lines.slated
                    {
                        lines.update(frame.electric_power, frame.being_viewed, &mut seq, rng);
                    }
                }
                ProjectionRef::ScanLine { owner, line } => {
                    if let Some(lines) =
                        list.scan_lines.iter_mut().find(|q| q.update_order == owner)
                    {
                        lines.update_line(line);
                    }
                }
                ProjectionRef::Circle(order) => {
                    let can_host = r.coral.as_ref().is_some_and(|c| {
                        list.circles
                            .iter()
                            .find(|q| q.update_order == order)
                            .is_some_and(|q| c.circle_owner_can_host(q.owner))
                    });
                    if let Some(circle) = list.circles.iter_mut().find(|q| q.update_order == order)
                        && !circle.slated
                    {
                        circle.update(can_host, rng);
                    }
                }
                ProjectionRef::Stars(order) => {
                    if let Some(stars) = list
                        .star_matrices
                        .iter_mut()
                        .find(|q| q.update_order == order)
                    {
                        stars.update(rng);
                    }
                }
            }
        }
        self.world.set_update_seq(seq);
    }

    /// The first draw after each `ProjectedCircle.Update` of the camera room runs its
    /// `UpdateDepth`: `rCam.DepthAtCoordinate` at the owner's centre (sky, 1, without a
    /// camera picture).
    pub fn update_circle_depths(&mut self) {
        let index = self.camera_room;
        let image = self.camera_image.as_ref().filter(|c| c.room == index);
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(coral) = r.coral.as_ref() else {
            return;
        };
        let depths: Vec<(u64, f32)> = r
            .projections
            .circles
            .iter()
            .filter(|c| c.update_depth && !c.slated)
            .map(|c| {
                let centre = coral.circle_center(c.owner, c.index, 1.0);
                (
                    c.update_order,
                    centre.map_or(1.0, |at| image.map_or(1.0, |i| i.depth_at_coordinate(at))),
                )
            })
            .collect();
        for (order, depth) in depths {
            if let Some(c) = r
                .projections
                .circles
                .iter_mut()
                .find(|c| c.update_order == order)
            {
                c.apply_depth(depth);
            }
        }
    }

    /// Drops the projections destroyed during room `index`'s update.
    pub(crate) fn remove_slated_projections(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            r.projections.remove_slated();
        }
    }

    /// The `_gridOffset` the last drawn `SingleGlyph` of the camera room's projector
    /// sets at `time_stacker` between ticks (a glyph draws it while the projector is
    /// visible and the glyph active); `None` when none is drawn, the shader keeps the
    /// last value it was given.
    #[must_use]
    pub fn projection_grid_offset(&self, time_stacker: f32) -> Option<Vec2> {
        let r = self.world.realized(self.camera_room)?;
        r.projections
            .projectors
            .iter()
            .rfind(|p| {
                p.visible
                    && !p.slated
                    && p.parts.iter().any(|part| {
                        !part.slated
                            && matches!(
                                part.kind,
                                PartKind::Single(SingleGlyph {
                                    inactive: false,
                                    ..
                                })
                            )
                    })
            })
            .map(|p| p.grid_offset(time_stacker))
    }
}

#[cfg(test)]
#[path = "tests/turn.rs"]
mod tests;
