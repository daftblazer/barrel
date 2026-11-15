//! The line a character says when the party enters a field (the entry message window), chosen
//! from the executable's field talk ([`crate::data::field_talk`]) the way the game does
//! (`0x140079770` on entering, then `0x140289c70` for the map's own lines and `0x140287fa0` for
//! the general ones, both over the lookup `0x14028a2a0`):
//!
//! 1. The candidates of a kind are the lines of its groups, in table order, that a squad member
//!    says (the line's face is one of the squad's eleven slots; a face repeated in the slots
//!    counts once per slot), whose events pass (the needed one seen, the barred one not) and
//!    which have not been said yet this session.
//! 2. The map's own lines (kind 9, the groups naming this map) come first: when any exist for
//!    the map, whether said or not, the general lines are not tried; one candidate is drawn at
//!    random.
//! 3. Otherwise, outside the towns (the maps whose region is 0), one candidate of the general
//!    lines (kind 0) is drawn at random.
//!
//! The draw is the game's `0x1404ca660` (0 to the count less one; one candidate draws
//! nothing). The game's generator carries on from the time it was seeded at boot and is not
//! saved, so the line differs from run to run; [`Draw`] names which candidate to take where a
//! check needs the same one every time.

use barrel_engine::rng::Rng;

use crate::data::field_talk::{Line, Talk};

/// The kind of the groups of lines for entering any field.
pub const KIND_GENERAL: i32 = 0;
/// The kind of the groups of lines for entering one map.
pub const KIND_MAP: i32 = 9;

/// What the choice reads of the session.
pub struct Situation<'a> {
    /// The map entered, a `FIELDMAP_*` number.
    pub map: i32,
    /// The map is a town (its region is 0): no general lines there.
    pub town: bool,
    /// The squad's character numbers by slot (-1 for an empty one).
    pub squad: &'a [i32],
    /// Which events have been seen, by event number.
    pub seen: &'a [bool],
    /// The lines said so far this session, by line number.
    pub said: &'a [usize],
}

/// How the random pick is made.
pub enum Draw<'a> {
    /// The game's generator.
    Rng(&'a mut Rng),
    /// The candidate at this position (the last one when it is past the end).
    Index(usize),
}

impl Draw<'_> {
    fn pick(&mut self, count: usize) -> usize {
        match self {
            Draw::Rng(r) => r.range(0, count as i32 - 1) as usize,
            Draw::Index(i) => (*i).min(count - 1),
        }
    }
}

/// The lines of `kind`'s groups a squad member could say now: all of them, and those not said
/// yet (`0x14028a2a0`).
pub fn candidates<'a>(talk: &'a Talk, kind: i32, s: &Situation) -> (Vec<&'a Line>, Vec<&'a Line>) {
    let event = |e: Option<usize>| e.is_some_and(|e| s.seen.get(e).copied().unwrap_or(false));
    let (mut all, mut fresh) = (Vec::new(), Vec::new());
    for line in talk.lines.iter().filter(|l| talk.group_of(l).kind == kind) {
        // `0x14035f2d0`: the needed event seen (or none needed), the barred one not.
        if !(line.needs.is_none() || event(line.needs)) || event(line.unless) {
            continue;
        }
        for _ in s.squad.iter().filter(|&&c| c == line.face) {
            all.push(line);
            if !s.said.contains(&line.id) {
                fresh.push(line);
            }
        }
    }
    (all, fresh)
}

/// The line to say on entering the map, if any.
pub fn pick<'a>(talk: &'a Talk, s: &Situation, mut draw: Draw) -> Option<&'a Line> {
    let (all, fresh) = candidates(talk, KIND_MAP, s);
    let own = |l: &&&Line| talk.group_of(l).param == s.map;
    if all.iter().any(|l| own(&l)) {
        let fresh: Vec<_> = fresh.iter().filter(own).collect();
        return (!fresh.is_empty()).then(|| *fresh[draw.pick(fresh.len())]);
    }
    if s.town {
        return None;
    }
    let (_, fresh) = candidates(talk, KIND_GENERAL, s);
    (!fresh.is_empty()).then(|| fresh[draw.pick(fresh.len())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::field_talk::{read, Group};

    fn line(id: usize, group: usize, face: i32, needs: Option<usize>, unless: Option<usize>) -> Line {
        Line { id, group, face, needs, unless, text_id: 0, text: format!("line {id}") }
    }

    fn talk() -> Talk {
        Talk {
            groups: vec![Group { kind: 0, param: -1 }, Group { kind: 9, param: 5 }],
            lines: vec![line(0, 0, 0, None, None), line(1, 0, 1, None, None), line(2, 0, 1, Some(7), None), line(3, 1, 1, None, Some(8))],
        }
    }

    #[test]
    fn general_lines_follow_the_squad_and_the_events() {
        let t = talk();
        let mut s = Situation { map: 12, town: false, squad: &[0, 1, -1], seen: &[false; 10], said: &[] };
        let text = |s: &Situation| candidates(&t, KIND_GENERAL, s).1.iter().map(|l| l.id).collect::<Vec<_>>();
        assert_eq!(text(&s), [0, 1]);
        let mut seen = [false; 10];
        seen[7] = true;
        s.seen = &seen;
        assert_eq!(text(&s), [0, 1, 2]);
        s.said = &[1];
        assert_eq!(text(&s), [0, 2]);
        s.squad = &[1];
        assert_eq!(text(&s), [2]);
        assert_eq!(pick(&t, &s, Draw::Index(9)).map(|l| l.id), Some(2));
    }

    #[test]
    fn a_map_with_its_own_lines_skips_the_general_ones() {
        let t = talk();
        let mut s = Situation { map: 5, town: false, squad: &[0, 1], seen: &[false; 10], said: &[] };
        assert_eq!(pick(&t, &s, Draw::Index(0)).map(|l| l.id), Some(3));
        // Said already: nothing, not a general line.
        s.said = &[3];
        assert_eq!(pick(&t, &s, Draw::Index(0)), None);
        // Towns have no general lines.
        s.map = 3;
        s.town = true;
        assert_eq!(pick(&t, &s, Draw::Index(0)), None);
    }

    #[test]
    fn the_draw_is_the_games() {
        let t = talk();
        let s = Situation { map: 12, town: false, squad: &[0, 1], seen: &[false; 10], said: &[] };
        let mut rng = Rng::seeded(1);
        let want = Rng::seeded(1).range(0, 1) as usize;
        assert_eq!(pick(&t, &s, Draw::Rng(&mut rng)).map(|l| l.id), Some(want));
    }

    /// The real table: entering the Southern Ruins (map 26) with the alchemists, a new save's
    /// events, the six lines of Escha and Logy that need no event are the candidates and the
    /// sixth is the one the game showed.
    #[test]
    #[ignore = "needs the game's executable (BARREL_EXE)"]
    fn the_ruins_offer_the_known_line() {
        let Ok(path) = std::env::var("BARREL_EXE") else { return };
        let exe = std::fs::read(path).unwrap();
        let talk = read(&barrel_formats::pe::Pe::parse(&exe).unwrap()).unwrap();
        let s = Situation { map: 26, town: false, squad: &[0, 1], seen: &[false; 1165], said: &[] };
        let (_, fresh) = candidates(&talk, KIND_GENERAL, &s);
        assert_eq!(fresh.iter().map(|l| l.id).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
        let l = pick(&talk, &s, Draw::Index(5)).unwrap();
        assert_eq!((l.face, l.text.as_str()), (1, "This place is worth investigating. We might find something interesting."));
    }
}
