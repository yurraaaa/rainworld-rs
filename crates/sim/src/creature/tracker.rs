//! `Tracker`, the `AIModule` that holds what a realized AI knows of other creatures:
//! one `CreatureRepresentation` each, simple (the last tile seen) or elaborate (ghosts
//! that guess where an unseen creature went). The data and its world-free parts are
//! here; its `Update`, noticing, ghosts and forgetting run in `game_tracker.rs`.
//! Decompile: `Tracker.cs`. Behaviour: `rainworld-re/notes/creature-model.md` section 17.

use rw_random::UnityRandom;

use super::{CreatureId, WorldCoordinate};
use crate::world::{MovementConnection, MovementType};
use crate::{FloatExt, Vec2};

/// A representation's identity (the game keeps object references to it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepKey(pub u32);

/// What `EstimatedChanceOfFinding` reads of a representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sighting {
    /// The raw `visualContact` field.
    pub visual_contact: bool,
    /// `LowestGenerationAvailable`.
    pub lowest_generation: i32,
    /// `TicksSinceSeen` (after the corners seen around).
    pub ticks_since_seen: i32,
}

impl Sighting {
    /// `EstimatedChanceOfFinding`: 1 in sight, else a logistic falloff over the
    /// generations and ticks unseen, then `30 / (n - 7)`.
    #[must_use]
    pub fn estimated_chance_of_finding(self) -> f32 {
        let n = (self.lowest_generation as f32 * 10.0 + self.ticks_since_seen as f32) / 4.0;
        if self.visual_contact {
            return 1.0;
        }
        if n < 45.0 {
            let e = std::f32::consts::E.mathf_pow(-(n / 12.0 - 5.0));
            return (1.0 / -(1.0 + e) + 1.007).clamp(0.0, 1.0);
        }
        1.0 / (n - 7.0) * 30.0
    }
}

/// `Tracker.Ghost`: one guess where an unseen creature is.
#[derive(Debug, Clone, PartialEq)]
pub struct Ghost {
    /// Identity within its representation.
    pub serial: u32,
    pub pos: Vec2,
    pub vel: Vec2,
    pub coord: WorldCoordinate,
    pub last_coord: WorldCoordinate,
    pub forbidden_room_exit: WorldCoordinate,
    pub move_buffer: f32,
    pub push_forward: i32,
    pub generation: i32,
    pub connections_followed: i32,
    pub delete_me_next_frame: bool,
    pub stopped: bool,
    pub allowed_to_move_in_visible_areas: bool,
}

/// The tracker's ghost settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GhostParams {
    pub speed: f32,
    pub push: i32,
    pub push_speed: i32,
    pub dismissal_range: i32,
}

impl Ghost {
    /// The constructor at `pos` with `vel` for a creature at `at`: one draw for its move
    /// buffer.
    pub fn new(
        serial: u32,
        at: WorldCoordinate,
        pos: Vec2,
        vel: Vec2,
        params: GhostParams,
        rng: &mut UnityRandom,
    ) -> Self {
        let coord = at.wash_node();
        Self {
            serial,
            pos,
            vel,
            coord,
            last_coord: coord,
            forbidden_room_exit: WorldCoordinate::new(0, 0, 0, 0),
            move_buffer: rng.value(),
            push_forward: params.push,
            generation: 0,
            connections_followed: 0,
            delete_me_next_frame: false,
            stopped: params.speed == 0.0,
            allowed_to_move_in_visible_areas: false,
        }
    }

    /// `ConnectionLegal`'s check that the move does not turn straight back.
    #[must_use]
    pub fn not_turning_back(&self, connection: &MovementConnection) -> bool {
        let d = connection.dest.tile;
        connection.dest.tile != self.last_coord.tile
            && Self::clamp_at_one(self.coord.tile, d)
                != Self::clamp_at_one(self.coord.tile, self.last_coord.tile)
    }

    /// `IntVector2.ClampAtOne(a - b)`.
    fn clamp_at_one(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
        ((a.0 - b.0).clamp(-1, 1), (a.1 - b.1).clamp(-1, 1))
    }

    /// The score `Update` gives an outgoing connection: how far it turns from the
    /// velocity, doubled for each objection.
    #[must_use]
    pub fn connection_score(&self, connection: &MovementConnection, objections: u32) -> f32 {
        let (s, d) = (connection.start.tile, connection.dest.tile);
        let step = Vec2::new((d.0 - s.0) as f32, (d.1 - s.1) as f32);
        let mut score = step.distance(self.vel * 100.0);
        for _ in 0..objections {
            score *= 2.0;
        }
        if matches!(
            connection.kind,
            MovementType::DropToClimb | MovementType::DropToFloor
        ) {
            score *= 2.0;
        }
        score
    }
}

