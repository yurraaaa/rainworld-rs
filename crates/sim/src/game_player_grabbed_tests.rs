//! A green lizard biting and holding the player in one hall. Expected values from
//! `Lizard.cs` (`Bite`), `Player.cs` (`Grabbed`, `Update`'s `dangerGrasp` block,
//! `ThrowToGetFree`, `DangerGraspPickup`, `Stun`, `Die`), `Creature.cs` (`Violence`,
//! `Update`), `Weapon.cs` (`Thrown`, `Update`'s `meleeHitChunk`) and `RainWorldGame.cs`
//! (`GameOver`). The slugcat template resists 1 and dies at 1 damage; a bite on the
//! player is 1.5.

use rw_assets::AbstractObjectType;

use crate::creature::lizard::LizardGraphicsLink;
use crate::creature::{CreatureId, LizardAnimation};
use crate::game::lizard_hits::ChunkRef;
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::{HIPS, Input, RainWorldGame, SoundId, Sounds};

const THROW: Input = Input {
    x: 0,
    y: 0,
    jump: false,
    map: false,
    pickup: false,
    throw: true,
};

const PICKUP: Input = Input {
    pickup: true,
    throw: false,
    ..THROW
};

impl RainWorldGame {
    /// Advances the stream until the next draw satisfies `want`.
    fn stream_until_next(&mut self, want: fn(f32) -> bool) {
        loop {
            let mut p = self.rng.clone();
            if want(p.value()) {
                return;
            }
            self.rng.value();
        }
    }

    /// The lizard's breed data with another `biteDamageChance`.
    fn set_bite_damage_chance(&mut self, l: CreatureId, chance: f32) {
        let mut params = self.world.lizard_ref(l).params.clone();
        params.bite_damage_chance = chance;
        self.world.lizard_at(l).params = Box::leak(Box::new(params));
    }

    /// A lizard two tiles right of the player, facing it for 40 ticks (it eats what
    /// it sees), whose bite hurts with `chance`, and its bite on the player's hips.
    fn bitten(&mut self, chance: f32) -> CreatureId {
        let l = self.still_lizard((5, 1), -1.0, 40);
        self.set_bite_damage_chance(l, chance);
        let hips = ChunkRef {
            owner: Entity::Creature(self.player_id),
            index: HIPS,
        };
        self.lizard_bite(l, Some(hips));
        l
    }

    /// A rock (or spear) realized at `tile` in the player's hand 0.
    fn armed(&mut self, kind: AbstractObjectType, tile: (i32, i32)) -> ObjectId {
        let id = self
            .dev_spawn_object(kind, 0, tile, false)
            .expect("spawned");
        self.slugcat_grab(Entity::Object(id), 0, &mut ());
        assert_eq!(self.player_grasps()[0], Some(Entity::Object(id)));
        id
    }
}

#[test]
fn a_bite_that_does_not_hurt_holds_the_player_in_a_danger_grasp() {
    // `Bite` on prey it eats: grabbed (dominance drawn), `BitCreature`'s ShakePrey (its
    // `Range`), the damage roll missed, `Lizard_Jaws_Grab_Player`. `Player.Grabbed` makes
    // the grasp the danger grasp, the body goes through floors.
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((5, 1), -1.0, 40);
    g.set_bite_damage_chance(l, 0.0);
    let shake = g.world.lizard_ref(l).params.shake_prey;
    let mut expect = g.rng.clone();
    expect.value();
    expect.range_i32(shake, shake * 2);
    expect.value();
    let head = g.world.lizard_ref(l).head().pos;
    let room = g.lizard_room(l);
    let mut sounds = if room == g.camera_room {
        g.sounds.clone()
    } else {
        Sounds::default()
    };
    let hips = ChunkRef {
        owner: Entity::Creature(g.player_id),
        index: HIPS,
    };
    g.lizard_bite(l, Some(hips));
    sounds.play_on_object(
        SoundId::LizardJawsGrabPlayer,
        head,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    assert_eq!(g.rng.state(), expect.state());
    let grasp = g.world.creature(l).grasps[0].expect("held");
    assert_eq!(grasp.grabbed, Entity::Creature(g.player_id));
    assert_eq!(g.player_ref().danger_grasp().grasp, Some(grasp));
    assert_eq!(g.player_ref().danger_grasp().time, 0);
    assert!(
        g.player_ref()
            .body
            .chunks
            .iter()
            .all(|c| c.go_through_floors)
    );
    assert_eq!(g.world.lizard_ref(l).animation, LizardAnimation::ShakePrey);
    assert!(!g.player_ref().is_dead());
    // Next tick: the time is 1 and the pacifying hold stuns the player at 10.
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().danger_grasp().time, 1);
    assert_eq!(g.player_ref().stun_ticks(), 10);
    assert!(g.player_ref().is_stunned());
}

