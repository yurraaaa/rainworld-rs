//! Block-compressed texture decoding: DXT1 (BC1) and DXT5 (BC3). Texels of a 4x4
//! block come out in data order (row 0 first, which is the lower row in Unity).
//!
//! Interpolated colours and alphas are truncated, not rounded: that reproduces
//! AssetRipper's decode of every texture in `resources.assets` exactly. GPUs may round
//! differently (by 1/255).
//!
//! Mirrors: -

/// One 8-byte DXT1 (BC1) block: two RGB565 endpoints and 2-bit indices.
pub(crate) struct Bc1Block(pub [u8; 8]);

/// One 16-byte DXT5 (BC3) block: an alpha block then a BC1 colour block.
pub(crate) struct Bc3Block(pub [u8; 16]);

impl Bc1Block {
    /// RGB565 to 8 bits per channel, replicating the high bits.
    fn expand(c: u16) -> [u32; 3] {
        let (r, g, b) = (
            u32::from(c >> 11),
            u32::from((c >> 5) & 63),
            u32::from(c & 31),
        );
        [
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        ]
    }

    /// The four palette colours; with `three_colour` (BC1 only) colour 0 <= colour 1
    /// selects the half mix and transparent black for indices 2 and 3.
    fn palette(&self, three_colour: bool) -> [[u8; 4]; 4] {
        let c0 = u16::from_le_bytes([self.0[0], self.0[1]]);
        let c1 = u16::from_le_bytes([self.0[2], self.0[3]]);
        let (a, b) = (Self::expand(c0), Self::expand(c1));
        if c0 > c1 || !three_colour {
            [
                Self::mix(a, b, 1, 0),
                Self::mix(a, b, 0, 1),
                Self::mix(a, b, 2, 1),
                Self::mix(a, b, 1, 2),
            ]
        } else {
            [
                Self::mix(a, b, 1, 0),
                Self::mix(a, b, 0, 1),
                Self::mix(a, b, 1, 1),
                [0, 0, 0, 0],
            ]
        }
    }

    /// `(wa * a + wb * b) / (wa + wb)`, truncated; opaque.
    fn mix(a: [u32; 3], b: [u32; 3], wa: u32, wb: u32) -> [u8; 4] {
        let total = wa + wb;
        let [r, g, b] = std::array::from_fn(|k| ((wa * a[k] + wb * b[k]) / total) as u8);
        [r, g, b, 255]
    }

    fn texels(&self, three_colour: bool) -> [[u8; 4]; 16] {
        let palette = self.palette(three_colour);
        let bits = u32::from_le_bytes([self.0[4], self.0[5], self.0[6], self.0[7]]);
        std::array::from_fn(|i| palette[(bits >> (2 * i)) as usize & 3])
    }

    pub fn decode(&self) -> [[u8; 4]; 16] {
        self.texels(true)
    }
}

impl Bc3Block {
    /// The eight alpha values: 6 interpolated when a0 > a1, else 4 plus 0 and 255.
    fn alphas(a0: u8, a1: u8) -> [u8; 8] {
        let (a, b) = (u32::from(a0), u32::from(a1));
        if a0 > a1 {
            std::array::from_fn(|i| match i {
                0 => a0,
                1 => a1,
                _ => {
                    let i = i as u32 - 1;
                    (((7 - i) * a + i * b) / 7) as u8
                }
            })
        } else {
            std::array::from_fn(|i| match i {
                0 => a0,
                1 => a1,
                6 => 0,
                7 => 255,
                _ => {
                    let i = i as u32 - 1;
                    (((5 - i) * a + i * b) / 5) as u8
                }
            })
        }
    }

    pub fn decode(&self) -> [[u8; 4]; 16] {
        let alphas = Self::alphas(self.0[0], self.0[1]);
        let mut bits = [0; 8];
        bits[..6].copy_from_slice(&self.0[2..8]);
        let bits = u64::from_le_bytes(bits);
        let mut colour = [0; 8];
        colour.copy_from_slice(&self.0[8..]);
        let mut texels = Bc1Block(colour).texels(false);
        for (i, t) in texels.iter_mut().enumerate() {
            t[3] = alphas[(bits >> (3 * i)) as usize & 7];
        }
        texels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc1_four_colour_block() {
        // c0 = pure red (0xF800), c1 = black; row 0 indices 0, 1, 2, 3.
        let t = Bc1Block([0x00, 0xF8, 0x00, 0x00, 0b1110_0100, 0, 0, 0]).decode();
        assert_eq!(
            &t[..4],
            [
                [255, 0, 0, 255],
                [0, 0, 0, 255],
                [170, 0, 0, 255],
                [85, 0, 0, 255]
            ]
        );
        assert_eq!(t[4], [255, 0, 0, 255]);
    }

    #[test]
    fn bc1_three_colour_block() {
        // c0 = black <= c1 = red: index 2 is the half mix, 3 transparent black.
        let t = Bc1Block([0x00, 0x00, 0x00, 0xF8, 0b1110_0100, 0, 0, 0]).decode();
        assert_eq!(t[2], [127, 0, 0, 255]);
        assert_eq!(t[3], [0, 0, 0, 0]);
    }

    #[test]
    fn bc1_expands_565_by_bit_replication() {
        // c0 = (1, 1, 1) in 565, c1 = 0: 8 = 0b00001000 -> r = 8, g = 4, b = 8.
        let t = Bc1Block([0x21, 0x08, 0, 0, 0, 0, 0, 0]).decode();
        assert_eq!(t[0], [8, 4, 8, 255]);
    }

    #[test]
    fn bc3_eight_alpha_block() {
        // a0 = 255 > a1 = 0; texels 0..8 use indices 0..7; colour block white.
        let mut block = [0u8; 16];
        block[0] = 255;
        // indices 0,1,2,3,4,5,6,7 packed in 3 bits each.
        let bits: u64 = (0..8).map(|i: u64| i << (3 * i)).sum();
        block[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
        block[8..12].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        let t = Bc3Block(block).decode();
        let alphas: Vec<u8> = t[..8].iter().map(|p| p[3]).collect();
        assert_eq!(alphas, [255, 0, 218, 182, 145, 109, 72, 36]);
        assert_eq!(t[0], [255, 255, 255, 255]);
        assert_eq!(t[8][3], 255);
    }

    #[test]
    fn bc3_six_alpha_block_and_four_colour_mode() {
        // a0 = 0 <= a1 = 200: indices 2..5 interpolate, 6 = 0, 7 = 255.
        let mut block = [0u8; 16];
        block[1] = 200;
        let bits: u64 = (0..8).map(|i: u64| i << (3 * i)).sum();
        block[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
        // Colour: c0 = black <= c1 = red; BC3 still uses the 2:1 mix for index 2.
        block[8..16].copy_from_slice(&[0x00, 0x00, 0x00, 0xF8, 0b1110_0100, 0, 0, 0]);
        let t = Bc3Block(block).decode();
        let alphas: Vec<u8> = t[..8].iter().map(|p| p[3]).collect();
        assert_eq!(alphas, [0, 200, 40, 80, 120, 160, 0, 255]);
        assert_eq!(&t[2][..3], [85, 0, 0]);
        assert_eq!(&t[3][..3], [170, 0, 0]);
    }
}
