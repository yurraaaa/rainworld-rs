//! `PlayerGraphics.PlayerObjectLooker` (236851): what the slugcat looks at among its
//! room's objects.

use rw_assets::CreatureType;
use rw_random::UnityRandom;
use rw_sim::creature::{CreatureId, Relationship, RelationshipType};
use rw_sim::{FloatExt, PlayerSurroundings, RoomExt, Vec2};

/// The object looker (`lookAtPoint` is only set by DLC and tutorial callers).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ObjectLooker {
    /// `currentMostInteresting`.
    pub current: Option<CreatureId>,
    /// `timeLookingAtThis`.
    pub time: i32,
}

/// A realized object as the looker reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seen {
    kind: CreatureType,
    /// `obj.room == player.room`.
    here: bool,
    /// `bodyChunks[0]` (also `DangerPos`, the main chunk, for flies and overseers).
    pos: Vec2,
    vel: Vec2,
    conscious: bool,
    slated: bool,
}

impl Seen {
    fn of(around: &PlayerSurroundings<'_>, id: CreatureId) -> Option<Self> {
        let c = around.world.creature(id);
        if let Some(f) = c.fly() {
            return Some(Self {
                kind: c.kind(),
                here: f.room == Some(around.room),
                pos: f.pos(),
                vel: f.chunk().vel,
                conscious: f.conscious(),
                slated: f.slated,
            });
        }
        let o = c.overseer_body()?;
        let chunk = o.body.chunks.first()?;
        Some(Self {
            kind: c.kind(),
            here: o.in_room && c.pos.room == around.room,
            pos: chunk.pos,
            vel: chunk.vel,
            conscious: c.state.alive,
            slated: false,
        })
    }
}

impl ObjectLooker {
    /// `mostInterestingLookPoint`.
    pub(crate) fn look_point(
        &self,
        around: &PlayerSurroundings<'_>,
        head: Vec2,
        look_direction: Vec2,
    ) -> Vec2 {
        match self.current.and_then(|id| Seen::of(around, id)) {
            Some(s) => s.pos,
            None => head + look_direction * 100.0,
        }
    }

    pub fn look_at_nothing(&mut self) {
        self.current = None;
        self.time = 0;
    }

    pub fn look_at_object(&mut self, id: CreatureId) {
        self.current = Some(id);
        self.time = 0;
    }

    /// `Update`: after a tick of looking, the most interesting object (above the
    /// current interest, which gets 0.5 more before 8 ticks and 0.5 less after) is
    /// looked at; little interest makes it look away at even odds.
    pub(crate) fn update(
        &mut self,
        around: &PlayerSurroundings<'_>,
        player_pos: Vec2,
        rng: &mut UnityRandom,
    ) {
        self.time += 1;
        if self
            .current
            .and_then(|id| Seen::of(around, id))
            .is_some_and(|s| s.slated)
        {
            self.current = None;
        }
        if self.time <= 1 {
            return;
        }
        let bonus = if self.time < 8 { 0.5 } else { -0.5 };
        let mut num = self.interest(around, self.current, player_pos) + bonus;
        for &id in &around.objects {
            let interest = self.interest(around, Some(id), player_pos);
            if interest > num {
                self.time = 0;
                self.current = Some(id);
                num = interest;
            }
        }
        if num < 0.2 && rng.value() < 0.5 {
            self.look_at_nothing();
        }
    }

    /// `HowInterestingIsThisObject`: 0 for nothing, an object elsewhere or one in the
    /// hands; else 1 plus the relationship's intensity (and an afraid bonus), more when
    /// moving and in sight within 400 px, divided by a slowly growing power of the
    /// distance.
    fn interest(
        &self,
        around: &PlayerSurroundings<'_>,
        id: Option<CreatureId>,
        player_pos: Vec2,
    ) -> f32 {
        let Some(id) = id else { return 0.0 };
        let Some(seen) = Seen::of(around, id) else {
            return 0.0;
        };
        if !seen.here {
            return 0.0;
        }
        if around.grasps.contains(&Some(id)) {
            return 0.0;
        }
        let mut num = 1.0;
        if let Some(r) = Relationship::of_slugcat_to(seen.kind) {
            num += r.intensity;
            if r.kind == RelationshipType::Afraid && seen.conscious {
                num += 3.0 + r.intensity * 3.0;
            }
        }
        let in_sight = around
            .world
            .realized(around.room)
            .is_some_and(|r| r.room.visual_contact(player_pos, seen.pos));
        if player_pos.dist_less(seen.pos, 400.0) && in_sight {
            num *= (seen.vel.length() + 1.0).lerp(2.0, 0.5);
        }
        num / player_pos
            .distance(seen.pos)
            .mathf_pow(1.5)
            .lerp(1.0, 0.995)
    }
}
