//! Which camera image of a room is shown and where the game's window sits on it
//! (`RoomCamera.GetCameraBestIndex`, `Update` and `DrawUpdate`). GPU-free.
//!
//! Each camera position is the room point of its image's bottom-left corner. The game
//! shows a [`RoomCamera::SCREEN_SIZE`] window resting at `cam + (17, 18)`; it leans
//! up to 8 px toward a camera the creature is heading for and switches with a hard cut
//! once the creature leaves the current camera's zone for another's.

use rw_sim::Vec2;

/// What the camera follows: the creature's first body chunk, or its on-screen position
/// while it travels through a shortcut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// The player's current input direction (`input[0]`), zero for other creatures.
    pub input: [i8; 2],
    pub in_shortcut_at: Option<[f32; 2]>,
}

/// One room's camera state, advanced once per 40 Hz tick.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomCamera {
    positions: Vec<Vec2>,
    screen: Vec2,
    current: usize,
    pos: Vec2,
    last_pos: Vec2,
    lean: Vec2,
    input_forward: Vec2,
}

impl RoomCamera {
    /// Default `ScreenSize` option (1366x768).
    pub const SCREEN_SIZE: [f32; 2] = [1366.0, 768.0];

    /// Size of a camera image in room pixels.
    pub const IMAGE_SIZE: [f32; 2] = [1400.0, 800.0];

    /// Starts at camera 0, as the game does when a room is entered without a camera.
    #[must_use]
    pub fn new(positions: &[[f32; 2]], screen: [f32; 2]) -> Self {
        let mut camera = Self {
            positions: positions.iter().map(|&p| Vec2::from(p)).collect(),
            screen: Vec2::from(screen),
            current: 0,
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            lean: Vec2::ZERO,
            input_forward: Vec2::ZERO,
        };
        camera.cut_to(0);
        camera
    }

    /// Index of the shown camera.
    #[must_use]
    pub fn current(&self) -> usize {
        self.current
    }

    /// Room position of camera `i` (image bottom-left); the origin for a room without
    /// cameras.
    #[must_use]
    pub fn position(&self, i: usize) -> [f32; 2] {
        self.positions
            .get(i)
            .copied()
            .unwrap_or(Vec2::ZERO)
            .to_array()
    }

    /// `hDisplace`: centres the window horizontally in the 1400 px image, minus 8.
    fn h_displace(&self) -> f32 {
        (Self::IMAGE_SIZE[0] - self.screen.x) / 2.0 - 8.0
    }

    fn cam(&self, i: usize) -> Vec2 {
        Vec2::from(self.position(i))
    }

    /// Where the window rests on camera `i`: `cam + (hDisplace + 8, 18)`.
    #[must_use]
    pub fn rest_pos(&self, i: usize) -> [f32; 2] {
        (self.cam(i) + Vec2::new(self.h_displace() + 8.0, 18.0)).to_array()
    }

    /// `CameraPositionDist` with `onlyInScreen`: distance from `p` to the centre of
    /// camera `i` (`cam + (700, 402)`) when `p` lies inside its zone, a 1024x768 box at
    /// `cam + (188, 18)` (190 px wider on each side for `widescreen`), else `None`.
    #[must_use]
    pub fn zone_distance(&self, i: usize, p: [f32; 2], widescreen: bool) -> Option<f32> {
        let (cam, p) = (self.cam(i), Vec2::from(p));
        let wide = if widescreen { 190.0 } else { 0.0 };
        let inside = p.x > cam.x + 188.0 - wide
            && p.x < cam.x + 188.0 + 1024.0 + wide
            && p.y > cam.y + 18.0
            && p.y < cam.y + 18.0 + 768.0;
        inside.then(|| p.distance(cam + Vec2::new(700.0, 402.0)))
    }

