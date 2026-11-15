//! Barrel's damage rules against the hits the real game dealt (escha-recomp's `battle-hits`
//! scenario: a field battle from the user's save, encounter group 25, `hits.jsonl`, one line per
//! hit with both fighters' state words and the resolved action's). For each hit the test builds
//! the attacker and the target as the play binary does (`auto_battle::Tables`: the growth rows,
//! the equipment kit, the save's maxima; monsters from the stat blocks), finds the action's skill
//! (action word 17 is its row in the skill table), and asks `battle::skill_damage` and `reduce`
//! for the range the roll gives, with and without the damage rate's steps.
//!
//! Skipped with a message when the install, the save or the hit log is absent. Set
//! `BARREL_ESCHA_ROOT` (the install folder) and `BARREL_HITS` (the scenario folder) to move them.

use barrel_escha::auto_battle::Tables;
use barrel_escha::battle::{reduce, skill_damage, stat, Defender};
use barrel_escha::data::battle_data as bd;
use barrel_escha::data::save;
use barrel_escha::event_start::Chain;
use barrel_escha::session::Session;
use std::path::{Path, PathBuf};

/// A JSON value: only what the hit log holds.
#[derive(Debug, Clone)]
enum J {
    Num(i64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(o) => &o.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no key {key}")).1,
            _ => panic!("not an object"),
        }
    }
    fn num(&self) -> i64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("not a number"),
        }
    }
    fn text(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => panic!("not a string"),
        }
    }
    fn nums(&self) -> Vec<i64> {
        match self {
            J::Arr(a) => a.iter().map(J::num).collect(),
            _ => panic!("not an array"),
        }
    }
}

fn parse(s: &[u8], at: &mut usize) -> J {
    let skip = |at: &mut usize| {
        while s[*at].is_ascii_whitespace() {
            *at += 1;
        }
    };
    skip(at);
    match s[*at] {
        b'{' => {
            *at += 1;
            let mut o = Vec::new();
            loop {
                skip(at);
                if s[*at] == b'}' {
                    *at += 1;
                    return J::Obj(o);
                }
                if s[*at] == b',' {
                    *at += 1;
                    continue;
                }
                let J::Str(k) = parse(s, at) else { panic!("key") };
                skip(at);
                *at += 1; // the colon
                o.push((k, parse(s, at)));
            }
        }
        b'[' => {
            *at += 1;
            let mut a = Vec::new();
            loop {
                skip(at);
                if s[*at] == b']' {
                    *at += 1;
                    return J::Arr(a);
                }
                if s[*at] == b',' {
                    *at += 1;
                    continue;
                }
                a.push(parse(s, at));
            }
        }
        b'"' => {
            let start = *at + 1;
            *at = start;
            while s[*at] != b'"' {
                *at += 1;
            }
            *at += 1;
            J::Str(String::from_utf8_lossy(&s[start..*at - 1]).into_owned())
        }
        _ => {
            let start = *at;
            while *at < s.len() && (s[*at] == b'-' || s[*at].is_ascii_digit()) {
                *at += 1;
            }
            J::Num(std::str::from_utf8(&s[start..*at]).unwrap().parse().expect("number"))
        }
    }
}

/// The install folder, the scenario folder and the save inside it, when all are there.
fn locate() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let root = std::env::var_os("BARREL_ESCHA_ROOT").map(PathBuf::from).unwrap_or_else(|| "<steam>/steamapps/common/Atelier Escha and Logy DX".into());
    let scenario = std::env::var_os("BARREL_HITS").map(PathBuf::from).unwrap_or_else(|| "../escha-recomp/work/scenarios/battle-hits".into());
    let save = scenario.join("drive_c/users/player/Documents/KoeiTecmo/Atelier Escha and Logy DX/SAVEDATA/GAMEDATA00.pcsave");
    (root.join("Atelier_Escha_and_Logy_EN.exe").exists() && scenario.join("hits.jsonl").exists() && save.exists()).then_some((root, scenario, save))
}

fn read_table(path: &Path) -> barrel_formats::table::Element {
    barrel_formats::table::parse(&std::fs::read(path).unwrap()).unwrap()
}

/// A fighter's stats as the game's words hold them (stat k at word 34 + k).
fn game_stats(words: &[i64]) -> Vec<i32> {
    words[34..34 + stat::COUNT].iter().map(|&w| w as i32).collect()
}

/// Barrel's fighter for a log entry: the stat vector [level, HP, MP, max HP, max MP, attack,
/// defence, speed, resistances 0 to 3], from the same source the battle uses.
fn fighter_stats(tables: &Tables, monsters: &[bd::Monster], who: &J) -> Vec<i32> {
    let (id, level) = (who.get("id").num() as usize, who.get("level").num() as i32);
    let (block, mp) = if who.get("kind").text() == "party" {
        tables.party_block(id, level).unwrap()
    } else {
        (monsters[id].blocks[0].clone(), 0)
    };
    let mut s = vec![0; stat::COUNT];
    s[stat::LEVEL] = block.level;
    s[stat::MAX_HP] = block.hp;
    s[stat::MAX_MP] = mp;
    s[stat::ATTACK] = block.attack;
    s[stat::DEFENSE] = block.defense;
    s[stat::SPEED] = block.speed;
    s[stat::RESIST..stat::RESIST + 4].copy_from_slice(&block.resist);
    if who.get("kind").text() == "party" {
        s[stat::POWER] = tables.party_power(id, level);
    }
    s
}

