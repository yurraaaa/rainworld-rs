//! Body chunks, physical objects, terrain collision and tile lookups.
//!
//! Mirrors: - (port-only: module index)

pub(crate) mod chunk;
pub(crate) mod object;
pub(crate) mod room_ext;
pub(crate) mod shared_physics;

#[cfg(test)]
mod tests;