    /// The camera whose zone holds `p` with the nearest centre (the game also requires
    /// a distance above 0), trying the widescreen zones when no plain one does.
    fn best_camera(&self, p: [f32; 2]) -> Option<usize> {
        [false, true].into_iter().find_map(|wide| {
            (0..self.positions.len())
                .filter_map(|i| Some((i, self.zone_distance(i, p, wide)?)))
                .filter(|&(_, d)| d > 0.0 && d < 10_000.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        })
    }

    /// One tick of `RoomCamera.Update`: pick the camera for `follow`, then move the
    /// window toward its rest position plus lean. Returns `true` when the camera was
    /// switched (a hard cut: load that camera's image and palette fade).
    pub fn update(&mut self, follow: Option<&Follow>) -> bool {
        let switched = follow.is_some_and(|f| self.choose_camera(f));
        self.last_pos = self.pos;
        let seek = Vec2::from(self.rest_pos(self.current)) + self.lean * 8.0;
        self.pos = self.clamp(self.pos.lerp(seek, 0.1));
        switched
    }

    /// `GetCameraBestIndex`; cuts to a new camera only when the creature's chunk has
    /// left the current camera's zone, else leans toward the candidate.
    fn choose_camera(&mut self, follow: &Follow) -> bool {
        if self.positions.len() <= 1 {
            return false;
        }
        self.lean *= 0.9;
        self.input_forward += Vec2::new(f32::from(follow.input[0]), f32::from(follow.input[1]));
        self.input_forward = Vec2::new(
            self.input_forward.x.clamp(-20.0, 20.0),
            self.input_forward.y.clamp(-20.0, 20.0),
        );
        self.input_forward *= 0.995;
        let (test, chunk) = match follow.in_shortcut_at {
            Some(p) => (Vec2::from(p), Vec2::from(p)),
            None => {
                let pos = Vec2::from(follow.pos);
                (pos + Vec2::from(follow.vel) + self.input_forward * 2.0, pos)
            }
        };
        let Some(best) = self.best_camera(test.to_array()) else {
            return false;
        };
        if best == self.current {
            return false;
        }
        if self
            .zone_distance(self.current, chunk.to_array(), false)
            .is_none()
        {
            self.cut_to(best);
            return true;
        }
        let (from, to) = (self.cam(self.current), self.cam(best));
        if (from.x - to.x).abs() > (from.y - to.y).abs() {
            self.lean.x = if from.x > to.x { -1.0 } else { 1.0 };
        } else {
            self.lean.y = if from.y > to.y { -1.0 } else { 1.0 };
        }
        false
    }

    /// `ApplyPositionChange`: show camera `i` with the window at rest and no lean.
    pub fn cut_to(&mut self, i: usize) {
        self.current = i;
        self.pos = Vec2::from(self.rest_pos(i));
        self.last_pos = self.pos;
        self.lean = Vec2::ZERO;
    }

    /// Keeps the window within 20 px of rest horizontally and between `cam.y + 1` and
    /// `cam.y + 33` vertically.
    fn clamp(&self, p: Vec2) -> Vec2 {
        let (cam, rest_x) = (self.cam(self.current), self.rest_pos(self.current)[0]);
        Vec2::new(
            p.x.clamp(rest_x - 20.0, rest_x + 20.0),
            p.y.clamp(cam.y + 1.0, cam.y + 33.0),
        )
    }

    /// Bottom-left of the visible window in room pixels at `alpha` between the last two
    /// ticks: clamped, floored to whole pixels, minus 0.02 (`DrawUpdate`'s `targetPoint`).
    #[must_use]
    pub fn view_origin(&self, alpha: f32) -> [f32; 2] {
        let p = self.clamp(self.last_pos.lerp(self.pos, alpha));
        [p.x.floor() - 0.02, p.y.floor() - 0.02]
    }

    /// Size of the visible window in room pixels.
    #[must_use]
    pub fn screen_size(&self) -> [f32; 2] {
        self.screen.to_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// su_b04's cameras: file `-160,-30|864,-40` in a 35-tile-high room.
    fn su_b04() -> RoomCamera {
        RoomCamera::new(&[[-160.0, -70.0], [864.0, -60.0]], RoomCamera::SCREEN_SIZE)
    }

    fn at(pos: [f32; 2]) -> Follow {
        Follow {
            pos,
            vel: [0.0, 0.0],
            input: [0, 0],
            in_shortcut_at: None,
        }
    }

    #[test]
    fn window_rests_at_cam_plus_17_18() {
        let cam = su_b04();
        assert_eq!(cam.current(), 0);
        assert_eq!(cam.rest_pos(0), [-143.0, -52.0]);
        assert_eq!(cam.view_origin(0.5), [-143.02, -52.02]);
        // A 1024-wide screen option rests 171 px in.
        let narrow = RoomCamera::new(&[[0.0, 0.0]], [1024.0, 768.0]);
        assert_eq!(narrow.rest_pos(0), [188.0, 18.0]);
    }

    #[test]
    fn zones() {
        let cam = su_b04();
        // Camera 0 zone: x in (28, 1052), y in (-52, 716).
        assert!(cam.zone_distance(0, [29.0, 0.0], false).is_some());
        assert!(cam.zone_distance(0, [28.0, 0.0], false).is_none());
        assert!(cam.zone_distance(0, [1051.0, 715.0], false).is_some());
        assert!(cam.zone_distance(0, [1052.0, 0.0], false).is_none());
        assert!(cam.zone_distance(0, [1100.0, 0.0], true).is_some());
        // Distance to the camera centre cam + (700, 402) = (540, 332).
        let d = cam.zone_distance(0, [640.0, 332.0], false).expect("inside");
        assert!((d - 100.0).abs() < 1e-3, "{d}");
    }

    #[test]
    fn switches_only_after_leaving_the_zone() {
        let mut cam = su_b04();
        // In both zones (camera 1's zone starts at x = 1052): stays, leans right.
        assert!(!cam.update(Some(&at([1040.0, 300.0]))));
        assert_eq!(cam.current(), 0);
        // Nearer camera 1's centre but still inside camera 0's zone: lean, no cut.
        let mut c2 = su_b04();
        let f = Follow {
            vel: [30.0, 0.0],
            ..at([1045.0, 300.0])
        };
        assert!(!c2.update(Some(&f)));
        assert_eq!(c2.current(), 0);
        assert_eq!(c2.lean, Vec2::new(1.0, 0.0));
        // Past camera 0's zone: hard cut to camera 1, window at its rest position.
        assert!(c2.update(Some(&at([1060.0, 300.0]))));
        assert_eq!(c2.current(), 1);
        assert_eq!(c2.view_origin(0.0), [881.0 - 0.02, -42.0 - 0.02]);
        // Back left beyond camera 1's zone (x <= 1052): cut back.
        assert!(c2.update(Some(&at([1000.0, 300.0]))));
        assert_eq!(c2.current(), 0);
    }

    #[test]
    fn lean_slides_the_window_at_most_8_px() {
        let mut cam = su_b04();
        let f = Follow {
            vel: [30.0, 0.0],
            ..at([1045.0, 300.0])
        };
        for _ in 0..200 {
            cam.update(Some(&f));
        }
        let x = cam.view_origin(1.0)[0];
        // Lean is set back to 1 every tick: seek = rest + 8, approached from below.
        assert!(
            (-136.02..=-135.02).contains(&x),
            "slid right by 7-8 px: {x}"
        );
        assert!(!cam.update(None));
    }

    #[test]
    fn single_camera_never_moves() {
        let mut cam = RoomCamera::new(&[[-220.0, -70.0]], RoomCamera::SCREEN_SIZE);
        for _ in 0..10 {
            assert!(!cam.update(Some(&at([5000.0, 5000.0]))));
        }
        assert_eq!(cam.view_origin(0.3), [-203.02, -52.02]);
    }
}
