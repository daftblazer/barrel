//! Ryza's events (`\event\event_<lang>\<chapter>\event_<name>.ebd`, docs/events.md, "Atelier
//! Ryza DX's events"): read with Ryza's operand table, carried over to the shared player's
//! commands, and played on the shared stage's clock.

use barrel_engine::event::{Player, Timed};
use barrel_engine::stage::{Playback, Stage};
use barrel_formats::ebd::{Ebd, OperandTable};

/// An event's script in the archives: its chapter is the name's first part (`mm00_010` is in
/// `mm00`). The scripts are under `event_jp` only; the languages differ in their messages.
pub fn path(name: &str) -> Option<String> {
    let chapter = name.split('_').next().filter(|c| !c.is_empty() && c.len() < name.len())?;
    Some(format!("\\event\\event_jp\\{chapter}\\event_{name}.ebd"))
}

// STAND-IN: Events are read without their messages; the real thing is the game's message tables playing each line.
/// The event's steps on its timeline (no messages read yet: its lines come up by number).
pub fn schedule(event: &Ebd) -> Result<Vec<Timed<'_>>, String> {
    let player = Player::translated(event, &[], &OperandTable::ryza(), barrel_formats::ebd_names::RYZA, barrel_formats::ebd_names::RYZA_SAME_AS_ESCHA).map_err(|e| format!("event: {e}"))?.counted_waits();
    Ok(player.schedule())
}

/// The number the game's `u104` draws when the clock reads `seconds` (`0x140479dec`): it seeds
/// the C library's generator with the time, throws one number away and maps the next onto
/// `low..=high` in equal shares of 0..0x7fff, drawing again above the last whole share.
pub fn clock_pick(seconds: u32, low: i32, high: i32) -> i32 {
    let n = high.saturating_add(1).saturating_sub(low);
    if n <= 0 {
        return low;
    }
    let mut state = seconds;
    let mut rand = || {
        state = state.wrapping_mul(214013).wrapping_add(2531011);
        (state >> 16 & 0x7fff) as i32
    };
    rand();
    let limit = 0x7fff - 0x7fff % n;
    let mut r = rand();
    while r > limit {
        r = rand();
    }
    // the game's count runs one past `high` when the number is exactly the limit; kept in range
    (low + r / (0x7fff / n)).min(high)
}

/// A looping event's steps (the title's `tt01_010`: a wait and one of three actions, drawn
/// each time round), up to `until` seconds. Its random commands take `given` numbers first,
/// in the order the event asks (for playing what a recorded run of the game drew), then draw
/// as the game does: a pick from the clock's seconds (`clock` at the event's start, plus the
/// waits so far), a wait anywhere in its range.
pub fn schedule_drawn<'a>(event: &'a Ebd, given: &[f32], clock: u32, until: f32) -> Result<Vec<Timed<'a>>, String> {
    Ok(drawn_player(event, given, clock, until)?.schedule())
}

/// A command the player took, as the script has it: when, in which action, and its words (the
/// word, then its operands). The steps carry Escha's words for the commands carried over;
/// these are Ryza's own.
pub type Taken = (f32, usize, Vec<u32>);

/// [`schedule_drawn`] with the commands taken ([`Taken`]) from the same run.
pub fn schedule_drawn_taken<'a>(event: &'a Ebd, given: &[f32], clock: u32, until: f32) -> Result<(Vec<Timed<'a>>, Vec<Taken>), String> {
    let table = OperandTable::ryza();
    let scripts: Vec<_> = event.actions.iter().map(|a| barrel_formats::ebd::decode(&a.words, &table)).collect::<Result<_, _>>().map_err(|e| format!("{e}"))?;
    let (steps, taken) = drawn_player(event, given, clock, until)?.schedule_traced();
    let taken = taken
        .into_iter()
        .filter_map(|(time, action, at)| {
            let c = scripts.get(action)?.get(at)?;
            Some((time, action, std::iter::once(c.word).chain(c.operands.iter().copied()).collect()))
        })
        .collect();
    Ok((steps, taken))
}

fn drawn_player<'a>(event: &'a Ebd, given: &[f32], clock: u32, until: f32) -> Result<Player<'a>, String> {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    let given = Rc::new(RefCell::new(given.iter().copied().collect::<std::collections::VecDeque<f32>>()));
    let waited = Rc::new(Cell::new(0.0f32));
    let (g, w) = (given.clone(), waited.clone());
    let whole = move |low: i32, high: i32| match g.borrow_mut().pop_front() {
        Some(n) => n as i32,
        None => clock_pick(clock.wrapping_add(w.get() as u32), low, high),
    };
    let mut state = clock | 1;
    let seconds = move |low: f32, high: f32| {
        let s = given.borrow_mut().pop_front().unwrap_or_else(|| {
            // xorshift: any even spread will do, the game's own generator is shared with all it draws
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            low + (high - low) * (state >> 8) as f32 / (1u32 << 24) as f32
        });
        waited.set(waited.get() + s);
        s
    };
    let chance = barrel_engine::event::Chance { whole: Box::new(whole), seconds: Box::new(seconds) };
    let player = Player::translated(event, &[], &OperandTable::ryza(), barrel_formats::ebd_names::RYZA, barrel_formats::ebd_names::RYZA_SAME_AS_ESCHA).map_err(|e| format!("event: {e}"))?;
    Ok(player.counted_waits().chance(chance).until(until))
}

