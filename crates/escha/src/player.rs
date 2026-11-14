//! The player on a field, as Escha & Logy DX moves her (the user brain, `0x140234910` on):
//! her speed and gait, the jump and its voice, the landing sounds, the field music. Walking
//! against the field's walls is the engine's (`barrel_engine::walk`).

use barrel_engine::rng::Rng;

/// The event whose having been seen makes the player run faster (`EVENT_XG05_110`).
pub const FASTER_RUN_EVENT: usize = 734;

/// How fast the player runs on a field, units a second: 500, or 625 once `FASTER_RUN_EVENT`
/// has been seen (the user brain's speed, `0x140234910`).
pub fn player_run_speed(faster: bool) -> f32 {
    if faster { 625.0 } else { 500.0 }
}

/// How much faster the player goes with the run toggle on.
pub const RUN_TOGGLE: f32 = 1.5;

/// How far the stick has to lean before the player moves at all.
pub const STICK_DEAD_ZONE: f32 = 0.2;

/// The player's move over `dt` seconds for a stick leaning `stick` (x, z; each -1 to 1): none
/// inside the dead zone, else along the stick at the run speed times how far it leans (at most
/// 1), half as fast again with the run toggle on (`run_on`: pad button 6, "Run On" / "Run
/// Off", off at the start), as the user brain does (`0x140234950`).
pub fn player_move(stick: [f32; 2], dt: f32, faster: bool, run_on: bool) -> [f32; 2] {
    let tilt = (stick[0] * stick[0] + stick[1] * stick[1]).sqrt();
    if tilt < STICK_DEAD_ZONE {
        return [0.0, 0.0];
    }
    let toggle = if run_on { RUN_TOGGLE } else { 1.0 };
    let scale = player_run_speed(faster) * dt * tilt.min(1.0) / tilt * toggle;
    [stick[0] * scale, stick[1] * scale]
}

/// The player's field motion: `MOTION_F_WAIT` (31), `MOTION_WALK` (32) or `MOTION_RUN` (33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Wait,
    Walk,
    Run,
    /// In a jump (`MOTION_JUMP`, 49): not a pick by speed, so the next pick always changes.
    Jump,
}

/// The motion for the player moving at `speed` (units a second, as she moved this frame) and
/// the rate it plays at, as the user brain picks them (`0x140235410`): running from 0.42 of the
/// run speed, walking below it, waiting when still; played at the speed times 0.00225, never
/// slower than 1. (It blends into a new run or walk over 0.15 seconds.)
pub fn player_gait(speed: f32, faster: bool) -> (Gait, f32) {
    let gait = if speed >= player_run_speed(faster) * 0.42 {
        Gait::Run
    } else if speed > 0.0 {
        Gait::Walk
    } else {
        Gait::Wait
    };
    (gait, (speed * 0.01 * 0.225).max(1.0))
}

/// The player's jump (pad button 1 in the user brain, `0x140234cbf`): `MOTION_JUMP` (49) at 1.5,
/// sound 0x23, and sometimes a voice. The chance of a voice starts at 5 (`0x14023429f`); each jump
/// rolls it (`0x1404ca5f0`): a hit plays a voice clip from 11315 to 11317 for Escha (hero 0) or
/// 11318 to 11320 for Logy (`0x1404ca660`, both ends included; `0x140160960`) and sets the chance
/// back to 5, a miss raises it by 10.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpVoice {
    pub chance: i32,
}

pub const JUMP_MOTION: u32 = 49;
pub const JUMP_SOUND: u32 = 0x23;

/// The landing sounds by ground kind (the map record's fifth word, `Area::landing`): a table
/// the field builds at its +0x2f68 (`0x14006dd5e`), ten 16-byte entries whose first word is
/// the bank effect, played at half volume as a jump lands (`0x140234e85`). The open fields
/// are kind 2 (17), the town's streets and ruins 7 (23), rooms 9 (25).
pub const LANDING_SOUNDS: [u32; 10] = [15, 16, 17, 18, 19, 20, 21, 23, 24, 25];
pub const LANDING_VOLUME: f32 = 0.5;

/// The music a map plays (`0x14028d0e0`, kept at the field's +0xb68): the map record's track
/// (`Area::music`), except that the Atelier, the two offices and the hallway (maps 0 to 2 and
/// 11) play the lead's Atelier theme, 40 (`fm_atelier_boy01`) for Logy and 41
/// (`fm_atelier_girl01_a`) for Escha, and track 45 (`fm_heigen01`, the Withering Plains)
/// turns to 39 (`fm_haikyo01`) once flag 303 (read by `0x14035f270`) is set.
pub fn field_music(map: usize, track: i32, logy_leads: bool, flag_303: bool) -> i32 {
    match (map, track) {
        (0..=2 | 11, _) => if logy_leads { 40 } else { 41 },
        (_, 45) if flag_303 => 39,
        _ => track,
    }
}

