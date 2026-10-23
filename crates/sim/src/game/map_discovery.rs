//! Map discovery: the progression's per-region discovery textures
//! (`PlayerProgression.mapDiscoveryTextures`, `mapLastUpdatedTime`), what `HUD.Map` and
//! the player's exit discovery write into them, and their save form: a base64 PNG in a
//! `MAP<progDivB>REGION<progDivB>...` progression line (`rainworld-re/notes/hud.md`).
//!
//! Mirrors: PlayerProgression.cs (mapDiscoveryTextures), HUD/Map.cs

use std::collections::BTreeMap;

use base64::Engine;
use rw_assets::PngImage;
use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_assets::world::AbstractRoomData;
use rw_random::UnityRandom;

use crate::Vec2;

/// `Custom.diagonals`.
pub const DIAGONALS: [(i32, i32); 4] = [(-1, -1), (-1, 1), (1, 1), (1, -1)];
/// `Map.DiscoverResolution`: map texture pixels per discovery pixel.
pub const DISCOVER_RESOLUTION: f32 = 7.0;

/// An 8-bit RGBA texture addressed like a Unity `Texture2D` (`y` = 0 is the bottom
/// row), wrapping out-of-range coordinates as `TextureWrapMode.Repeat`, the default of
/// `new Texture2D` and `LoadImage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityTexture {
    width: i32,
    height: i32,
    /// Bottom row first.
    pixels: Vec<[u8; 4]>,
}

impl UnityTexture {
    /// A texture filled with opaque black.
    #[must_use]
    pub fn black(width: i32, height: i32) -> Self {
        let n = (width.max(1) * height.max(1)) as usize;
        Self {
            width: width.max(1),
            height: height.max(1),
            pixels: vec![[0, 0, 0, 255]; n],
        }
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y.rem_euclid(self.height) * self.width + x.rem_euclid(self.width)) as usize
    }

    /// `GetPixel(x, y).r`.
    #[must_use]
    pub fn r(&self, x: i32, y: i32) -> f32 {
        f32::from(self.pixels[self.index(x, y)][0]) / 255.0
    }

    /// `SetPixel(x, y, new Color(r, g, b))`: alpha 1, channels stored as bytes
    /// (rounded; inferred).
    pub fn set(&mut self, x: i32, y: i32, rgb: [f32; 3]) {
        let i = self.index(x, y);
        let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        self.pixels[i] = [r, g, b, 255];
    }

    /// `SetPixel` of a grey level.
    pub fn set_grey(&mut self, x: i32, y: i32, v: f32) {
        self.set(x, y, [v, v, v]);
    }

    /// The red channel of every pixel, bottom row first.
    #[must_use]
    pub fn reds(&self) -> Vec<u8> {
        self.pixels.iter().map(|p| p[0]).collect()
    }

    /// The texture as an image in PNG row order (top row first).
    #[must_use]
    pub fn to_image(&self) -> PngImage {
        let rows: Vec<[u8; 4]> = self
            .pixels
            .chunks_exact(self.width as usize)
            .rev()
            .flatten()
            .copied()
            .collect();
        PngImage::from_pixels(self.width as usize, self.height as usize, rows)
    }

    /// `LoadImage` of a decoded PNG.
    #[must_use]
    pub fn from_image(image: &PngImage) -> Self {
        let (w, h) = (image.width(), image.height());
        let pixels = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| image.unity_pixel(x, y))
            .collect();
        Self {
            width: w as i32,
            height: h as i32,
            pixels,
        }
    }
}

/// A region's discovery texture: red > 0 where the map may be revealed (white where
/// walked, 0.5 and 0.25 grey at the fringes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryTexture(pub UnityTexture);

impl DiscoveryTexture {
    /// `Map.OnePixelDiscoverMap`.
    pub fn one_pixel_discover(&mut self, p: (i32, i32)) {
        self.0.set_grey(p.0, p.1, 1.0);
    }

    /// `Map.SmallDiscoverMap`: the pixel white, each of the four neighbours darker than
    /// 0.25 set to 0.25 on a coin flip. Always four draws.
    pub fn small_discover(&mut self, p: (i32, i32), rng: &mut UnityRandom) {
        self.0.set_grey(p.0, p.1, 1.0);
        for (dx, dy) in FOUR_DIRECTIONS {
            let (x, y) = (p.0 + dx, p.1 + dy);
            if rng.value() < 0.5 && self.0.r(x, y) < 0.25 {
                self.0.set_grey(x, y, 0.25);
            }
        }
    }

