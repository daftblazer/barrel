//! A battle skill event (`Event/skill`, `docs/events.md`, "Which skill event an action plays")
//! played between a battle's fighters: the actor is character 10000 and its target 10001, as
//! the events name them; the others they use (10004 and up: stand-ins the events place
//! themselves, for the camera to aim at) come from the event.
//!
//! The result is the scene sampled each frame: where the actor and the target stand and face,
//! and the camera, for the battle playback to show in place of its own camera and placement.

use barrel_engine::stage::{Measures, Playback, Snapshot, Stage};
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
/// not read), and the events only make sense if, for the target, that points from the actor
/// to it: `Skill_PARTY_ESCHA_0` puts her 600 to the target's −z, and a camera stand-in 350
/// there looking at it, the eye behind that. So give the target the way from the actor to it,
/// not its own facing (with the party in a row facing the monsters, the party's facing).
#[derive(Clone, Copy, Debug)]
pub struct Cast {
    pub position: [f32; 3],
    pub facing: f32,
    pub measures: Option<Measures>,
}

/// A played scene: its frames, and its hits as (seconds from its start, share of the damage
/// in percent).
pub struct Scene {
    pub frames: Vec<Snapshot>,
    pub hits: Vec<(f32, i32)>,
    /// The actor's motion at each frame (its `MOTION_*` number and division), as the scene's
    /// `s5`/`s10`/`s13` set it.
    pub motions: Vec<Option<barrel_engine::stage::Playing>>,
    /// How many of its steps named something the scene does not have (`Stage::unresolved`).
    pub unresolved: usize,
}

/// `s1` (`ECT_CHARA_SET_MARKER`, `{character, marker, 0, 4, share}`): marker 0 is a hit landing,
/// dealing `share` percent of the action's damage (each event's shares add to 100: one 100 for
/// a single strike, `Skill_MONSTER_WARDOG_00_0` two); marker 2 (24 uses) is not read.
const SET_MARKER: u32 = 0x7300_0001;