const COMPARED: [usize; 10] = [stat::LEVEL, stat::MAX_HP, stat::ATTACK, stat::DEFENSE, stat::SPEED, 8, 9, 10, 11, stat::POWER];

#[test]
fn barrel_deals_the_damage_the_game_dealt() {
    let Some((root, scenario, save_path)) = locate() else {
        eprintln!("battle_hits: no game install, save or hit log; skipped");
        return;
    };
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).unwrap();
    let pe = barrel_formats::pe::Pe::parse(&exe).unwrap();
    let effects = read_table(&root.join("Saves_EN/item/item_effect.xml"));
    let chara = std::fs::read(root.join("Saves/chara/Chara.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let items = read_table(&root.join("Saves_EN/item/itemData.xml"));
    let mut tables = Tables::load(&pe, &effects, chara.as_ref(), Some(&items)).unwrap().with_traits(&read_table(&root.join("Saves_EN/item/item_potential.xml")));
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).unwrap();
    let skills = bd::skills(&pe, bd::SKILLS_EN).unwrap();

    // The party as Session::from_game_save and the play binary give it: equipment by item, the
    // save's maxima and current HP and MP.
    let bytes = std::fs::read(&save_path).unwrap();
    let state = save::decode(save::parse(&bytes).unwrap().body).unwrap();
    let summary = save::summary(&save::state_records(&state).unwrap());
    for m in &summary.party {
        let c = m.character as usize;
        tables.equip(c, &m.equipment);
        let units: Vec<_> = m.units.iter().flatten().copied().collect();
        tables.equip_units(c, &units);
        tables.current(c, [m.hp, m.equipped[0], m.mp, m.equipped[1]]);
    }

    let log = std::fs::read(scenario.join("hits.jsonl")).unwrap();
    let hits: Vec<J> = log.split(|&b| b == b'\n').filter(|l| !l.is_empty()).map(|l| parse(l, &mut 0)).collect();
    assert_eq!(hits.len(), 14);

    let barrel_stats = |who: &J| fighter_stats(&tables, &monsters, who);

    // The damage range of a hit: every roll of the skill's first damage effect at `rate`.
    let range = |skill: &bd::Skill, user: &[i32], foe: &[i32], rate: i32| -> (i32, i32) {
        let e = skill.effects.iter().find(|e| matches!(e.act, 0 | 6)).expect("a damage effect");
        // The rate's scale, then the user's skill power (effect percent 2), as `Battle::act`.
        let scale = (100 + rate) as f32 / 100.0 * (100 + user[stat::POWER]) as f32 / 100.0;
        let at = |amount: i32| {
            let d = skill_damage(amount, user[stat::ATTACK], user[stat::LEVEL], foe[stat::DEFENSE], e.element.is_none(), scale);
            let resist = [foe[8], foe[9], foe[10], foe[11]];
            barrel_escha::battle::hit_amount(reduce(d, e.element, &Defender { resist, ..Defender::default() }), 0)
        };
        (at(e.min), at(e.max))
    };

    let (mut stat_diffs, mut inside, mut inside_game_stats, mut inside_any_rate, mut implied_inside) = (0, 0, 0, 0, 0);
    for h in &hits {
        let (user, target) = (h.get("user"), h.get("target"));
        let action = h.get("action").nums();
        let skill = &skills[action[17] as usize];
        let damage = h.get("damage").num() as i32;
        let (bu, bt) = (barrel_stats(user), barrel_stats(target));
        let (gu, gt) = (game_stats(&user.get("words").nums()), game_stats(&target.get("words").nums()));
        let mut diffs = Vec::new();
        for (who, b, g) in [("user", &bu, &gu), ("target", &bt, &gt)] {
            for &k in &COMPARED {
                if b[k] != g[k] {
                    diffs.push(format!("{who} stat {k}: Barrel {} game {}", b[k], g[k]));
                }
            }
        }
        stat_diffs += diffs.len();
        let party = user.get("kind").text() == "party";
        let element = skill.effects.iter().find(|e| matches!(e.act, 0 | 6)).and_then(|e| e.element).map_or(-1, i32::from);
        assert_eq!(element as i64, h.get("element").num(), "frame {}: the skill's element", h.get("frame").num());
        let (lo, hi) = range(skill, &bu, &bt, 0);
        let (glo, ghi) = range(skill, &gu, &gt, 0);
        // The rates the steps can add up to: none, critical (10), flank (25), both (35); a
        // monster's action has none.
        let rates: &[i32] = if party { &[0, 10, 25, 35] } else { &[0] };
        // The rate the log's flags imply: a critical (the user's +0x2f0, word 188) is 10. The
        // flank flag (+0x2e8) is not read: by the time of the hook the game has cleared or
        // re-set it, and a flank step (25) would show as a 25 % excess no hit here has.
        let implied = 10 * i32::from(party && user.get("words").nums()[0x2f0 / 4] != 0);
        let (ilo, ihi) = range(skill, &bu, &bt, implied);
        let implied_ok = (ilo..=ihi).contains(&damage);
        implied_inside += usize::from(implied_ok);
        let fits: Vec<i32> = rates.iter().copied().filter(|&r| { let (a, b) = range(skill, &bu, &bt, r); (a..=b).contains(&damage) }).collect();
        let any = !fits.is_empty();
        let ok = (lo..=hi).contains(&damage);
        inside += usize::from(ok);
        inside_game_stats += usize::from((glo..=ghi).contains(&damage));
        inside_any_rate += usize::from(any);
        eprintln!(
            "frame {:5} {} {} ({}) -> {} {}: game {damage:3}, Barrel {lo}..={hi} (game stats {glo}..={ghi}), implied rate {implied}: {ilo}..={ihi}{}{}{}",
            h.get("frame").num(),
            user.get("kind").text(),
            user.get("id").num(),
            skill.name,
            target.get("kind").text(),
            target.get("id").num(),
            if implied_ok { "" } else { "  OUTSIDE" },
            if !implied_ok && any { format!(" (inside at rate {fits:?})") } else { String::new() },
            if diffs.is_empty() { String::new() } else { format!("  [{}]", diffs.join("; ")) },
        );
    }
    eprintln!("battle_hits: {implied_inside}/14 inside Barrel's range at the rate the flags imply, {inside}/14 at rate 0, {inside_any_rate}/14 allowing rate steps, {inside_game_stats}/14 with the game's own stats; {stat_diffs} stat differences");
    assert_eq!(stat_diffs, 0, "Barrel's stats differ from the game's");
    assert_eq!(implied_inside, 14, "some hits are outside Barrel's range at the rate their flags imply");
}