/// The best ghost of an elaborate representation: the game holds the object, which
/// outlives its removal from the list.
#[derive(Debug, Clone, PartialEq)]
pub enum BestGhost {
    Listed(u32),
    Removed(Box<Ghost>),
}

/// `ElaborateCreatureRepresentation`'s own state.
#[derive(Debug, Clone, PartialEq)]
pub struct ElaborateRep {
    pub ghosts: Vec<Ghost>,
    pub best_ghost: Option<BestGhost>,
    pub best_ghost_dirty: bool,
    pub max_ghosts: i32,
    pub lowest_generation_available: i32,
    pub last_visual_contact: bool,
    next_ghost: u32,
}

impl ElaborateRep {
    /// The constructor: one ghost on the creature's main chunk (one draw).
    pub fn new(
        at: WorldCoordinate,
        pos: Vec2,
        vel: Vec2,
        max_ghosts: i32,
        params: GhostParams,
        rng: &mut UnityRandom,
    ) -> Self {
        let ghost = Ghost::new(0, at, pos, vel, params, rng);
        Self {
            ghosts: vec![ghost],
            best_ghost: Some(BestGhost::Listed(0)),
            best_ghost_dirty: true,
            max_ghosts,
            lowest_generation_available: 0,
            last_visual_contact: false,
            next_ghost: 1,
        }
    }

    /// A new ghost's serial.
    pub fn next_serial(&mut self) -> u32 {
        let s = self.next_ghost;
        self.next_ghost += 1;
        s
    }

    /// `MaxGhosts`.
    #[must_use]
    pub fn max_ghosts_now(&self, chance_of_finding: f32) -> i32 {
        let n = (self.max_ghosts as f32 * chance_of_finding.mathf_pow(0.25)) as i32;
        n.clamp(2, self.max_ghosts)
    }

    /// Removes ghost `i`, keeping it as the best ghost if it was.
    pub fn remove_ghost(&mut self, i: usize) {
        let g = self.ghosts.remove(i);
        if self.best_ghost == Some(BestGhost::Listed(g.serial)) {
            self.best_ghost = Some(BestGhost::Removed(Box::new(g)));
        }
    }

    /// `bestGhost`.
    #[must_use]
    pub fn best(&self) -> Option<&Ghost> {
        match self.best_ghost.as_ref()? {
            BestGhost::Listed(s) => self.ghosts.iter().find(|g| g.serial == *s),
            BestGhost::Removed(g) => Some(g),
        }
    }

    /// `bestGhost`, mutably.
    pub fn best_mut(&mut self) -> Option<&mut Ghost> {
        match self.best_ghost.as_mut()? {
            BestGhost::Listed(s) => {
                let s = *s;
                self.ghosts.iter_mut().find(|g| g.serial == s)
            }
            BestGhost::Removed(g) => Some(g),
        }
    }
}

/// The kind of a `CreatureRepresentation`.
#[derive(Debug, Clone, PartialEq)]
pub enum RepKind {
    Simple { forget_when_not_visible: bool },
    Elaborate(ElaborateRep),
}

/// `Tracker.CreatureRepresentation`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureRep {
    pub key: RepKey,
    pub creature: CreatureId,
    pub forget_counter: i32,
    pub age: i32,
    pub dynamic_relationship: Option<super::DynamicRelationship>,
    /// The raw `visualContact` field (the `VisualContact` getter also checks the room).
    pub visual_contact: bool,
    pub gone_to_other_room_sense: bool,
    /// The raw `ticksSinceSeen`.
    pub ticks_since_seen: i32,
    pub delete_me_next_frame: bool,
    pub priority: f32,
    pub last_seen_coord: WorldCoordinate,
    pub kind: RepKind,
}

impl CreatureRep {
    /// The base constructor: in sight, at the creature's coordinate.
    #[must_use]
    pub fn new(
        key: RepKey,
        creature: CreatureId,
        at: WorldCoordinate,
        priority: f32,
        kind: RepKind,
    ) -> Self {
        Self {
            key,
            creature,
            forget_counter: 0,
            age: 0,
            dynamic_relationship: None,
            visual_contact: true,
            gone_to_other_room_sense: true,
            ticks_since_seen: 0,
            delete_me_next_frame: false,
            priority,
            last_seen_coord: at.wash_node(),
            kind,
        }
    }

