//! The title going out on New Game, as the game's opening-key run has it (ryza-recomp
//! `scenarios/opening-key`, A on New Game at capture frame 1535): the logo, the cursor, the
//! items and the version string fade out together over 15 frames (the copyright line stays),
//! a black fill rises over the whole screen from the frame after the button for a second (the
//! command `u49 1 2 100` its flow runs on the frame after that), and the title is torn down
//! when the opening event begins, 65 frames after the button (1600).

/// Frames the title's layer takes to fade out after the button: 1545 is at a third of its
/// opacity, 1555 gone.
pub const LEAVE: i32 = 15;

/// Frames the black fill takes to rise: 1555 is at 19/60, 1595 at 59/60.
pub const COVER_RISE: i32 = 60;

/// The frame, counted from the button, the opening event begins on (capture frame 1600): its
/// first command runs that frame, the title is gone and the scene shows black.
pub const OPENING_AFTER: i32 = 65;

/// How much of the title's layer is left `frame` frames into the title, the button on `choose`:
/// 1 up to the button, then a line down to 0 over [`LEAVE`] frames.
pub fn layer_left(frame: i32, choose: i32) -> f32 {
    (1.0 - (frame - choose).max(0) as f32 / LEAVE as f32).clamp(0.0, 1.0)
}

/// The black fill's opacity on `frame`: nothing up to the button, then a step a frame (the
/// first on the frame after the button) to full after [`COVER_RISE`] frames.
pub fn cover(frame: i32, choose: i32) -> f32 {
    ((frame - choose - 1).max(0) as f32 / COVER_RISE as f32).min(1.0)
}

/// Whether the opening event has begun on `frame`.
pub fn opening_begun(frame: i32, choose: i32) -> bool {
    frame >= choose + OPENING_AFTER
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture's frames (button on 1535): the layer's opacity and the fill's.
    #[test]
    fn the_games_frames() {
        assert_eq!(layer_left(1535, 1535), 1.0);
        assert!((layer_left(1545, 1535) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(layer_left(1555, 1535), 0.0);
        assert_eq!(cover(1535, 1535), 0.0);
        assert!((cover(1545, 1535) - 0.15).abs() < 1e-6);
        assert!((cover(1555, 1535) - 0.316_666_7).abs() < 1e-6);
        assert!((cover(1595, 1535) - 0.983_333_3).abs() < 1e-6);
        assert_eq!(cover(1605, 1535), 1.0);
        assert!(!opening_begun(1599, 1535) && opening_begun(1600, 1535));
    }
}
