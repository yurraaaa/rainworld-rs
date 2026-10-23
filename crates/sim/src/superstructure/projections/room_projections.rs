//! A realized room's superstructure projections in its update list: the
//! `SuperStructureProjector`s with their parts, the `ProjectedScanLines` with their lines
//! and the `StarMatrix`es, and the `ProjectedCircle`s of the coral. Each list is kept in update-list order (an object's
//! `update_order` grows with `AddObject`).
//!
//! Mirrors: Room.cs (updateList: SuperStructureProjector, ProjectedScanLines, StarMatrix, ProjectedCircle)

use crate::superstructure::projections::projected_circle::ProjectedCircle;
use crate::superstructure::projections::projector::SuperStructureProjector;
use crate::superstructure::projections::scan_lines::ProjectedScanLines;
use crate::superstructure::projections::star_matrix::StarMatrix;

/// An object of [`RoomProjections`], by its place in the update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectionRef {
    Projector(u64),
    /// A part of the projector at `projector`.
    Part {
        projector: u64,
        part: u64,
    },
    ScanLines(u64),
    /// A line of the `ProjectedScanLines` at `owner`.
    ScanLine {
        owner: u64,
        line: u64,
    },
    Stars(u64),
    Circle(u64),
}

/// A realized room's projections.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomProjections {
    pub projectors: Vec<SuperStructureProjector>,
    pub scan_lines: Vec<ProjectedScanLines>,
    pub star_matrices: Vec<StarMatrix>,
    /// Kept after they are slated: other circles' connections still read them.
    pub circles: Vec<ProjectedCircle>,
}

impl RoomProjections {
    /// Drops what `Destroy` slated: the projectors and scan line sets (their parts and
    /// lines stay while the owner's lists still count them, see the owners).
    pub fn remove_slated(&mut self) {
        self.projectors.retain(|p| !p.slated);
        self.scan_lines.retain(|p| !p.slated);
        for p in &mut self.scan_lines {
            p.remove_slated();
        }
    }

    /// Every object with its place in the update list.
    pub(crate) fn objects(&self) -> Vec<(u64, ProjectionRef)> {
        let mut list: Vec<(u64, ProjectionRef)> = Vec::new();
        for p in &self.projectors {
            list.push((p.update_order, ProjectionRef::Projector(p.update_order)));
            list.extend(p.parts.iter().filter(|part| !part.slated).map(|part| {
                (
                    part.update_order,
                    ProjectionRef::Part {
                        projector: p.update_order,
                        part: part.update_order,
                    },
                )
            }));
        }
        for p in &self.scan_lines {
            list.push((p.update_order, ProjectionRef::ScanLines(p.update_order)));
            list.extend(p.lines.iter().filter(|l| !l.slated).map(|l| {
                (
                    l.update_order,
                    ProjectionRef::ScanLine {
                        owner: p.update_order,
                        line: l.update_order,
                    },
                )
            }));
        }
        for s in &self.star_matrices {
            list.push((s.update_order, ProjectionRef::Stars(s.update_order)));
        }
        list.extend(
            self.circles
                .iter()
                .filter(|c| !c.slated)
                .map(|c| (c.update_order, ProjectionRef::Circle(c.update_order))),
        );
        list
    }

    /// The places in the update list of the objects that have sprites.
    pub fn drawn_orders(&self) -> impl Iterator<Item = u64> + '_ {
        let parts = self
            .projectors
            .iter()
            .flat_map(|p| p.parts.iter().filter(|part| !part.slated))
            .map(|part| part.update_order);
        let lines = self
            .scan_lines
            .iter()
            .flat_map(|p| p.lines.iter().filter(|l| !l.slated))
            .map(|l| l.update_order);
        parts
            .chain(lines)
            .chain(self.star_matrices.iter().map(|s| s.update_order))
            .chain(
                self.circles
                    .iter()
                    .filter(|c| !c.slated)
                    .map(|c| c.update_order),
            )
    }
}