/// The event's commands as Barrel's player takes them, for comparing with the game's own log
/// (ryza-recomp's `tools/cmdlog.py --hex`): the time in seconds, the action, and the command's
/// words as the script has them (the word, then its operands). `given` are the random commands'
/// numbers in the order asked for (the lowest after them) and `until` where a looping event stops.
pub fn commands(event: &Ebd, given: &[f32], until: f32) -> Result<Vec<Taken>, String> {
    Ok(schedule_drawn_taken(event, given, 0, until)?.1)
}

/// `q5` (`ECT_FIELDMAP_CHARA_ADDITION_EX`): `{slot, 0, 0, 0, x, y, z, rx, ry, rz, 0, 0, 2}`, adds
/// a character at a position turned by three angles in whole degrees.
pub const CHARA_ADD: u32 = 0x7100_0005;

/// Where and how an event's `q5` adds a slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Added {
    pub position: [f32; 3],
    /// Degrees about x, y and z.
    pub degrees: [f32; 3],
}

/// What the event's `q5` commands add (the last one for a slot). The shared stage does not take
/// them over (Ryza's `q5` has one operand fewer than Escha's `q4`, which keeps only the yaw): the
/// props lying in the scene (the weapons) are read from here, with all three angles
/// ([`rotation`]).
pub fn added(event: &Ebd) -> Result<std::collections::BTreeMap<u32, Added>, String> {
    let table = OperandTable::ryza();
    let mut out = std::collections::BTreeMap::new();
    for a in &event.actions {
        for c in barrel_formats::ebd::decode(&a.words, &table).map_err(|e| format!("{e}"))? {
            if let (CHARA_ADD, [slot, _, _, _, x, y, z, rx, ry, rz, ..]) = (c.word, c.operands) {
                let whole = |v: &u32| *v as i32 as f32;
                out.insert(*slot, Added { position: [x, y, z].map(whole), degrees: [rx, ry, rz].map(whole) });
            }
        }
    }
    Ok(out)
}

/// The turn of a prop added at angles `degrees` about x, y and z: a right-handed turn about x,
/// then y, then z (the matrix `Rz Ry Rx`). It reproduces the game's world matrices of the
/// title's three weapons from their `q5` angles (see the test).
pub fn rotation(degrees: [f32; 3]) -> glam::Quat {
    let [x, y, z] = degrees.map(f32::to_radians);
    glam::Quat::from_euler(glam::EulerRot::ZYX, z, y, x)
}

/// The stage `seconds` into the event, played tick by tick from its start.
pub fn stage_at(steps: Vec<Timed<'_>>, seconds: f32) -> Stage {
    let mut play = Playback::new(Stage::default(), steps);
    while play.stage.time < seconds {
        play.tick();
    }
    play.stage
}

/// The telop on screen in a stage, and its opacity (none when it is faded out).
pub fn telop(stage: &Stage) -> Option<(u32, f32)> {
    let alpha = stage.telop_alpha.now();
    stage.telop.filter(|_| alpha > 0.0).map(|n| (n, alpha.min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn props_turn_as_the_games_matrices() {
        // The game's local-to-world rotations of the title's three weapons (its draw capture)
        // against the angles of their `q5` commands.
        let cases = [
            ([-35.0, -295.0, -280.0], [[0.0734, -0.8970, -0.4360], [0.4162, -0.3697, 0.8307], [-0.9063, -0.2424, 0.3462]]),
            ([-19.0, -18.0, -129.0], [[-0.5985, 0.6715, 0.4369], [-0.7391, -0.6732, 0.0222], [0.3090, -0.3096, 0.8992]]),
            ([-48.0, -41.0, -100.0], [[-0.1311, 0.5743, 0.8081], [-0.7432, -0.5963, 0.3033], [0.6561, -0.5609, 0.5050]]),
        ];
        for (angles, rows) in cases {
            let m = glam::Mat3::from_quat(rotation(angles));
            for (r, row) in rows.iter().enumerate() {
                for (c, want) in row.iter().enumerate() {
                    assert!((m.col(c)[r] - want).abs() < 1e-3, "{angles:?} [{r}][{c}]: {} against {want}", m.col(c)[r]);
                }
            }
        }
    }

    #[test]
    fn picks_from_the_clock_stay_in_range_and_spread() {
        let mut seen = [0u32; 3];
        for t in 0..3000 {
            let n = clock_pick(1_700_000_000 + t, 0, 2);
            assert!((0..=2).contains(&n));
            seen[n as usize] += 1;
        }
        assert!(seen.iter().all(|&c| c > 800), "{seen:?}");
        assert_eq!(clock_pick(5, 4, 4), 4);
        // the C library's second number after srand(1) is 18467: the middle share of three
        assert_eq!(clock_pick(1, 0, 2), 1);
    }

    #[test]
    fn paths_by_chapter() {
        assert_eq!(path("mm00_010").as_deref(), Some("\\event\\event_jp\\mm00\\event_mm00_010.ebd"));
        assert_eq!(path("mm00"), None);
    }

    /// The opening (`BARREL_RYZA_OPENING` names `event_mm00_010.ebd`): its first telop fades
    /// in from 2 s over 1 s, holds, and fades out from 5 s; the second follows at 5.5 s.
    #[test]
    fn the_openings_telops() {
        let Some(p) = std::env::var_os("BARREL_RYZA_OPENING") else { return };
        let event = Ebd::parse(&barrel_files::read(p).unwrap()).unwrap();
        let at = |t: f32| telop(&stage_at(schedule(&event).unwrap(), t));
        assert_eq!(at(1.9), None);
        let (n, a) = at(2.5).unwrap();
        assert_eq!(n, 15);
        assert!((a - 0.5).abs() < 0.05, "{a}");
        assert_eq!(at(4.0), Some((15, 1.0)));
        assert_eq!(at(5.6).map(|(n, _)| n), Some(16));
    }
}
