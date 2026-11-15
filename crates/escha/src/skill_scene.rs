//! A battle skill event (`Event/skill`, `docs/events.md`, "Which skill event an action plays")
//! played between a battle's fighters: the actor is character 10000 and its target 10001, as
//! the events name them; the others they use (10004 and up: stand-ins the events place
//! themselves, for the camera to aim at) come from the event.
//!
//! The result is the scene sampled each frame: where the actor and the target stand and face,
//! and the camera, for the battle playback to show in place of its own camera and placement.

use barrel_engine::stage::{Measures, Playback, Snapshot, Stage, FRAME};
use barrel_formats::ebd::{Ebd, OperandTable};
use std::collections::BTreeMap;

/// The events' number for the acting fighter.
pub const ACTOR: u32 = 10000;
/// The events' number for the fighter acted on.
pub const TARGET: u32 = 10001;

/// A fighter as the scene starts: where it stands, its facing (radians about the vertical axis,
/// 0 towards +z) and its character's measures (`Chara.xml`; the points the events aim at).
///
/// The facing is the one the events' offsets turn by. The game takes it from the object a
/// battle character hangs from (`0x1402f0fa0`: its +0x880's +0x240 rotation when it has one,
/// not read), and the events only make sense if that is the party's facing towards the
/// monsters for the target too: `Skill_PARTY_ESCHA_0` puts her 600 to the target's −z, and a
/// camera stand-in 350 there looking at it, the eye behind that. So give a monster target the
/// party's facing, not its own.
#[derive(Clone, Copy, Debug)]
pub struct Cast {
    pub position: [f32; 3],
    pub facing: f32,
    pub measures: Option<Measures>,
}

/// The scene a frame ([`FRAME`]) apart, from its start to its last step, at most `limit`
/// seconds. `motion_length` gives a motion's length (character, motion, division), for the
/// event's waits on a motion's end; without it those waits take no time.
pub fn play(
    event: &Ebd,
    actor: Cast,
    target: Cast,
    curves: &BTreeMap<u32, barrel_engine::curve::Curve>,
    motion_length: Option<barrel_engine::event::MotionLength<'_>>,
    limit: f32,
) -> Result<Vec<Snapshot>, String> {
    let player = barrel_engine::event::Player::new(event, &[], &OperandTable::escha()).map_err(|e| e.to_string())?.anchored_places();
    let player = match motion_length {
        Some(m) => player.motion_length(m),
        None => player,
    };
    let steps = player.schedule();
    let mut measures = BTreeMap::new();
    let mut stage = Stage { curves: curves.clone(), ..Stage::default() };
    for (who, c) in [(ACTOR, actor), (TARGET, target)] {
        stage.apply(&barrel_engine::event::Step::Place { action: 0, character: who, position: c.position });
        if let Some(b) = stage.characters.get_mut(&who) {
            b.facing = c.facing;
        }
        if let Some(m) = c.measures {
            measures.insert(who, m);
        }
    }
    stage.measures = measures;
    let mut play = Playback::new(stage, steps);
    let mut out = vec![play.stage.snapshot()];
    while (!play.finished() || play.stage.characters.values().any(|b| b.moving())) && play.stage.time < limit {
        play.tick();
        out.push(play.stage.snapshot());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Escha's attack (`Skill_PARTY_ESCHA_0`) on a monster at the stage centre (given the
    /// party's facing, as [`Cast`] says): she
    /// starts 600 in front of it, runs in to strike, and goes back; the camera is set.
    #[test]
    #[ignore = "needs the game (BARREL_ROOT)"]
    fn eschas_attack_runs_in_and_back() {
        let root = std::path::PathBuf::from(std::env::var("BARREL_ROOT").expect("BARREL_ROOT"));
        let bytes = std::fs::read(root.join("Event/skill/Skill_PARTY_ESCHA_0.ebd")).unwrap();
        let event = Ebd::parse(&bytes).unwrap();
        let her = Cast { position: [0.0, 0.0, -800.0], facing: 0.0, measures: None };
        let it = Cast { position: [0.0, 0.0, 0.0], facing: 0.0, measures: None };
        // Every motion 0.6 s long (her attack's clips are not read here).
        let frames = play(&event, her, it, &BTreeMap::new(), Some(Box::new(|_, _, _| Some((0.6, false)))), 20.0).unwrap();
        let at = |k: usize| frames[k].characters[&ACTOR].0;
        let gap = |k: usize| at(k)[2].abs();
        let first = frames.iter().position(|f| f.characters[&ACTOR].0 != [0.0, 0.0, -800.0]).expect("she is placed");
        // Placed 600 off, and already a frame into her run (800/s).
        assert!((gap(first) - (600.0 - 800.0 * FRAME)).abs() < 1.0, "placed at {:?}", at(first));
        let nearest = (0..frames.len()).map(gap).fold(f32::MAX, f32::min);
        assert!((nearest - 170.0).abs() < 5.0, "closest {nearest}");
        // Back 600 from where she struck.
        assert!((gap(frames.len() - 1) - 770.0).abs() < 1.0, "ends at {:?}", at(frames.len() - 1));
        assert!(frames.iter().any(|f| f.eye.is_some() && f.target.is_some()), "a camera");
    }
}