#[test]
fn a_bite_that_hurts_kills_the_player_with_the_lizards_kill_tag() {
    // `Violence(Bite, 1.5)`: stun 45, damage 1.5 over the limit of 1: `Die`, then
    // `GameOver(null)`; the kill tag is the lizard's.
    let mut g = game(&HALL, (3, 1));
    let l = g.bitten(1.0);
    assert!(g.player_ref().is_dead());
    assert_eq!(g.player_ref().stun_ticks(), 45);
    assert_eq!(g.world.creature(g.player_id).kill_tag, Some(l));
    let over = g.take_game_over().expect("game over");
    assert_eq!(over.dependent_on_grasp, None);
    assert!(g.player_ref().danger_grasp().grasp.is_some());
}

#[test]
fn held_sixty_ticks_the_game_over_waits_on_the_grasp() {
    // `dangerGraspTime == 60`: `GameOver(dangerGrasp)`; tick 30 dropped the rock the
    // player held. The player is alive.
    let mut g = game(&HALL, (3, 1));
    let rock = g.armed(AbstractObjectType::Rock, (3, 2));
    let l = g.bitten(0.0);
    for t in 1..=60 {
        g.tick(Input::NONE, &mut ());
        assert!(g.world.creature(l).grasps[0].is_some(), "let go at {t}");
        if t == 29 {
            assert_eq!(g.player_grasps()[0], Some(Entity::Object(rock)));
        }
        if t == 30 {
            assert_eq!(g.player_grasps(), [None, None]);
        }
        if t < 60 {
            assert_eq!(g.take_game_over(), None, "at {t}");
        }
    }
    let serial = g.world.creature(l).grasps[0].expect("held").serial;
    let over = g.take_game_over().expect("game over at 60");
    assert_eq!(over.dependent_on_grasp, Some(serial));
    assert!(!g.player_ref().is_dead());
}

#[test]
fn a_rock_thrown_to_get_free_draws_once_and_hits_the_grabber() {
    // `ThrowToGetFree` with a rock: `value < 0.5` throws it as usual; otherwise it
    // is `Thrown` from the main chunk toward the grabber's nearest chunk (the head)
    // with that chunk as `meleeHitChunk`, `Slugcat_Throw_Rock`, and let go.
    let mut g = game(&HALL, (3, 1));
    let rock = g.armed(AbstractObjectType::Rock, (3, 2));
    let l = g.bitten(0.0);
    g.stream_until_next(|v| v >= 0.5);
    let mut expect = g.rng.clone();
    expect.value();
    g.throw_to_get_free(0, &mut ());
    let pos = g.world.chunk_pos(Entity::Object(rock), 0).expect("rock");
    let room = g.pos().room;
    let mut sounds = if room == g.camera_room {
        g.sounds.clone()
    } else {
        Sounds::default()
    };
    sounds.play_on_object(SoundId::SlugcatThrowRock, pos, false, 1.0, 1.0, &mut expect);
    assert_eq!(g.rng.state(), expect.state());
    assert_eq!(g.player_grasps(), [None, None]);
    let w = g.world.weapon_mut(rock).expect("rock");
    assert_eq!(w.mode, WeaponMode::Thrown);
    assert_eq!(w.throw_dir, (1, 0));
    assert_eq!(w.melee_hit_chunk, Some((l, 0)));
}

#[test]
fn a_throw_press_frees_the_player_when_the_rock_stuns_the_lizard() {
    // The press reaches `ThrowToGetFree` through the stun's neutral input; the rock
    // hits the head it was aimed at, the stunned lizard lets go (`LoseAllGrasps`), the
    // danger grasp is discontinued and forgotten the tick after.
    let mut g = game(&HALL, (3, 1));
    g.armed(AbstractObjectType::Rock, (3, 2));
    let l = g.bitten(0.0);
    g.tick(Input::NONE, &mut ());
    g.stream_until_next(|v| v >= 0.5);
    g.tick(THROW, &mut ());
    assert_eq!(g.player_grasps(), [None, None]);
    let mut freed = None;
    for t in 0..10 {
        g.tick(Input::NONE, &mut ());
        if g.world.creature(l).grasps[0].is_none() {
            freed = Some(t);
            break;
        }
    }
    assert!(freed.is_some(), "the lizard still holds the player");
    assert!(g.world.lizard_ref(l).stunned());
    assert!(g.player_grabbed_by().is_empty());
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().danger_grasp().grasp, None);
}

