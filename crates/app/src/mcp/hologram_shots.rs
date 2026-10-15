//! Headless screenshots of the guide's holograms on the tutorial route (new Survivor
//! game, seed 1): the box, bat, pole and food pictures with no food, and the shelter
//! pointers and super-jump lesson fed by the pit.

use std::path::{Path, PathBuf};

use rw_assets::PngImage;
use rw_graphics::{Color, OverseerGraphics};
use rw_render::FrameImage;
use rw_sim::Vec2;
use rw_sim::creature::overseer::OverseerColor;
use rw_sim::creature::overseer::hologram::{
    HoloImage, HologramKind, HologramPart, ImageHologram, ImageId, OverseerHologram, PartKind,
};

use super::Session;
use super::params::{NewGameParams, ScreenshotParams, SetPlayerParams, StepParams};
use super::tutorial_route::{RouteView, TutorialRoute};

/// Ticks after a hologram first shows when it is photographed.
const SHOT_AGES: [u64; 7] = [3, 8, 14, 22, 40, 80, 160];

/// Half the side of the crop around a hologram, in screen pixels, and its zoom.
const CROP: i32 = 110;
const ZOOM: usize = 3;

const MAX_TICKS: u64 = 6000;

/// One hologram in the camera's room.
struct Shown {
    id: u64,
    label: String,
    pos: [f32; 2],
    fade: f32,
}

/// The photographed run.
struct Shots {
    dir: PathBuf,
    name: &'static str,
    session: Session,
    /// Each hologram's id and the tick it was first seen.
    seen: Vec<(u64, u64)>,
}

impl Shots {
    fn run(dir: &Path, name: &'static str, food: i32, pit_first: bool) {
        let Some(mut session) = Session::installed() else {
            return;
        };
        session
            .new_game(&NewGameParams {
                slugcat: "White".to_owned(),
                seed: Some(1),
            })
            .expect("new game");
        session
            .set_player(&SetPlayerParams {
                food: Some(food),
                ..SetPlayerParams::default()
            })
            .expect("food");
        let mut shots = Self {
            dir: dir.to_owned(),
            name,
            session,
            seen: Vec::new(),
        };
        let mut route = if pit_first {
            TutorialRoute::pit_first()
        } else {
            TutorialRoute::new()
        };
        let mut state = shots.session.state().expect("state");
        while !route.done() && state.tick < MAX_TICKS {
            let input = route.input(&RouteView::of(&state));
            state = shots
                .session
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(input),
                })
                .expect("steps");
            shots.photograph(state.tick);
        }
    }

    /// The holograms in the camera's room.
    fn shown(&mut self) -> (Vec<Shown>, [f32; 2]) {
        let scene = self.session.scene_mut().expect("scene");
        let origin = scene.camera().view_origin(1.0);
        let sim = scene.game().sim();
        let room = sim.camera_room();
        let shown = sim
            .world()
            .creatures()
            .filter_map(|(_, c)| c.overseer_body())
            .flat_map(|b| b.holograms.iter())
            .filter(|h| h.room == room)
            .map(|h| Shown {
                id: h.id,
                label: format!("{:?}", h.message),
                pos: h.pos.to_array(),
                fade: h.fade,
            })
            .collect();
        (shown, origin)
    }

    fn photograph(&mut self, tick: u64) {
        let (shown, origin) = self.shown();
        for h in shown {
            let first = match self.seen.iter().find(|(id, _)| *id == h.id) {
                Some(&(_, t)) => t,
                None => {
                    self.seen.push((h.id, tick));
                    tick
                }
            };
            if !SHOT_AGES.contains(&(tick - first)) {
                continue;
            }
            let stem = format!("{}_{tick}_{}_h{}", self.name, h.label, h.id);
            let full = self.dir.join(format!("{stem}.png"));
            let shot = self
                .session
                .screenshot(&ScreenshotParams {
                    save_path: Some(full.to_string_lossy().into_owned()),
                    ..ScreenshotParams::default()
                })
                .expect("screenshot");
            let image = PngImage::decode(&shot.png).expect("decodes");
            let centre = [
                (h.pos[0] - origin[0]) as i32,
                image.height() as i32 - (h.pos[1] - origin[1]) as i32,
            ];
            let crop = Self::crop(&image, centre);
            let path = self.dir.join(format!("{stem}_crop.png"));
            std::fs::write(&path, crop.encode_png().expect("encodes")).expect("writes");
            eprintln!(
                "{stem}: fade {:.3}, at ({:.0}, {:.0})",
                h.fade, h.pos[0], h.pos[1]
            );
        }
    }

    /// The square of side `2 * CROP` around `centre` (pixels, y down), zoomed.
    fn crop(image: &PngImage, centre: [i32; 2]) -> FrameImage {
        let side = (2 * CROP) as usize;
        let mut rgba = Vec::with_capacity(side * side * ZOOM * ZOOM * 4);
        for y in 0..side * ZOOM {
            for x in 0..side * ZOOM {
                let sx = centre[0] - CROP + (x / ZOOM) as i32;
                let sy = centre[1] - CROP + (y / ZOOM) as i32;
                let inside = sx >= 0
                    && sy >= 0
                    && (sx as usize) < image.width()
                    && (sy as usize) < image.height();
                let px = if inside {
                    image.pixel(sx as usize, sy as usize)
                } else {
                    [0, 0, 0, 255]
                };
                rgba.extend_from_slice(&px);
            }
        }
        let n = (side * ZOOM) as u32;
        FrameImage::new(n, n, rgba)
    }
}