/// The party the play binary builds from a game save (`Session::open`, then `Tables::outfit`
/// and `Tables::current` as `battle_people` does) fights with the stats the game logged for
/// each party member, and Barrel's text save keeps the worn items it was built from.
#[test]
fn the_play_path_party_has_the_games_stats() {
    let Some((root, scenario, save_path)) = locate() else {
        eprintln!("battle_hits: no game install, save or hit log; skipped");
        return;
    };
    let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).unwrap();
    let pe = barrel_formats::pe::Pe::parse(&exe).unwrap();
    let effects = read_table(&root.join("Saves_EN/item/item_effect.xml"));
    let chara = std::fs::read(root.join("Saves/chara/Chara.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let items = read_table(&root.join("Saves_EN/item/itemData.xml"));
    let mut tables = Tables::load(&pe, &effects, chara.as_ref(), Some(&items)).unwrap().with_traits(&read_table(&root.join("Saves_EN/item/item_potential.xml")));
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).unwrap();
    let landmarks = barrel_escha::data::world::Landmarks::from_exe(&pe).unwrap();
    let chain = || Chain::new(Vec::new(), Vec::new(), Vec::new());

    let (session, at) = Session::open(chain(), &std::fs::read(&save_path).unwrap(), &landmarks).unwrap();
    // Barrel's text save of it keeps the units: read back, the members are the same.
    let (back, _) = Session::load(chain(), &session.save(at).unwrap()).unwrap();
    for (c, (a, b)) in session.party.iter().zip(&back.party).enumerate() {
        assert_eq!((&a.equipment, &a.worn), (&b.equipment, &b.worn), "member {c}: worn items through a text save");
    }
    assert!(session.party.iter().any(|m| m.worn.iter().any(Option::is_some)), "the game save holds worn units");

    // The party of the session read back from the text, with the vitals of the game save.
    for (c, m) in back.party.iter().enumerate() {
        tables.outfit(c, m);
        if let Some(vitals) = session.party[c].now {
            tables.current(c, vitals);
        }
    }
    let log = std::fs::read(scenario.join("hits.jsonl")).unwrap();
    let (mut diffs, mut checked) = (Vec::new(), 0);
    for h in log.split(|&b| b == b'\n').filter(|l| !l.is_empty()).map(|l| parse(l, &mut 0)) {
        for who in [h.get("user"), h.get("target")].into_iter().filter(|w| w.get("kind").text() == "party") {
            let (b, g) = (fighter_stats(&tables, &monsters, who), game_stats(&who.get("words").nums()));
            checked += 1;
            for &k in &COMPARED {
                if b[k] != g[k] {
                    diffs.push(format!("member {} stat {k}: Barrel {} game {}", who.get("id").num(), b[k], g[k]));
                }
            }
        }
    }
    assert!(checked > 0);
    assert!(diffs.is_empty(), "{diffs:#?}");
}