#[test]
fn a_spear_is_always_thrown_at_the_grabber_without_a_draw() {
    // A spear skips the half-chance; the Survivor's `ThrownSpear` draws nothing.
    let mut g = game(&HALL, (3, 1));
    let spear = g.armed(AbstractObjectType::Spear, (3, 2));
    let l = g.bitten(0.0);
    let pos = g.player_ref().head().pos;
    let mut expect = g.rng.clone();
    g.throw_to_get_free(0, &mut ());
    let room = g.pos().room;
    let mut sounds = if room == g.camera_room {
        g.sounds.clone()
    } else {
        Sounds::default()
    };
    sounds.play_on_object(
        SoundId::SlugcatThrowSpear,
        pos,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    assert_eq!(g.rng.state(), expect.state());
    let w = g.world.weapon_mut(spear).expect("spear");
    assert_eq!(w.mode, WeaponMode::Thrown);
    assert_eq!(w.melee_hit_chunk, Some((l, 0)));
}

#[test]
fn a_pickup_press_grabs_the_nearest_rock_into_the_last_free_hand() {
    // `DangerGraspPickup`: the last empty hand, `PickupCandidate(100)`, `SlugcatGrab`,
    // `PickedUp`'s sound.
    let mut g = game(&HALL, (3, 1));
    let rock = g
        .dev_spawn_object(AbstractObjectType::Rock, 0, (3, 1), false)
        .expect("rock");
    g.bitten(0.0);
    g.tick(PICKUP, &mut ());
    assert_eq!(g.player_grasps(), [None, Some(Entity::Object(rock))]);
}

#[test]
fn just_out_of_a_shortcut_the_bite_cannot_grab_the_player() {
    // `CanBeGrabbed`: `cantBeGrabbedCounter` refuses before any grasp is made.
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((5, 1), -1.0, 40);
    g.set_bite_damage_chance(l, 0.0);
    g.player_mut().set_cant_be_grabbed_after_spit_out();
    let hips = ChunkRef {
        owner: Entity::Creature(g.player_id),
        index: HIPS,
    };
    g.lizard_bite(l, Some(hips));
    assert!(g.world.creature(l).grasps[0].is_none());
    assert_eq!(g.player_ref().danger_grasp().grasp, None);
}

#[test]
fn the_jaws_hold_the_player_inside_the_lizards_container_until_it_lets_go() {
    // `Bite`: with graphics, `AddObjectToInternalContainer` of the bitten player's
    // graphics module; `Lizard.LoseAllGrasps` releases all internally contained sprites.
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((5, 1), -1.0, 40);
    g.set_bite_damage_chance(l, 0.0);
    g.world.lizard_at(l).graphics = Some(LizardGraphicsLink::default());
    g.player_graphics = true;
    let hips = ChunkRef {
        owner: Entity::Creature(g.player_id),
        index: HIPS,
    };
    g.lizard_bite(l, Some(hips));
    let link = g.world.lizard_ref(l).graphics.as_ref().expect("graphics");
    assert_eq!(link.internal_container, vec![Entity::Creature(g.player_id)]);
    g.lizard_lose_all_grasps(l);
    let link = g.world.lizard_ref(l).graphics.as_ref().expect("graphics");
    assert!(link.internal_container.is_empty());
}

#[test]
fn releasing_one_contained_object_skips_the_entry_after_it() {
    // `ReleaseSpecificInternallyContainedObjectSprites(obj)` removes by index inside a
    // rising loop, so of two adjacent entries for `obj` the second stays.
    let mut link = LizardGraphicsLink::default();
    let (a, b) = (Entity::Object(ObjectId(1)), Entity::Object(ObjectId(2)));
    for e in [a, a, b, a] {
        link.add_to_internal_container(e);
    }
    link.release_internally_contained(a);
    assert_eq!(link.internal_container, vec![a, b]);
}