/// Screenshots of every hologram as it fades in and stays, written to
/// `HOLOGRAM_SHOTS=<dir>` (`<run>_<tick>_<message>_h<id>.png` and `_crop` zoomed on it).
#[test]
#[ignore = "writes screenshots; run with HOLOGRAM_SHOTS=<dir>"]
fn hologram_screenshots() {
    let Some(dir) = std::env::var_os("HOLOGRAM_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    Shots::run(&dir, "route", 0, false);
    Shots::run(&dir, "fed", 4, true);
}

/// A hand-made image hologram.
struct ImageVariant {
    name: &'static str,
    image: ImageId,
    random: bool,
    alpha: f32,
    fade: f32,
    angry: bool,
}

impl ImageVariant {
    const ALL: [Self; 5] = [
        Self::new("swarmers", ImageId::Swarmers, false, 1.0, 1.0, false),
        Self::new("moon", ImageId::MoonPortrait, false, 1.0, 1.0, false),
        Self::new("random", ImageId::Swarmers, true, 1.0, 1.0, false),
        Self::new("fading", ImageId::ScavOutpost, false, 0.6, 0.85, false),
        Self::new("angry", ImageId::DeadSlugcatA, false, 1.0, 1.0, true),
    ];

    const fn new(
        name: &'static str,
        image: ImageId,
        random: bool,
        alpha: f32,
        fade: f32,
        angry: bool,
    ) -> Self {
        Self {
            name,
            image,
            random,
            alpha,
            fade,
            angry,
        }
    }

    /// `template` made into this picture at `pos`.
    fn hologram(&self, template: &OverseerHologram, id: u64, pos: Vec2) -> OverseerHologram {
        let mut h = template.clone();
        h.id = id;
        h.pos = pos;
        h.last_pos = pos;
        h.fade = self.fade;
        h.last_fade = self.fade;
        h.parts = Vec::new();
        if self.angry {
            h.parts
                .push(HologramPart::new(PartKind::RedCross { small: false }));
        }
        h.parts.push(HologramPart::new(PartKind::HoloImage(Box::new(
            self.holo_image(),
        ))));
        h.parts.push(HologramPart::new(PartKind::Frame));
        h.kind = if self.angry {
            HologramKind::Angry {
                counter: 0,
                show_time: 0,
            }
        } else {
            HologramKind::Image(ImageHologram {
                images: vec![self.image],
                show_pos: None,
                counter: 0,
                curr_image: 0,
                time_on_each_image: 0,
                show_time: 0,
                logged_as_seen: false,
            })
        };
        h
    }

    fn holo_image(&self) -> HoloImage {
        let pan = Vec2::new(0.1, -0.05);
        HoloImage {
            last_img: 0,
            last_show_img: (0, 0),
            pan_pos: pan,
            last_pan_pos: pan,
            pan_vel: Vec2::ZERO,
            pan_center: Vec2::ZERO,
            my_alpha: self.alpha,
            last_my_alpha: self.alpha,
            random_image: 7,
            random_image_time: 0.0,
            random_flicker: 0.0,
            show_random_flicker_image: self.random,
            sound: false,
        }
    }
}

/// The image holograms the tutorial never shows (gate scenes, anger), drawn by hand
/// beside the box lesson in SU_C04 (`image_all.png` and `_crop`, in
/// `HOLOGRAM_SHOTS=<dir>`).
#[test]
#[ignore = "writes screenshots; run with HOLOGRAM_SHOTS=<dir>"]
fn image_hologram_screenshots() {
    let Some(dir) = std::env::var_os("HOLOGRAM_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let Some(mut session) = Session::installed() else {
        return;
    };
    session
        .new_game(&NewGameParams {
            slugcat: "White".to_owned(),
            seed: Some(1),
        })
        .expect("new game");
    let mut route = TutorialRoute::new();
    let mut state = session.state().expect("state");
    while state.tick < 1580 {
        let input = route.input(&RouteView::of(&state));
        state = session
            .step(&StepParams {
                ticks: Some(1),
                input: Some(input),
            })
            .expect("steps");
    }
    let template = session
        .scene_mut()
        .expect("scene")
        .game()
        .sim()
        .world()
        .creatures()
        .filter_map(|(_, c)| c.overseer_body())
        .flat_map(|b| b.holograms.iter())
        .next()
        .cloned()
        .expect("the box lesson's hologram");
    let base = OverseerGraphics::hologram_color_of(OverseerColor::Yellow, 1.0, Color::BLACK);
    for (k, v) in ImageVariant::ALL.iter().enumerate() {
        let pos = Vec2::new(1500.0 + 140.0 * k as f32, 520.0);
        eprintln!("image {} at {pos:?}", v.name);
        let h = v.hologram(&template, 1000 + k as u64, pos);
        session
            .scene_mut()
            .expect("scene")
            .game_mut()
            .show_extra_hologram(h, base, 10);
    }
    let full = dir.join("image_all.png");
    let shot = session
        .screenshot(&ScreenshotParams {
            save_path: Some(full.to_string_lossy().into_owned()),
            ..ScreenshotParams::default()
        })
        .expect("screenshot");
    let origin = session
        .scene_mut()
        .expect("scene")
        .camera()
        .view_origin(1.0);
    let png = PngImage::decode(&shot.png).expect("decodes");
    let centre = [
        (1640.0 - origin[0]) as i32,
        png.height() as i32 - (520.0 - origin[1]) as i32,
    ];
    let crop = Shots::crop(&png, centre);
    std::fs::write(
        dir.join("image_all_crop.png"),
        crop.encode_png().expect("encodes"),
    )
    .expect("writes");
}