/// `origin`: where the battle's own origin stands in the field. The events' field positions
/// (`SKILL_COMMON_SYSTEM_START_CAM_01` puts its camera stand-in at (0, 0, 0)) are the battle's,
/// as the party's places are (`battle::party_spot`); the casts and the frames are in the field's.
///
/// The scene a frame ([`barrel_engine::stage::FRAME`]) apart, from its start to its last step, at most `limit`
/// seconds. `motion_length` gives a motion's length (character, motion, division), for the
/// event's waits on a motion's end; without it those waits take no time.
pub fn play(
    event: &Ebd,
    actor: Cast,
    target: Cast,
    curves: &BTreeMap<u32, barrel_engine::curve::Curve>,
    motion_length: Option<barrel_engine::event::MotionLength<'_>>,
    limit: f32,
    origin: [f32; 3],
) -> Result<Scene, String> {
    let local = |p: [f32; 3]| [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
    let field = |p: [f32; 3]| [p[0] + origin[0], p[1] + origin[1], p[2] + origin[2]];
    let player = barrel_engine::event::Player::new(event, &[], &OperandTable::escha()).map_err(|e| e.to_string())?.anchored_places();
    let player = match motion_length {
        Some(m) => player.motion_length(m),
        None => player,
    };
    let steps = player.schedule();
    let hits = steps
        .iter()
        .filter_map(|t| match &t.step {
            barrel_engine::event::Step::Command { word: SET_MARKER, operands: [_, 0, _, _, share, ..], .. } => Some((t.time, *share as i32)),
            _ => None,
        })
        .collect();
    let mut measures = BTreeMap::new();
    let mut stage = Stage { curves: curves.clone(), ..Stage::default() };
    for (who, c) in [(ACTOR, actor), (TARGET, target)] {
        stage.apply(&barrel_engine::event::Step::Place { action: 0, character: who, position: local(c.position) });
        if let Some(b) = stage.characters.get_mut(&who) {
            b.facing = c.facing;
        }
        if let Some(m) = c.measures {
            measures.insert(who, m);
        }
    }
    stage.measures = measures;
    let mut play = Playback::new(stage, steps);
    let motion = |p: &Playback| p.stage.characters.get(&ACTOR).and_then(|b| b.motion);
    let snapshot = |p: &Playback| {
        let mut s = p.stage.snapshot();
        for c in s.characters.values_mut() {
            c.0 = field(c.0);
        }
        s.eye = s.eye.map(field);
        s.target = s.target.map(field);
        s
    };
    let (mut out, mut motions) = (vec![snapshot(&play)], vec![motion(&play)]);
    let busy = |p: &Playback| !p.finished() || p.stage.characters.values().any(|b| b.moving()) || p.stage.eye.moving() || p.stage.target.moving();
    while busy(&play) && play.stage.time < limit {
        play.tick();
        out.push(snapshot(&play));
        motions.push(motion(&play));
    }
    Ok(Scene { frames: out, hits, motions, unresolved: play.stage.unresolved })
}

#[cfg(test)]
mod tests {
    use super::*;
    use barrel_engine::stage::FRAME;

    /// The battle's start camera (`SKILL_COMMON_SYSTEM_START_CAM_01`) looks at the battle's
    /// origin, wherever that stands in the field: from 1,200 to the side and 500 up, in to
    /// 1,000 and 200 over a second.
    #[test]
    #[ignore = "needs the game (BARREL_ROOT)"]
    fn the_start_camera_flies_in_on_the_battle_origin() {
        let root = std::path::PathBuf::from(std::env::var("BARREL_ROOT").expect("BARREL_ROOT"));
        let bytes = std::fs::read(root.join("Event/skill/SKILL_COMMON_SYSTEM_START_CAM_01.ebd")).unwrap();
        let event = Ebd::parse(&bytes).unwrap();
        let nobody = Cast { position: [3200.0, 0.0, 3200.0], facing: 0.0, measures: None };
        let scene = play(&event, nobody, nobody, &BTreeMap::new(), None, 10.0, [3200.0, 0.0, 3200.0]).unwrap();
        let first = scene.frames.iter().find_map(|f| f.eye.zip(f.target)).unwrap();
        let (eye, target) = scene.frames.last().and_then(|f| f.eye.zip(f.target)).unwrap();
        // (Its first frame is already a tick into the move.)
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 20.0);
        assert!(near(first.0, [2000.0, 500.0, 3600.0]) && near(first.1, [2700.0, 100.0, 3200.0]), "{first:?}");
        assert_eq!((eye, target), ([2200.0, 200.0, 3400.0], [3200.0, 50.0, 3200.0]));
        assert!((scene.frames.len() as f32 * FRAME - 1.0).abs() < 0.1, "{} frames", scene.frames.len());
    }

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
        let scene = play(&event, her, it, &BTreeMap::new(), Some(Box::new(|_, _, _| Some((0.6, false)))), 20.0, [0.0; 3]).unwrap();
        // Her attack motion (14, ATTACK00) in its divisions 0 to 6.
        let divisions: Vec<Option<u32>> = scene.motions.iter().flatten().filter(|p| p.motion == 14).map(|p| p.division).collect();
        assert!(divisions.contains(&Some(0)) && divisions.contains(&Some(6)), "{:?}", divisions.last());
        let frames = scene.frames;
        // One hit, the whole damage, once she has run in.
        assert_eq!(scene.hits.len(), 1);
        assert_eq!(scene.hits[0].1, 100);
        let at = |k: usize| frames[k].characters[&ACTOR].0;
        let gap = |k: usize| at(k)[2].abs();
        let struck = (scene.hits[0].0 / FRAME) as usize;
        assert!((gap(struck.min(frames.len() - 1)) - 170.0).abs() < 1.0, "hit at {:?}", at(struck));
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
