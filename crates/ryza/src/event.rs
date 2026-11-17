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
pub fn commands(event: &Ebd, given: &[f32], until: f32) -> Result<Vec<(f32, usize, Vec<u32>)>, String> {
    let table = OperandTable::ryza();
    let scripts: Vec<_> = event.actions.iter().map(|a| barrel_formats::ebd::decode(&a.words, &table)).collect::<Result<_, _>>().map_err(|e| format!("{e}"))?;
    let (_, taken) = drawn_player(event, given, 0, until)?.schedule_traced();
    Ok(taken
        .into_iter()
        .filter_map(|(time, action, at)| {
            let c = scripts.get(action)?.get(at)?;
            Some((time, action, std::iter::once(c.word).chain(c.operands.iter().copied()).collect()))
        })
        .collect())
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
