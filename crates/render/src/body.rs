//! Render interpolation for simulation types (extension traits on `rw_sim` types).
//!
//! Mirrors: -

use rw_sim::BodyChunk;

/// Position of a chunk between simulation ticks.
pub trait ChunkInterpolation {
    /// Position `alpha` of the way from the previous tick (`last_pos`) to the latest
    /// (`pos`), as room-pixel `[x, y]`. `alpha` is clamped to `0..=1`, so a frame never
    /// extrapolates past the simulated state.
    #[must_use]
    fn interpolated_pos(&self, alpha: f32) -> [f32; 2];
}

impl ChunkInterpolation for BodyChunk {
    fn interpolated_pos(&self, alpha: f32) -> [f32; 2] {
        self.last_pos
            .lerp(self.pos, alpha.clamp(0.0, 1.0))
            .to_array()
    }
}

#[cfg(test)]
mod tests {
    use rw_sim::Vec2;

    use super::*;

    fn moving_chunk() -> BodyChunk {
        let mut chunk = BodyChunk::new(Vec2::new(0.0, 0.0), 9.0, 0.35);
        chunk.pos = Vec2::new(8.0, -4.0);
        chunk
    }

    #[test]
    fn chunk_interpolates_from_last_pos_to_pos() {
        let chunk = moving_chunk();
        assert_eq!(chunk.interpolated_pos(0.0), [0.0, 0.0]);
        assert_eq!(chunk.interpolated_pos(0.25), [2.0, -1.0]);
        assert_eq!(chunk.interpolated_pos(0.5), [4.0, -2.0]);
        assert_eq!(chunk.interpolated_pos(1.0), [8.0, -4.0]);
    }

    #[test]
    fn alpha_is_clamped() {
        let chunk = moving_chunk();
        assert_eq!(chunk.interpolated_pos(-1.0), [0.0, 0.0]);
        assert_eq!(chunk.interpolated_pos(3.0), [8.0, -4.0]);
    }
}