    /// `TicksSinceSeen`.
    #[must_use]
    pub fn ticks_since_seen(&self, see_around_corners: i32) -> i32 {
        if self.ticks_since_seen < see_around_corners {
            0
        } else {
            self.ticks_since_seen - see_around_corners
        }
    }

    /// `LowestGenerationAvailable`.
    #[must_use]
    pub fn lowest_generation(&self) -> i32 {
        match &self.kind {
            RepKind::Simple { .. } => 1,
            RepKind::Elaborate(e) => e.lowest_generation_available,
        }
    }

    #[must_use]
    pub fn sighting(&self, see_around_corners: i32) -> Sighting {
        Sighting {
            visual_contact: self.visual_contact,
            lowest_generation: self.lowest_generation(),
            ticks_since_seen: self.ticks_since_seen(see_around_corners),
        }
    }

    /// `MakeVisible`.
    pub fn make_visible(&mut self) {
        self.ticks_since_seen = 0;
        self.visual_contact = true;
    }

    /// `Destroy`.
    pub fn destroy(&mut self) {
        self.delete_me_next_frame = true;
    }

    /// The current dynamic relationship, if established.
    #[must_use]
    pub fn current_relationship(&self) -> Option<super::Relationship> {
        self.dynamic_relationship.as_ref().map(|d| d.current)
    }
}

/// A representation as reported (MCP): its fields and what reading it now gives.
#[derive(Debug, Clone, PartialEq)]
pub struct RepReading {
    pub creature: CreatureId,
    pub elaborate: bool,
    pub age: i32,
    /// The raw `ticksSinceSeen`.
    pub ticks_since_seen: i32,
    pub forget_counter: i32,
    /// The `VisualContact` getter.
    pub visual_contact: bool,
    pub estimated_chance_of_finding: f32,
    /// `BestGuessForPosition()`.
    pub best_guess: WorldCoordinate,
    pub last_seen: WorldCoordinate,
    pub priority: f32,
    pub delete_me_next_frame: bool,
    /// An elaborate representation's ghosts.
    pub ghosts: Vec<Ghost>,
    pub relationship: Option<super::DynamicRelationship>,
}

/// `Tracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracker {
    /// `creatures`, in order.
    pub reps: Vec<CreatureRep>,
    /// Representations removed from the list that something still refers to (a look
    /// target, the casual aggression target).
    pub detached: Vec<CreatureRep>,
    next_key: u32,
    pub see_around_corners: i32,
    pub max_tracked_creatures: i32,
    /// -1 never forgets.
    pub frames_to_remember_creatures: i32,
    pub ghosts: GhostParams,
}

impl Tracker {
    /// The constructor (`useTrackedCreatures` false).
    #[must_use]
    pub fn new(
        see_around_corners: i32,
        max_tracked_creatures: i32,
        frames_to_remember_creatures: i32,
        ghosts: GhostParams,
    ) -> Self {
        Self {
            reps: Vec::new(),
            detached: Vec::new(),
            next_key: 0,
            see_around_corners,
            max_tracked_creatures,
            frames_to_remember_creatures,
            ghosts,
        }
    }

    /// `LizardAI`'s `Tracker(this, 10, 10, -1, 0.35f, 5, 5, 10)`.
    #[must_use]
    pub fn lizard() -> Self {
        Self::new(
            10,
            10,
            -1,
            GhostParams {
                speed: 0.35,
                push: 5,
                push_speed: 5,
                dismissal_range: 10,
            },
        )
    }

    /// A new representation's key.
    pub fn next_key(&mut self) -> RepKey {
        let k = RepKey(self.next_key);
        self.next_key += 1;
        k
    }

    /// The representation `key`, listed or detached.
    #[must_use]
    pub fn rep(&self, key: RepKey) -> Option<&CreatureRep> {
        self.reps
            .iter()
            .chain(&self.detached)
            .find(|r| r.key == key)
    }

    pub fn rep_mut(&mut self, key: RepKey) -> Option<&mut CreatureRep> {
        self.reps
            .iter_mut()
            .chain(&mut self.detached)
            .find(|r| r.key == key)
    }

