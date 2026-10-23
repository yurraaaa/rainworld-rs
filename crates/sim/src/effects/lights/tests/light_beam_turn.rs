//! Placed light beams and spot lights in one test room (`Room.Loaded`, `LightBeam.Update`,
//! `ApplyPalette`).

use crate::Input;
use crate::Vec2;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::effects::lights::turn::tests::{WithGraphics, camera_image, game};
use crate::game::RoomObject;

/// A beam coloured by the environment (`colorB` 0.5) at (100, 60), a spot light at
/// (200, 80), a beam with no environment colour at (300, 60) and a beam whose data the
/// game cannot parse (too few fields: it throws, the port skips it).
const PLACED: &str = "LightBeam><100><60><0~40~40~40~40~0~0~0~0.4~0~0.5~1, \
     SpotLight><200><80><0~300~300~300~300~0, \
     LightBeam><300><60><0~40~40~40~40~0~0~0~0.5~0.25~0~1, \
     LightBeam><400><60><0~40~40~40~40~0~0~0>";

fn story() -> (&'static SlugcatCampaign, StoryStart) {
    (
        &SlugcatCampaign::SURVIVOR,
        StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
    )
}

/// Each placed beam and spot light takes its place in the room's update list in settings
/// order (the spot light's slot sits between the beams'); a beam's quad is its position
/// plus the three handles; data the game would throw on is skipped.
#[test]
fn beams_and_spots_join_the_update_list_in_settings_order() {
    let (campaign, start) = story();
    let g = game(PLACED, campaign, start, 3);
    let r = g.world.realized(0).expect("room");
    assert_eq!((r.light_beams.len(), r.spot_lights.len()), (2, 1));
    let (a, b, spot) = (&r.light_beams[0], &r.light_beams[1], &r.spot_lights[0]);
    assert!(a.update_order < spot.update_order && spot.update_order < b.update_order);
    assert_eq!(
        a.quad,
        [
            Vec2::new(100.0, 60.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(140.0, 100.0),
            Vec2::new(140.0, 60.0)
        ]
    );
    assert_eq!(spot.quad[3], Vec2::new(500.0, 80.0));
    assert_eq!(
        g.room_light_beam_objects(0),
        [
            (a.update_order, RoomObject::LightBeam(a.update_order)),
            (b.update_order, RoomObject::LightBeam(b.update_order))
        ]
    );
}

/// With camera 0's picture: every beam takes palette texel (8, 5), here (64, 150, 0) /
/// 255 (`ApplyPalette`); on its next update only the beam with `colorB` above 0 reads
/// the picture's colour at its second corner (`PixelColorAtCoordinate(quad[1])`).
#[test]
fn camera_picture_colours_beams() {
    let (campaign, start) = story();
    let mut g = game(PLACED, campaign, start, 3);
    g.set_camera_image(Some(camera_image(5)));
    let r = g.world.realized(0).expect("room");
    for b in &r.light_beams {
        assert_eq!(b.palette_lit_color, [64.0 / 255.0, 150.0 / 255.0, 0.0, 1.0]);
        assert_eq!(b.environment_color, [0.0; 4]);
    }
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let want = camera_image(5).pixel_color_at_coordinate(Vec2::new(100.0, 100.0));
    assert_eq!(r.light_beams[0].environment_color, want);
    assert_ne!(want, [0.0; 4]);
    assert_eq!(r.light_beams[1].environment_color, [0.0; 4]);
}

/// Neither `LightBeam` nor `SpotLight` draws from `UnityEngine.Random` (making or
/// updating): the shared stream is where it is without them.
#[test]
fn beams_and_spots_make_no_random_draws() {
    let (campaign, start) = story();
    let with = game(PLACED, campaign, start, 5);
    let (campaign, start) = story();
    let without = game("", campaign, start, 5);
    assert_eq!(with.rng.state(), without.rng.state());
}
