//! Headless screenshots of the realized bat flies: SU_A42 among its five flies (full
//! frames, a fly cropped and zoomed, consecutive motion frames) and a fly travelling a
//! pipe of SU_A38.

use std::path::Path;

use rw_sim::Vec2;

use super::fly_trace::FlyTrace;
use super::params::ScreenshotParams;

impl FlyTrace {
    /// Writes `<name>.png`: the whole camera frame, or a square of half-size `crop` around
    /// `centre`.
    fn shot(&mut self, dir: &Path, name: &str, crop: Option<(f32, Vec2)>) {
        let params = ScreenshotParams {
            crop_radius: crop.map(|(r, _)| r),
            crop_center: crop.map(|(_, c)| [c.x, c.y]),
            scale: crop.map(|_| 3.0),
            save_path: Some(
                dir.join(format!("{name}.png"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..ScreenshotParams::default()
        };
        self.session_mut().screenshot(&params).expect("screenshot");
    }

    /// The camera room's first fly whose graphics are drawn, if any.
    fn drawn_fly(&mut self) -> Option<Vec2> {
        let flies = self.fly_views();
        let game = self.session_mut().scene_mut()?.game_mut();
        flies
            .into_iter()
            .find(|(id, _)| game.fly_graphics(*id).is_some_and(|g| !g.culled))
            .map(|(_, pos)| pos)
    }

    /// The tube tile of a fly's vessel in the camera's room.
    fn vessel_in_view(&mut self) -> Option<(i32, i32)> {
        let sim = self.session_mut().scene_mut()?.game().sim();
        let room = sim.camera_room();
        sim.transport_vessels()
            .flatten()
            .find(|v| v.room == room)
            .map(|v| v.tiles().0)
    }
}

/// Screenshots written to `FLY_SHOTS=<dir>`.
#[test]
#[ignore = "writes screenshots; run with FLY_SHOTS=<dir>"]
fn fly_screenshots() {
    let Some(dir) = std::env::var_os("FLY_SHOTS") else {
        return;
    };
    let dir = Path::new(&dir);
    let Some(mut t) = FlyTrace::entered("SU_A41", 200, "SU_A42") else {
        return;
    };
    t.shot(dir, "a42_full_arrival", None);
    let mut motion = 0;
    for k in 0..600 {
        t.wander_tick(k);
        if k % 150 == 75 {
            t.shot(dir, &format!("a42_full_{k}"), None);
        }
        if motion < 8
            && k >= 100
            && let Some(pos) = t.drawn_fly()
        {
            t.shot(dir, &format!("a42_fly_motion_{motion}"), Some((60.0, pos)));
            motion += 1;
        }
    }
    assert_eq!(motion, 8, "a fly drawn in SU_A42");
    let Some(mut t) = FlyTrace::loaded("SU_A38") else {
        return;
    };
    let mut last = None;
    let mut in_pipe = 0;
    for _ in 0..600 {
        t.wait(1);
        let Some(tile) = t.vessel_in_view() else {
            continue;
        };
        if in_pipe < 4 && last != Some(tile) {
            let centre = Vec2::new(tile.0 as f32 * 20.0 + 10.0, tile.1 as f32 * 20.0 + 10.0);
            t.shot(
                dir,
                &format!("a38_fly_in_pipe_{in_pipe}"),
                Some((80.0, centre)),
            );
            if in_pipe == 0 {
                t.shot(dir, "a38_full_fly_in_pipe", None);
            }
            in_pipe += 1;
        }
        last = Some(tile);
    }
    assert_eq!(in_pipe, 4, "a fly through a pipe of SU_A38");
}
