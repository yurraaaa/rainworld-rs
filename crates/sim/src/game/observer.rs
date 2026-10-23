//! `PlayerObserver`: the app's hooks into the game, called where the game calls its
//! graphics modules (the player's, the overseers', the flies' and the lizards').
//!
//! Mirrors: - (port-only: the app's graphics-module hooks)

use rw_assets::Room;
use rw_random::UnityRandom;

use super::{PlayerHands, PlayerSurroundings};
use crate::creature::fly::FlyView;
use crate::creature::lizard::{LizardGraphicsLink, LizardGraphicsTick, LizardView};
use crate::creature::overseer::hologram::OverseerHologram;
use crate::creature::overseer::{OverseerBody, OverseerView};
use crate::creature::{CreatureId, EntityId};
use crate::game::camera::CameraView;
use crate::item::Entity;
use crate::{MyceliaCx, OwnerKey, Player, Vec2, Water};

/// What the app attaches to the player (its graphics module), called where the game
/// calls it.
pub trait PlayerObserver {
    /// The player was constructed, before it is placed (`AddObject` builds the graphics
    /// module in a viewed room).
    fn created(&mut self, player: &Player, rng: &mut UnityRandom);
    /// Right after `Player.Update`: `PlayerGraphics.Update`.
    fn updated(
        &mut self,
        player: &Player,
        room: &Room,
        water: Option<&Water>,
        around: &PlayerSurroundings<'_>,
        rng: &mut UnityRandom,
    );
    /// The player left a shortcut: the inner `Player.SpitOutOfShortCut`'s graphics reset
    /// and `NewRoom`'s.
    fn spat_out(&mut self, player: &Player, rng: &mut UnityRandom);
    /// The outer `Player.SpitOutOfShortCut`'s reset, after the objects carried out with it.
    fn spat_out_finished(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    /// Whether the player has a graphics module.
    fn has_player_graphics(&self) -> bool {
        false
    }
    /// `PlayerGraphics.LookAtObject(target)`, or `LookAtNothing` for `None`.
    fn player_look_at(&mut self, _target: Option<Entity>) {}
    /// `PlayerGraphics.BiteFly(hand)` up to its drips: returns `head.pos`, where they
    /// start (`None` without a graphics module).
    fn player_bite_fly(&mut self, _hand: usize, _fly: CreatureId) -> Option<Vec2> {
        None
    }
    /// What the graphics' hands did in their update.
    fn player_hands(&self) -> Option<PlayerHands> {
        None
    }
    /// `PlayerGraphics.swallowing = 20` (`GrabUpdate` swallowed an object).
    fn player_swallowing(&mut self) {}
    /// `PlayerGraphics.ThrowObject(hand, obj)`: the hand follows the thrown weapon for
    /// five updates.
    fn player_throw_object(&mut self, _hand: usize, _obj: Entity) {}
    /// `(graphicsModule as PlayerGraphics).head.vel += dv`.
    fn nudge_player_head(&mut self, _dv: Vec2) {}
    /// A room script adds `dvx` to every tail segment's x velocity.
    fn nudge_tail_x(&mut self, _dvx: f32) {}
    /// A realized overseer is in the camera's room (`AddObject` or `NowViewed`): its
    /// `OverseerGraphics` is built unless it has one. Returns whether it built one.
    fn overseer_added(
        &mut self,
        _view: &OverseerView<'_>,
        _mx: &mut MyceliaCx<'_>,
        _rng: &mut UnityRandom,
    ) -> bool {
        false
    }
    /// `OverseerGraphics.Reset`.
    fn overseer_reset(
        &mut self,
        _view: &OverseerView<'_>,
        _mx: &mut MyceliaCx<'_>,
        _rng: &mut UnityRandom,
    ) {
    }
    /// Right after an overseer hologram's `Update` (the parts lerp their colours).
    fn hologram_updated(&mut self, _owner: EntityId, _hologram: &OverseerHologram) {}
    /// `OverseerGraphics.Emerge`.
    fn overseer_emerge(
        &mut self,
        _view: &OverseerView<'_>,
        _mx: &mut MyceliaCx<'_>,
        _rng: &mut UnityRandom,
    ) {
    }
    /// Right after `Overseer.Update`: the graphics module's update.
    fn overseer_updated(
        &mut self,
        _view: &OverseerView<'_>,
        _mx: &mut MyceliaCx<'_>,
        _rng: &mut UnityRandom,
    ) {
    }
    /// `Overseer.NewRoom`'s `UpdateCoralNeuronSystem`: the overseer now has the system of
    /// room `new` (`view.neuron_system` is the one it had); its graphics module, if any,
    /// moves its strands.
    fn overseer_new_room(
        &mut self,
        _view: &OverseerView<'_>,
        _mx: &mut MyceliaCx<'_>,
        _new: Option<crate::SystemId>,
    ) {
    }
    /// `OverseerGraphics.DrawPosOfSegment(0, 1)`, the eye, while the overseer has a
    /// graphics module.
    fn overseer_eye(&self, _id: EntityId, _body: &OverseerBody) -> Option<Vec2> {
        None
    }
    /// The overseer's graphics module is dropped (`NoLongerViewed`, or abstracted); the
    /// module's strand owner, if it had one.
    fn overseer_removed(&mut self, _id: EntityId) -> Option<OwnerKey> {
        None
    }
    /// A realized fly is in the camera's room (`AddObject` or `NowViewed`): its
    /// `FlyGraphics` is built unless it has one.
    fn fly_added(&mut self, _view: &FlyView<'_>, _rng: &mut UnityRandom) {}
    /// `FlyGraphics.Reset`.
    fn fly_reset(&mut self, _view: &FlyView<'_>) {}
    /// Right after `Fly.Update`: `FlyGraphics.Update`. `cameras` is the room's camera
    /// count (the culling test needs more than two).
    #[allow(clippy::too_many_arguments)]
    fn fly_updated(
        &mut self,
        _view: &FlyView<'_>,
        _room: &Room,
        _cameras: usize,
        _camera_room: &Room,
        _camera: &CameraView,
        _rng: &mut UnityRandom,
    ) {
    }
    /// `MoveToFront` of the fly's sprites (it joined a chain).
    fn fly_to_front(&mut self, _id: EntityId) {}
    /// The fly's graphics module is dropped.
    fn fly_removed(&mut self, _id: EntityId) {}
    /// Whether this observer builds lizard graphics modules (else a lizard never has
    /// one, as out of view).
    fn builds_lizard_graphics(&self) -> bool {
        false
    }
    /// `Lizard.InitiateGraphicsModule`: the `LizardGraphics` constructor unless it has
    /// one (under the lizard's seed, the stream restored), then its `Reset`; the module
    /// leaves its head and parts in `link`.
    fn lizard_added(
        &mut self,
        _view: &LizardView<'_>,
        _link: &mut LizardGraphicsLink,
        _rng: &mut UnityRandom,
    ) {
    }
    /// `LizardGraphics.Reset`.
    fn lizard_reset(
        &mut self,
        _view: &LizardView<'_>,
        _link: &mut LizardGraphicsLink,
        _rng: &mut UnityRandom,
    ) {
    }
    /// `LizardGraphics.Update`.
    fn lizard_updated(
        &mut self,
        _view: &LizardView<'_>,
        _link: &mut LizardGraphicsLink,
        _cx: &mut LizardGraphicsTick<'_>,
    ) {
    }
    /// `LizardGraphics.SuckedIntoShortCut(pos)`.
    fn lizard_sucked_into_shortcut(
        &mut self,
        _id: EntityId,
        _link: &mut LizardGraphicsLink,
        _pos: Vec2,
    ) {
    }
    /// The lizard's graphics module is dropped.
    fn lizard_removed(&mut self, _id: EntityId) {}
}

/// No graphics.
impl PlayerObserver for () {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(
        &mut self,
        _: &Player,
        _: &Room,
        _: Option<&Water>,
        _: &PlayerSurroundings<'_>,
        _: &mut UnityRandom,
    ) {
    }
    fn spat_out(&mut self, _: &Player, _: &mut UnityRandom) {}
}