/// A ground kind's landing sound.
pub fn landing_sound(kind: i32) -> Option<u32> {
    LANDING_SOUNDS.get(usize::try_from(kind).ok()?).copied()
}

impl Default for JumpVoice {
    fn default() -> Self {
        JumpVoice { chance: 5 }
    }
}

impl JumpVoice {
    /// One jump's voice, if any, for `hero` (0 Escha, else Logy).
    pub fn jump(&mut self, hero: u32, rng: &mut Rng) -> Option<u32> {
        if rng.chance(self.chance) {
            let top = if hero == 0 { 0x2c35 } else { 0x2c38 };
            let clip = rng.range(top - 2, top) as u32;
            self.chance = 5;
            Some(clip)
        } else {
            self.chance += 10;
            None
        }
    }
}

/// A character's gravity, metres a second squared (its +0x164, set to 0x416b5c0f by
/// `0x14026b9c9`).
pub const GRAVITY: f32 = 14.71;
/// The jump's launch speed upward, metres a second (`0x140234f80`: 5, or 25 when the character's
/// +0x12d is set), given as it leaves the ground (the jump's second phase).
pub const JUMP_SPEED: f32 = 5.0;
pub const JUMP_SPEED_HIGH: f32 = 25.0;

/// How far above where it left the ground a character launched at `speed` (metres a second) is
/// after `t` seconds, in field units (`0x14026bc80`: each frame it moves by v dt - a dt^2 / 2,
/// the height times 100, and its speed drops by a dt). Below 0 it has come down.
pub fn jump_height(speed: f32, t: f32) -> f32 {
    100.0 * (speed * t - GRAVITY * t * t / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atelier_theme_by_lead() {
        assert_eq!(field_music(0, 40, true, false), 40);
        assert_eq!(field_music(0, 40, false, false), 41);
        assert_eq!(field_music(11, 40, false, false), 41);
        assert_eq!(field_music(3, 29, false, false), 29);
        assert_eq!(field_music(40, 45, false, true), 39);
        assert_eq!(field_music(40, 45, false, false), 45);
    }

    #[test]
    fn landing_sounds_by_ground() {
        assert_eq!(landing_sound(2), Some(17));
        assert_eq!(landing_sound(9), Some(25));
        assert_eq!(landing_sound(10), None);
        assert_eq!(landing_sound(-1), None);
    }

    #[test]
    fn the_stick_leans_the_speed() {
        assert_eq!(player_move([0.1, 0.1], 1.0, false, false), [0.0, 0.0], "inside the dead zone");
        let half = player_move([0.0, 0.5], 1.0, false, false);
        assert!((half[1] - 250.0).abs() < 1e-3 && half[0] == 0.0, "{half:?}");
        assert!((player_move([0.0, 1.0], 1.0, false, true)[1] - 750.0).abs() < 1e-3, "run toggle on");
        let full = player_move([3.0, 4.0], 0.5, true, false);
        assert!((full[0] - 187.5).abs() < 1e-3 && (full[1] - 250.0).abs() < 1e-3, "past full lean: {full:?}");
    }

    #[test]
    fn the_speed_picks_the_gait() {
        assert_eq!(player_gait(0.0, false), (Gait::Wait, 1.0));
        assert_eq!(player_gait(200.0, false), (Gait::Walk, 1.0), "under 0.42 of 500");
        assert_eq!(player_gait(500.0, false).0, Gait::Run);
        assert!((player_gait(500.0, false).1 - 1.125).abs() < 1e-6);
        assert_eq!(player_gait(250.0, true).0, Gait::Walk, "under 0.42 of 625");
    }

    #[test]
    fn jump_voices_grow_likelier_until_one_plays() {
        let mut v = JumpVoice::default();
        let mut rng = Rng::from_state([1, 2, 3, 4]);
        let mut misses = 0;
        let clip = loop {
            match v.jump(0, &mut rng) {
                Some(c) => break c,
                None => misses += 1,
            }
            assert_eq!(v.chance, 5 + 10 * misses);
        };
        assert!((11315..=11317).contains(&clip) && v.chance == 5);
        // By 100 the voice is certain.
        let mut v = JumpVoice { chance: 100 };
        assert!(v.jump(1, &mut rng).is_some_and(|c| (11318..=11320).contains(&c)));
    }

    #[test]
    fn a_jump_rises_85_units_and_lands_in_0_68_s() {
        let peak = JUMP_SPEED / GRAVITY;
        assert!((jump_height(JUMP_SPEED, peak) - 85.0).abs() < 0.1);
        assert!(jump_height(JUMP_SPEED, 2.0 * peak).abs() < 1e-3 && (2.0 * peak - 0.68).abs() < 0.01);
    }
}