    /// `Map.DiscoverMap`: [`Self::small_discover`] in complete darkness, else the pixel
    /// and its four neighbours white and the diagonals at least 0.5.
    pub fn discover(&mut self, p: (i32, i32), complete_darkness: bool, rng: &mut UnityRandom) {
        if complete_darkness {
            self.small_discover(p, rng);
            return;
        }
        for (dx, dy) in [(0, 0)].into_iter().chain(FOUR_DIRECTIONS) {
            self.0.set_grey(p.0 + dx, p.1 + dy, 1.0);
        }
        for (dx, dy) in DIAGONALS {
            let (x, y) = (p.0 + dx, p.1 + dy);
            if self.0.r(x, y) < 0.5 {
                self.0.set_grey(x, y, 0.5);
            }
        }
    }

    /// `LoadByteStringIntoMapTexture`: base64 PNG bytes; `None` when they do not decode
    /// or the image is narrower than 10 (the game's corruption check).
    #[must_use]
    pub fn from_base64_png(text: &str) -> Option<Self> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .ok()?;
        let image = PngImage::decode(&bytes).ok()?;
        (image.width() >= 10).then(|| Self(UnityTexture::from_image(&image)))
    }

    /// `Convert.ToBase64String(texture.EncodeToPNG())`.
    #[must_use]
    pub fn to_base64_png(&self) -> String {
        let png = self.0.to_image().encode().unwrap_or_default();
        base64::engine::general_purpose::STANDARD.encode(png)
    }
}

/// `PlayerProgression.mapDiscoveryTextures` and `mapLastUpdatedTime`, by region name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapDiscovery {
    pub textures: BTreeMap<String, DiscoveryTexture>,
    /// `DateTime.Now.Ticks` when the map last created or loaded the texture.
    pub last_updated: BTreeMap<String, i64>,
}

impl MapDiscovery {
    #[must_use]
    pub fn texture(&self, region: &str) -> Option<&DiscoveryTexture> {
        self.textures.get(region)
    }

    pub fn texture_mut(&mut self, region: &str) -> Option<&mut DiscoveryTexture> {
        self.textures.get_mut(region)
    }

    /// Loads a `MAP` line's texture string for `region`; false (and nothing stored) when
    /// it is corrupt.
    pub fn load_base64(&mut self, region: &str, text: &str) -> bool {
        match DiscoveryTexture::from_base64_png(text) {
            Some(t) => {
                self.textures.insert(region.to_owned(), t);
                true
            }
            None => {
                self.textures.remove(region);
                false
            }
        }
    }

    /// The texture string a `MAP` line stores for `region`.
    #[must_use]
    pub fn store_base64(&self, region: &str) -> Option<String> {
        self.textures
            .get(region)
            .map(DiscoveryTexture::to_base64_png)
    }
}

/// What the map reads of a room (`MapData`'s position, size and layer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapRoom {
    /// Global room index.
    pub index: i32,
    /// `mapPos`.
    pub pos: Vec2,
    /// Size in tiles.
    pub size: (i32, i32),
    pub layer: i32,
}

impl MapRoom {
    #[must_use]
    pub fn of(room: &AbstractRoomData) -> Self {
        Self {
            index: room.index,
            pos: Vec2::new(room.map_pos[0], room.map_pos[1]),
            size: room.size,
            layer: room.layer,
        }
    }

    /// `Map.OnTexturePos`: room position `pos` in map texture pixels; with
    /// `account_for_layer` the room's layer picks its third of the stacked texture.
    #[must_use]
    pub fn on_texture_pos(&self, pos: Vec2, map_size: Vec2, account_for_layer: bool) -> Vec2 {
        let mut v = self.pos / 3.0 + Vec2::new(10.0, 10.0);
        let half = Vec2::new(self.size.0 as f32 * 20.0, self.size.1 as f32 * 20.0) / 2.0;
        if account_for_layer {
            v.y += self.layer as f32 * map_size.y;
        }
        v + (pos - half) / 20.0
    }

    /// `IntVector2.FromVector2(OnTexturePos(pos, room, true) / DiscoverResolution)`.
    #[must_use]
    pub fn discovery_pixel(&self, pos: Vec2, map_size: Vec2) -> (i32, i32) {
        Self::pixel_of(self.on_texture_pos(pos, map_size, true) / DISCOVER_RESOLUTION)
    }

    /// `IntVector2.FromVector2`: truncation toward zero.
    #[must_use]
    pub fn pixel_of(v: Vec2) -> (i32, i32) {
        (v.x as i32, v.y as i32)
    }
}

/// Camera 0's HUD map as the player's exit discovery sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HudMapLink {
    /// `hud.map != null`.
    pub present: bool,
    /// `discLoaded`: the map made or loaded its discovery texture.
    pub loaded: bool,
    /// `mapData.regionName`.
    pub region: String,
    /// `mapSize`: the map texture's width and a third of its height.
    pub map_size: Vec2,
}

#[cfg(test)]
#[path = "tests/map_discovery.rs"]
mod tests;