    /// The listed representation's index.
    #[must_use]
    pub fn index_of(&self, key: RepKey) -> Option<usize> {
        self.reps.iter().position(|r| r.key == key)
    }

    /// `RepresentationForCreature(crit, false)`: the first listed one.
    #[must_use]
    pub fn representation_for(&self, creature: CreatureId) -> Option<&CreatureRep> {
        self.reps.iter().find(|r| r.creature == creature)
    }

    /// Removes listed representation `i`, keeping it while `held` refers to it.
    pub fn remove_rep(&mut self, i: usize, held: &[RepKey]) {
        let r = self.reps.remove(i);
        if held.contains(&r.key) {
            self.detached.push(r);
        }
    }

    /// Drops the detached representations nothing in `held` refers to any more.
    pub fn collect_detached(&mut self, held: &[RepKey]) {
        self.detached.retain(|r| held.contains(&r.key));
    }

    /// `EstimatedChanceOfFinding` of a representation.
    #[must_use]
    pub fn chance_of_finding(&self, rep: &CreatureRep) -> f32 {
        rep.sighting(self.see_around_corners)
            .estimated_chance_of_finding()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `EstimatedChanceOfFinding` (`Tracker.cs` 62-77): 1 in sight; a fresh simple
    /// representation (generation 1, 0 ticks) is `n = 2.5`; at `n = 45` the other branch.
    #[test]
    fn chance_of_finding() {
        let seen = Sighting {
            visual_contact: true,
            lowest_generation: 1,
            ticks_since_seen: 500,
        };
        assert_eq!(seen.estimated_chance_of_finding(), 1.0);
        let fresh = Sighting {
            visual_contact: false,
            ticks_since_seen: 0,
            ..seen
        };
        let n: f32 = 2.5;
        let expected = 1.0 / -(1.0 + std::f32::consts::E.powf(-(n / 12.0 - 5.0))) + 1.007;
        let got = fresh.estimated_chance_of_finding();
        assert!((got - expected).abs() < 1e-6, "{got} vs {expected}");
        assert!((got - 0.998_77).abs() < 1e-4);
        // n = (0 + 180) / 4 = 45: 30 / 38.
        let far = Sighting {
            lowest_generation: 0,
            ticks_since_seen: 180,
            ..fresh
        }
        .estimated_chance_of_finding();
        assert!((far - 30.0 / 38.0).abs() < 1e-6);
    }

    /// `TicksSinceSeen` counts only past the corners seen around (10 for lizards).
    #[test]
    fn ticks_since_seen_after_corners() {
        let mut r = CreatureRep::new(
            RepKey(0),
            CreatureId(1),
            WorldCoordinate::new(3, 4, 5, 2),
            1.0,
            RepKind::Simple {
                forget_when_not_visible: false,
            },
        );
        assert_eq!(r.last_seen_coord, WorldCoordinate::new(3, 4, 5, -1));
        r.ticks_since_seen = 9;
        assert_eq!(r.ticks_since_seen(10), 0);
        r.ticks_since_seen = 25;
        assert_eq!(r.ticks_since_seen(10), 15);
        assert_eq!(r.lowest_generation(), 1);
    }

    /// `MaxGhosts`: `maxGhosts * chance^0.25` clamped to 2..maxGhosts.
    #[test]
    fn max_ghosts() {
        let mut rng = UnityRandom::new(1);
        let p = Tracker::lizard().ghosts;
        let e = ElaborateRep::new(
            WorldCoordinate::new(0, 1, 1, -1),
            Vec2::ZERO,
            Vec2::ZERO,
            5,
            p,
            &mut rng,
        );
        assert_eq!(e.max_ghosts_now(1.0), 5);
        assert_eq!(e.max_ghosts_now(0.5), 4);
        assert_eq!(e.max_ghosts_now(0.0), 2);
    }

    /// The ghost constructor draws its move buffer, once.
    #[test]
    fn ghost_constructor_draws_once() {
        let mut rng = UnityRandom::new(7);
        let mut probe = UnityRandom::new(7);
        let g = Ghost::new(
            0,
            WorldCoordinate::new(2, 3, 4, 1),
            Vec2::ZERO,
            Vec2::ZERO,
            Tracker::lizard().ghosts,
            &mut rng,
        );
        assert_eq!(g.move_buffer, probe.value());
        assert_eq!(rng.state(), probe.state());
        assert!(!g.stopped);
        assert_eq!(g.push_forward, 5);
        assert_eq!(g.coord.node, -1);
    }
}
