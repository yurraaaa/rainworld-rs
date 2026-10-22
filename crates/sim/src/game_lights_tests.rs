//! Placed lights, the glowing player's light and the room's light queries in one test
//! room (`Room.Loaded`, `LightSource.Update`, `PlayerGraphics.Update`'s light,
//! `LightSourceExposure`, `LightSourceColor`, `DarknessOfPoint`).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{
    ActiveMods, PaletteImage, PaletteTexture, PlacedObject, PngImage, Room, RoomSettings,
};
use rw_random::UnityRandom;

use super::super::RoomObject;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room;
use crate::world::World;
use crate::{
    CameraImage, Input, LightTie, Player, PlayerGlow, PlayerObserver, PlayerSurroundings,
    RainWorldGame, Sounds, Start, Vec2, Water,
};

/// `su_a34`'s first light (environment colour, fading with the sun) at (100, 60), a
/// `SuperJumpInstruction` and a flat effect-colour light at (300, 60) that does not
/// fade.
const PLACED: &str = "LightSource><100><60><0.75~Environment~-40~-30~65~37.5~1~0, \
     SuperJumpInstruction><200><100><60~0~-40~-50~40~-30, \
     LightSource><300><60><1~EffectColor1~0~100~0~0~0~1";

/// A player observer with a graphics module and nothing else.
struct WithGraphics;

impl PlayerObserver for WithGraphics {
    fn created(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    fn updated(
        &mut self,
        _player: &Player,
        _room: &Room,
        _water: Option<&Water>,
        _around: &PlayerSurroundings<'_>,
        _rng: &mut UnityRandom,
    ) {
    }
    fn spat_out(&mut self, _player: &Player, _rng: &mut UnityRandom) {}
    fn has_player_graphics(&self) -> bool {
        true
    }
}

/// A story of `campaign` in a 20 x 8 room with `placed` objects, the player at (2, 1),
/// after `ticks` ticks.
fn game(
    placed: &str,
    campaign: &'static SlugcatCampaign,
    story: StoryStart,
    ticks: usize,
) -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        story,
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut WithGraphics,
    );
    for _ in 0..ticks {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    g
}

/// A 1400 x 800 level image of red `level_red` everywhere and a palette whose runtime
/// texel (x, y) is (8x, 30y, 0) over 255.
fn camera_image(level_red: u8) -> CameraImage {
    let mut pal = Vec::with_capacity(32 * 16);
    for png_row in 0..16u8 {
        let y = (15 - png_row).saturating_sub(8);
        pal.extend((0..32u8).map(|x| [x * 8, y * 30, 0, 255]));
    }
    let pal = PaletteImage::from_image(&PngImage::from_pixels(32, 16, pal)).expect("palette");
    CameraImage {
        room: 0,
        cam_pos: Vec2::ZERO,
        image: Arc::new(PngImage::from_pixels(
            1400,
            800,
            vec![[level_red, 0, 0, 255]; 1400 * 800],
        )),
        palette: PaletteTexture::blend(&pal, None, 0.0),
    }
}

/// `Room.Loaded` makes each placed light where the settings list it, between the
/// objects around it in the update list; after its first update it has the data's
/// rad (the handle's length) and strength, and `lastAlpha` is its `Alpha`.
#[test]
fn placed_lights_join_the_update_list_in_settings_order() {
    let g = game(
        PLACED,
        &SlugcatCampaign::SURVIVOR,
        StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
        3,
    );
    let r = g.world.realized(0).expect("room");
    let lights = &r.light_sources;
    assert_eq!(lights.len(), 2);
    let trigger = g.room_trigger_objects(0)[0].0;
    assert!(lights[0].id < trigger && trigger < lights[1].id);
    let (a, b) = (&lights[0], &lights[1]);
    assert_eq!(
        (a.pos, a.rad, a.alpha),
        (Vec2::new(100.0, 60.0), 50.0, 0.75)
    );
    assert!(a.environmental_light && a.fade_with_sun && a.color_from_environment && !a.flat);
    assert_eq!(
        (a.effect_color, a.affected_by_palette_darkness, a.tied_to),
        (-1, 0.0, None)
    );
    assert_eq!((b.rad, b.alpha, b.effect_color), (100.0, 1.0, 0));
    assert!(b.flat && !b.fade_with_sun && !b.color_from_environment);
    let shader_light = g.world.rain_cycle.shader_light();
    assert_eq!(a.last_alpha, a.alpha_now(shader_light));
    assert_eq!(b.last_alpha, 1.0);
    // White until the camera's picture colours them.
    assert_eq!((a.color, a.color_alpha), ([1.0; 3], 1.0));
    let in_list = g.room_light_source_objects(0);
    assert_eq!(
        in_list,
        [
            (a.id, RoomObject::LightSource(a.id)),
            (b.id, RoomObject::LightSource(b.id))
        ]
    );
}

