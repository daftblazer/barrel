//! Barrel's engine: what every game's play shares (events and their stage, cameras, the
//! Squirrel VM, walking, the clock, wind), each game's rules sitting on top in its own crate.

pub mod camera;
pub mod clock;
pub mod curve;
pub mod event;
pub mod field;
pub mod field_camera;
pub mod play_camera;
pub mod rng;
pub mod skin;
pub mod squirrel;
pub mod stage;
pub mod walk;
pub mod wind;
