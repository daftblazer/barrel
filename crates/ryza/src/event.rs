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

/// The event's steps on its timeline (no messages read yet: its lines come up by number).
pub fn schedule(event: &Ebd) -> Result<Vec<Timed<'_>>, String> {
    let player = Player::translated(event, &[], &OperandTable::ryza(), barrel_formats::ebd_names::RYZA, barrel_formats::ebd_names::RYZA_SAME_AS_ESCHA).map_err(|e| format!("event: {e}"))?;
    Ok(player.schedule())
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
    fn paths_by_chapter() {
        assert_eq!(path("mm00_010").as_deref(), Some("\\event\\event_jp\\mm00\\event_mm00_010.ebd"));
        assert_eq!(path("mm00"), None);
    }

    /// The opening (`BARREL_RYZA_OPENING` names `event_mm00_010.ebd`): its first telop fades
    /// in from 2 s over 1 s, holds, and fades out from 5 s; the second follows at 5.5 s.
    #[test]
    fn the_openings_telops() {
        let Some(p) = std::env::var_os("BARREL_RYZA_OPENING") else { return };
        let event = Ebd::parse(&std::fs::read(p).unwrap()).unwrap();
        let at = |t: f32| telop(&stage_at(schedule(&event).unwrap(), t));
        assert_eq!(at(1.9), None);
        let (n, a) = at(2.5).unwrap();
        assert_eq!(n, 15);
        assert!((a - 0.5).abs() < 0.05, "{a}");
        assert_eq!(at(4.0), Some((15, 1.0)));
        assert_eq!(at(5.6).map(|(n, _)| n), Some(16));
    }
}