/// With camera 0's picture: the effect-colour light takes palette texel (30, 5),
/// here (240, 150, 0) / 255 (`ApplyPalette`), the environment light the level pixel
/// under it on its next update.
#[test]
fn camera_picture_colours_placed_lights() {
    let mut g = game(
        PLACED,
        &SlugcatCampaign::SURVIVOR,
        StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
        3,
    );
    g.set_camera_image(Some(camera_image(5)));
    let r = g.world.realized(0).expect("room");
    let b = &r.light_sources[1];
    assert_eq!(b.color_alpha, 240.0 / 255.0);
    assert_eq!((b.color[0], b.color[2]), (1.0, 0.0));
    assert!((b.color[1] - 0.625).abs() < 1e-6);
    assert_eq!(r.light_sources[0].color, [1.0; 3]);
    g.tick(Input::NONE, &mut WithGraphics);
    let a = &g.world.realized(0).expect("room").light_sources[0];
    let [r, gr, bl, _] = camera_image(5).pixel_color_at_coordinate(a.pos);
    let want = crate::LightSource::new(0, 0, Vec2::ZERO, true, [r, gr, bl], None);
    assert_eq!((a.color, a.color_alpha), (want.color, want.color_alpha));
    assert!(a.color_alpha > 0.0 && a.color[0] == 1.0);
}

/// A glowing player in a dark room gets its graphics' light (`Lerp(white, slugcat
/// colour, 0.5)`, rad 300, alpha 1, kept alive by upkeep) at its main chunk; when the
/// room is no longer dark the graphics let go and the light dies two updates later.
/// Without the glow, none.
#[test]
fn the_glowing_player_carries_a_light() {
    let story = StoryStart {
        the_glow: true,
        ..StoryStart::new_game(&SlugcatCampaign::MONK)
    };
    let mut g = game("", &SlugcatCampaign::MONK, story, 3);
    assert!(g.world.realized(0).expect("room").light_sources.is_empty());
    g.set_camera_darkness(0.5);
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    let light = &r.light_sources[0];
    assert_eq!(light.tied_to, Some(LightTie::Player));
    assert!(light.require_up_keep);
    assert_eq!(
        (light.color[0], light.color[1], light.color_alpha),
        (1.0, 1.0, 1.0)
    );
    assert!((light.color[2] - (0.5 + 0.5 * 115.0 / 255.0)).abs() < 1e-6);
    assert_eq!((light.rad, light.alpha), (0.0, 0.0));
    for _ in 0..3 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let main = g.player.as_ref().expect("player").body.chunks[0].pos;
    let light = &g.world.realized(0).expect("room").light_sources[0];
    assert_eq!((light.rad, light.alpha), (300.0, 1.0));
    assert!(light.pos.distance(main) < 20.0);
    g.set_camera_darkness(0.0);
    g.tick(Input::NONE, &mut WithGraphics);
    assert_eq!(g.world.realized(0).expect("room").light_sources.len(), 1);
    g.tick(Input::NONE, &mut WithGraphics);
    g.tick(Input::NONE, &mut WithGraphics);
    assert!(g.world.realized(0).expect("room").light_sources.is_empty());

    let mut g = game(
        "",
        &SlugcatCampaign::MONK,
        StoryStart::new_game(&SlugcatCampaign::MONK),
        3,
    );
    g.set_camera_darkness(0.5);
    g.tick(Input::NONE, &mut WithGraphics);
    assert!(g.world.realized(0).expect("room").light_sources.is_empty());
}

/// `DefaultSlugcatColor`: Survivor white, Monk (1, 1, 115/255), Hunter (1, 115/255,
/// 115/255); the light is halfway to white.
#[test]
fn slugcat_light_colours() {
    use rw_assets::SlugcatName;
    assert_eq!(PlayerGlow::light_color(SlugcatName::White), [1.0; 3]);
    let hunter = PlayerGlow::light_color(SlugcatName::Red);
    let half = 0.5 + 0.5 * 115.0 / 255.0;
    assert_eq!(hunter[0], 1.0);
    assert!((hunter[1] - half).abs() < 1e-6 && (hunter[2] - half).abs() < 1e-6);
}

/// `LightSourceExposure`, `LightSourceColor` and `DarknessOfPoint` at the light, half
/// its rad away and outside it; `DarknessOfPoint` is 0 below darkness 0.2 and outside
/// the camera's room.
#[test]
fn room_light_queries() {
    let mut g = game(
        PLACED,
        &SlugcatCampaign::SURVIVOR,
        StoryStart::new_game(&SlugcatCampaign::SURVIVOR),
        3,
    );
    g.set_camera_image(Some(camera_image(5)));
    let r = g.world.realized(0).expect("room");
    let b = &r.light_sources[1];
    let lightness = (1.0 + 0.625 + 0.0) / 3.0 * (240.0 / 255.0);
    assert!((b.lightness() - lightness).abs() < 1e-6);
    let at = Vec2::new(300.0, 60.0);
    assert!((r.light_source_exposure(at) - lightness).abs() < 1e-6);
    let c = r.light_source_color(at);
    assert!((c[0] - lightness).abs() < 1e-6 && c[2] == 0.0);
    assert!((c[1] - 0.625 * lightness).abs() < 1e-6);
    assert!((r.light_source_exposure(Vec2::new(350.0, 60.0)) - 0.5 * lightness).abs() < 1e-6);
    assert_eq!(r.light_source_exposure(Vec2::new(300.0, 161.0)), 0.0);
    g.set_camera_darkness(0.6);
    let want = 0.5 * (1.0 - lightness);
    assert!((g.darkness_of_point(0, at) - want).abs() < 1e-6);
    g.set_camera_darkness(0.2);
    assert_eq!(g.darkness_of_point(0, at), 0.0);
    g.set_camera_darkness(0.6);
    assert_eq!(g.darkness_of_point(1, at), 0.0);
}

/// No `UnityEngine.Random` call in `LightSource` or `Room.Loaded`'s light branch: the
/// shared stream is where it is without the lights, also with a glowing player's.
#[test]
fn lights_make_no_draws() {
    let jump_only = "SuperJumpInstruction><200><100><60~0~-40~-50~40~-30";
    let story = StoryStart {
        the_glow: true,
        ..StoryStart::new_game(&SlugcatCampaign::SURVIVOR)
    };
    let mut with = game(PLACED, &SlugcatCampaign::SURVIVOR, story.clone(), 3);
    let mut without = game(jump_only, &SlugcatCampaign::SURVIVOR, story, 3);
    with.set_camera_image(Some(camera_image(5)));
    for g in [&mut with, &mut without] {
        g.set_camera_darkness(0.5);
        for _ in 0..20 {
            g.tick(Input::NONE, &mut WithGraphics);
        }
    }
    assert_eq!(with.world.realized(0).expect("room").light_sources.len(), 3);
    assert_eq!(
        without.world.realized(0).expect("room").light_sources.len(),
        1
    );
    assert_eq!(with.rng.state(), without.rng.state());
}
