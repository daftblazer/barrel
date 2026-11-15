//! A field encounter fought out without the battle screen: the encounter group's monsters drawn
//! as the game draws them (`battle::encounter_monsters`), against party members at given levels,
//! by [`crate::skirmish`] (each member standing in for the player). The step between touching an
//! enemy in the field and the played battle screen.
//!
//! Not yet the game's: the party members' stats their base stats at their level plus a new
//! game's starting kit (not the save's own equipment), HP and MP full, the battle items a session carries ([`Tables::carry`]); everything [`crate::skirmish`] leaves out.

use crate::data::battle_data::{self as bd, ActInfo, EncounterGroup, ItemEffect, LevelStats, Monster, PartySkill, Skill};
use barrel_formats::pe::Pe;

use crate::fight::Event;
pub use crate::skirmish::Command;
use barrel_engine::rng::Rng;

/// The party members' names by character number.
pub const MEMBER_NAMES: [&str; 11] = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
/// The events a battle may take before it is called off.
const MAX_EVENTS: usize = 1000;
/// A monster's size when `Chara.xml` gives none.
const MONSTER_SIZE: f32 = 100.0;

/// The tables a battle reads, loaded once.
pub struct Tables {
    monsters: Vec<Monster>,
    /// The drop pools (`battle_data::drop_pools`), for drop slots naming items 560 and up.
    pools: Vec<Vec<bd::PoolEntry>>,
    skills: Vec<Skill>,
    lists: Vec<Vec<PartySkill>>,
    growth: Vec<Vec<LevelStats>>,
    groups: Vec<EncounterGroup>,
    acts: Vec<ActInfo>,
    kinds: Vec<String>,
    effects: Vec<ItemEffect>,
    radii: Vec<f32>,
    /// What each member's starting kit adds (`party_defaults`' equipment through
    /// `equipment::item_stat`): HP, MP, attack, defence, speed, the four resistances.
    kit: Vec<[i32; 9]>,
    /// `itemData.xml`'s equipment values, for [`Tables::equip`].
    items: Vec<crate::equipment::ItemRow>,
    /// The experience table (`battle_data::exp_table`).
    pub exp: Vec<(i32, i32)>,
    /// The battle item table (`battle_data::item_battles`: target, area, wait).
    item_battles: Vec<bd::ItemBattle>,
    /// `itemData.xml`'s item names.
    item_names: Vec<String>,
    /// `item_potential.xml`'s traits ([`Tables::with_traits`]); none: carried items roll no bonus.
    traits: Vec<bd::Trait>,
    /// The battle items each member carries ([`Tables::carry`]), and for each the index of
    /// the session item it was made from.
    held: Vec<Vec<(usize, crate::skirmish::Held)>>,
}

impl Tables {
    /// What a battle the party won gives (`battle::rewards`, no bonuses): experience and money;
    /// nothing for a battle lost or called off.
    pub fn spoils(&self, o: &Outcome) -> (i32, i32) {
        if o.winner != Some(0) {
            return (0, 0);
        }
        let defeated: Vec<i32> = o.monsters.iter().map(|&m| m as i32).collect();
        crate::battle::rewards(&self.monsters, &defeated, 0, 0)
    }

    /// Member `member` fights in `items` (item numbers, -1 for none) instead of its new-game
    /// kit: a session's equipment. Made as fresh items too (quality 50, no traits or effects:
    /// a held item's own quality and traits are not carried yet).
    pub fn equip(&mut self, member: usize, items: &[i32]) {
        if self.items.is_empty() {
            return;
        }
        if self.kit.len() <= member {
            self.kit.resize(member + 1, [0; 9]);
        }
        self.kit[member] = kit_stats(&self.items, items);
    }

    /// Reads `item_potential.xml` (`potential_xml`), for carried items' traits.
    pub fn with_traits(mut self, potential_xml: &barrel_formats::table::Element) -> Tables {
        let tags: Vec<&str> = self.effects.iter().map(|r| r.tag.as_str()).collect();
        self.traits = bd::traits(potential_xml, &tags);
        self
    }

    /// Member `member` carries `items` (a session's battle items): each as its battle action
    /// (`battle::item_action`, target, area and wait from the battle item table), power from its
    /// quality (`battle::item_power`), its uses left and its traits. An item with no battle
    /// record is left out.
    pub fn carry(&mut self, member: usize, items: &[crate::session::BattleItem]) {
        if self.held.len() <= member {
            self.held.resize(member + 1, Vec::new());
        }
        self.held[member] = items
            .iter()
            .enumerate()
            .filter_map(|(k, u)| {
                let row = self.item_battles.iter().find(|b| b.item == u.item)?;
                let name = self.item_names.get(u.item as usize).cloned().unwrap_or_default();
                let action = crate::battle::item_action(&name, row.target, row.area, row.wait, &u.effects, &self.effects);
                let traits = u.traits.iter().filter_map(|&t| self.traits.get(t).cloned()).collect();
                Some((k, crate::skirmish::Held { action, power: crate::battle::item_power(u.quality), count: u.uses.max(0) as u32, traits }))
            })
            .collect();
    }

    /// The uses left after a battle of member `member`'s carried items (`left`: an outcome's
    /// [`Outcome::items_left`] entry for it), as (index among the session's items, uses).
    pub fn uses_left(&self, member: usize, left: &[u32]) -> Vec<(usize, i32)> {
        self.held.get(member).map(|h| h.iter().zip(left).map(|((k, _), &n)| (*k, n as i32)).collect()).unwrap_or_default()
    }

    /// From the English executable, `item_effect.xml`'s table (`effects_xml`) and `Chara.xml`'s
    /// (`chara`, for the fighters' sizes).
    ///
    /// With `itemData.xml` (`items`), each member fights in the kit a new game equips it with
    /// (`docs/tables.md`, "A new game's party"); without it, in its base stats alone.
    pub fn load(pe: &Pe, effects_xml: &barrel_formats::table::Element, chara: Option<&barrel_formats::table::Element>, items: Option<&barrel_formats::table::Element>) -> Result<Tables, String> {
        let (act_names, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
        let rows: Vec<crate::equipment::ItemRow> = items.map(|i| i.children.iter().map(crate::equipment::ItemRow::from_row).collect()).unwrap_or_default();
        let kit = match bd::party_defaults(pe, bd::PARTY_DEFAULTS_EN) {
            Some(defaults) if !rows.is_empty() => defaults.iter().map(|d| kit_stats(&rows, &d.equipment)).collect(),
            _ => Vec::new(),
        };
        Ok(Tables {
            monsters: bd::monsters(pe, bd::MONSTERS_EN).ok_or("no monster table")?,
            pools: bd::drop_pools(pe, bd::DROP_POOLS_EN).unwrap_or_default(),
            skills: bd::skills(pe, bd::SKILLS_EN).ok_or("no skill table")?,
            lists: bd::party_skills(pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists")?,
            growth: bd::party_growth(pe, bd::PARTY_GROWTH_EN).ok_or("no party growth table")?,
            groups: bd::encounter_groups(pe, bd::ENCOUNTER_GROUPS_EN),
            acts: bd::acts(pe, bd::ACTS_EN),
            kinds: bd::trait_kinds(pe).into_iter().map(str::to_string).collect(),
            effects: bd::item_effects(effects_xml, &act_names),
            radii: chara.map(bd::chara_radii).unwrap_or_default(),
            kit,
            items: rows,
            exp: bd::exp_table(pe, bd::EXP_TABLE_EN).unwrap_or_default(),
            item_battles: bd::item_battles(pe, bd::ITEM_BATTLE_EN, bd::ITEM_BATTLE_END_EN),
            item_names: items.map(|i| i.children.iter().map(|r| r.attr("ItemName").unwrap_or("?").to_string()).collect()).unwrap_or_default(),
            traits: Vec::new(),
            held: Vec::new(),
        })
    }
}

/// What a set of freshly made items (`0x14029ce00`: quality 50, no effects; their default traits
/// from a per-item table are not read) adds to stats 0 to 8.
fn kit_stats(rows: &[crate::equipment::ItemRow], items: &[i32]) -> [i32; 9] {
    let t = &crate::equipment::Tables { items: rows, effects: &[], traits: &[], acts: &[] };
    let mut out = [0; 9];
    for &item in items.iter().filter(|&&i| i >= 0) {
        let unit = crate::data::save::ItemUnit { serial: 1, item: item as i16, quality: 50.0, traits: [-1; 3], word_e: 0, effects: [-1; 4], equip_cost: 0, word_1a: 0, counts: [0; 4] };
        for (k, o) in out.iter_mut().enumerate() {
            *o += crate::equipment::item_stat(t, &unit, k, 0, &mut |lo, _| lo);
        }
    }
    out
}

/// How a battle went: the fighters' names (the party first), what happened and the winning
/// side (0 the party; none when it was called off).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub names: Vec<String>,
    /// Each fighter's HP and MP as the battle starts (all full): HP, max HP, MP, max MP.
    pub start: Vec<[i32; 4]>,
    pub events: Vec<Event>,
    pub winner: Option<u8>,
    /// The monsters fought, by monster number.
    pub monsters: Vec<usize>,
    /// What the defeated monsters dropped (a won battle only): item, count, quality.
    pub drops: Vec<(i32, i32, f32)>,
    /// The uses left of each party member's items (party order, their battle items' order).
    pub items_left: Vec<Vec<u32>>,
}

impl Outcome {
    /// The party ran from the battle (no winner, no defeat).
    pub fn escaped(&self) -> bool {
        self.events.iter().any(|e| matches!(e, Event::Ran { escaped: true, .. }))
    }

    /// What happened, a line per event and one for the end.
    pub fn lines(&self) -> Vec<String> {
        let name = |k: &usize| self.names.get(*k).map_or("?", String::as_str);
        let mut out: Vec<String> = self
            .events
            .iter()
            .map(|e| match e {
                Event::Felled { who, skill, target } => format!("{} uses {skill} on {}: felled", name(who), name(target)),
                Event::Ran { who, chance, escaped } => format!("{} runs ({chance}%): {}", name(who), if *escaped { "got away" } else { "could not escape" }),
                Event::Moved { who, sector } => format!("{} moves to sector {sector}", name(who)),
                Event::Rate { rate, support, .. } => format!("damage rate {}%, support {support}", 100 + rate),
                Event::Poisoned { who, damage, hp_left } => format!("{} takes {damage} poison damage ({hp_left} HP left)", name(who)),
                Event::Acted { who, skill, target, damage, hp_left, .. } => format!("{} uses {skill} on {}: {damage} damage ({hp_left} HP left)", name(who), name(target)),
                Event::Healed { who, skill, target, amount, hp_left, .. } => format!("{} uses {skill} on {}: heals {amount} ({hp_left} HP)", name(who), name(target)),
            })
            .collect();
        out.push(match self.winner {
            Some(0) => "the party wins".into(),
            Some(_) => "the monsters win".into(),
            None if self.escaped() => "the party ran away".to_string(),
            None => format!("no winner after {} events", self.events.len()),
        });
        out
    }
}

/// Encounter group `group`'s kind (its word 2: `battle::music`), if there is such a group.
pub fn group_kind(t: &Tables, group: i32) -> Option<i32> {
    t.groups.iter().find(|g| g.id == group).map(|g| g.kind)
}

/// Encounter group `group`'s monsters (by its id) drawn as the game draws them, each with its
/// size (`Chara.xml` radius).
pub fn draw(t: &Tables, group: i32, rng: &mut Rng) -> Result<Vec<(usize, f32)>, String> {
    let g = t.groups.iter().find(|g| g.id == group).ok_or_else(|| format!("no encounter group {group}"))?;
    Ok(crate::battle::encounter_monsters(g, rng).into_iter().map(|m| (m as usize, bd::monster_size(&t.radii, m as usize).unwrap_or(MONSTER_SIZE))).collect())
}

/// Encounter group `group` (its id, as a field enemy names it) against `party`, each a
/// (character number, level).
pub fn fight(t: &Tables, party: &[(usize, i32)], group: i32, rng: &mut Rng) -> Result<Outcome, String> {
    let drawn: Vec<usize> = draw(t, group, rng)?.into_iter().map(|(m, _)| m).collect();
    fight_drawn(t, party, &drawn, rng)
}

/// Monsters already drawn (`draw`) against `party`: the battle scatters them with `rng` first
/// (`battle::scatter`), so a copy of `rng` taken now places them where the battle does.
pub fn fight_drawn(t: &Tables, party: &[(usize, i32)], drawn: &[usize], rng: &mut Rng) -> Result<Outcome, String> {
    fight_with(t, party, drawn, rng, None).map(|(o, _)| o)
}

fn fight_with(t: &Tables, party: &[(usize, i32)], drawn: &[usize], rng: &mut Rng, commands: Option<&[Command]>) -> Result<(Outcome, Option<Turn>), String> {
    let mut names = Vec::new();
    let mut start = Vec::new();
    let mut members = Vec::new();
    for &(c, level) in party {
        let stats = t.growth.get(c).and_then(|l| l.get((level as usize).wrapping_sub(1))).ok_or_else(|| format!("character {c}: no growth row for level {level}"))?;
        let mut block = crate::duel::party_block(level, stats);
        let kit = t.kit.get(c).copied().unwrap_or_default();
        block.hp += kit[0];
        block.attack += kit[2];
        block.defense += kit[3];
        block.speed += kit[4];
        for (r, k) in block.resist.iter_mut().zip(&kit[5..]) {
            *r += k;
        }
        let mp = stats.mp + kit[1];
        let list = t.lists.get(c).ok_or_else(|| format!("character {c}: no skill list"))?;
        let mut member = crate::skirmish::Member::new(block.clone(), mp, list, &t.skills).ok_or("no party attack")?;
        member.size = bd::party_size(&t.radii, c).unwrap_or(0.0);
        if let Some((_, cost)) = member.support.as_mut() {
            *cost = crate::battle::support_attack_cost(Some(c));
        }
        member.items = t.held.get(c).map(|h| h.iter().map(|(_, h)| h.clone()).collect()).unwrap_or_default();
        start.push([block.hp, block.hp, mp, mp]);
        members.push(member);
        names.push(MEMBER_NAMES.get(c).map_or_else(|| format!("member {c}"), |n| n.to_string()));
    }
    let normal = t.skills.get(1).ok_or("no skill 1")?;
    let mut foes = Vec::new();
    for &k in drawn {
        let m = t.monsters.get(k).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("monster {k}: none"))?;
        foes.push(crate::duel::Fighter { block: m.blocks[0].clone(), normal, skills: Some(&t.skills[..]), death_immune: m.death_immune, chara: bd::MONSTER_CHARA_FIRST as i32 + k as i32 });
        names.push(m.name.clone());
        start.push([m.blocks[0].hp, m.blocks[0].hp, 0, 0]);
    }
    let sizes: Vec<f32> = drawn.iter().map(|&k| bd::monster_size(&t.radii, k).unwrap_or(MONSTER_SIZE)).collect();
    let kinds: Vec<&str> = t.kinds.iter().map(String::as_str).collect();
    let rules = crate::skirmish::ItemRules { acts: &t.acts, kinds: &kinds, effects: &t.effects };
    let (events, winner, turn, items_left) = match commands {
        None => {
            let (events, winner, left) = crate::skirmish::fought(&members, &foes, &sizes, rules, rng, MAX_EVENTS);
            (events, winner, None, left)
        }
        Some(commands) => run_commanded(&members, &foes, &sizes, rules, rng, commands),
    };
    // Won: each defeated monster's drop slots, in order, on the battle's generator
    // (`0x140192f10` → `battle::monster_drops`, no bonuses yet); a slot naming a drop pool draws
    // from it (`encounter::pick_pool`, every entry eligible) and the item comes at a quality
    // drawn in the entry's range (the draw's order not read: not yet the game's). Any other
    // item is made as it is, at quality 50 (`0x14029ce00`).
    let mut drops = Vec::new();
    if winner == Some(0) && turn.is_none() {
        for &m in drawn {
            for (item, count) in t.monsters.get(m).map(|m| crate::battle::monster_drops(m, 0, 0, rng)).unwrap_or_default() {
                let pool = usize::try_from(item - bd::DROP_POOL_FIRST).ok().and_then(|k| t.pools.get(k));
                match pool {
                    Some(pool) => {
                        if let Some(e) = crate::encounter::pick_pool(pool, |_| true, rng) {
                            let quality = rng.range(e.quality.0, e.quality.1);
                            drops.push((e.item, count, quality as f32));
                        }
                    }
                    None => drops.push((item, count, 50.0)),
                }
            }
        }
    }
    Ok((Outcome { names, start, events, winner, monsters: drawn.to_vec(), drops, items_left }, turn))
}

/// A member's turn waiting for a command: who, the skills it may use (name, MP cost, whether it
/// can pay) and the monsters standing (fighter numbers).
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub who: usize,
    pub skills: Vec<(String, i32, bool)>,
    pub foes: Vec<usize>,
    /// The turn order from `who` on: the fighters standing, by the battle's timeline.
    pub order: Vec<usize>,
    /// The timeline's entries in turn order (`Skirmish::slots`): the turn bar's slots.
    pub slots: Vec<Option<usize>>,
    /// The turn bar's next-turn mark for each command: the attack, the skills, Wait
    /// (`Skirmish::next_places`).
    pub next_places: Vec<usize>,
    /// The member's sector round the battle (0 to 2; its place +0x394 less 3).
    pub sector: i32,
    /// Its items: (name, how many are left, thrown at the monsters).
    pub items: Vec<(String, u32, bool)>,
}

/// Monsters already drawn against `party` with the player's `commands`, one per member turn in
/// order, from the battle's generator `rng` (taken by value: the same generator and commands
/// always fight the same battle, so a battle grows a command at a time). Returns the battle so
/// far and the member turn waiting for the next command (none once a side has won).
pub fn fight_commanded(t: &Tables, party: &[(usize, i32)], drawn: &[usize], mut rng: Rng, commands: &[Command]) -> Result<(Outcome, Option<Turn>), String> {
    fight_with(t, party, drawn, &mut rng, Some(commands))
}

fn run_commanded(members: &[crate::skirmish::Member], foes: &[crate::duel::Fighter], sizes: &[f32], rules: crate::skirmish::ItemRules, rng: &mut Rng, commands: &[Command]) -> (Vec<Event>, Option<u8>, Option<Turn>, Vec<Vec<u32>>) {
    let mut s = crate::skirmish::Skirmish::new(members, foes, sizes, rules, rng);
    let mut events = Vec::new();
    let mut given = commands.iter().peekable();
    while events.len() < MAX_EVENTS && s.winner().is_none() && !s.battle.escaped {
        let (who, start) = s.begin_turn();
        events.extend(start);
        if !s.can_act(who) {
            continue;
        }
        if !s.is_member(who) {
            events.extend(s.auto(who, rng));
            continue;
        }
        // Supports with no action before them stand for nothing.
        while given.next_if(|c| matches!(c, Command::Support { .. })).is_some() {}
        match given.next() {
            Some(&c) => {
                let from = events.len();
                events.extend(s.command(who, c, rng));
                // The supports given while this action played, on its target.
                let target = events[from..].iter().find_map(|e| match *e {
                    Event::Acted { who: w, target, .. } if w == who => Some(target),
                    _ => None,
                });
                while let Some(&Command::Support { by }) = given.next_if(|c| matches!(c, Command::Support { .. })) {
                    if let Some(t) = target {
                        events.extend(s.support(by, t, rng));
                    }
                }
            }
            None => {
                let skills = s.skills(who).into_iter().map(|(k, cost, can)| (k.name.clone(), cost, can)).collect();
                return (events, None, Some(Turn { who, skills, foes: s.foes_standing(), order: s.order(), slots: s.slots(), next_places: s.next_places(who), sector: s.battle.fighters[who].slot - 3, items: s.items(who) }), s.items_left());
            }
        }
    }
    let winner = s.winner();
    (events, winner, None, s.items_left())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_the_fighters() {
        let o = Outcome {
            names: vec!["Escha".into(), "Puni".into()],
            start: vec![[10; 4], [10; 4]],
            events: vec![Event::Acted { who: 0, skill: "Attack".into(), target: 1, damage: 12, hp_left: 0, mp_left: 0 }],
            winner: Some(0),
            monsters: vec![0],
            drops: Vec::new(),
            items_left: Vec::new(),
        };
        assert_eq!(o.lines(), vec!["Escha uses Attack on Puni: 12 damage (0 HP left)".to_string(), "the party wins".into()]);
    }

    #[test]
    #[ignore = "needs the game (BARREL_ROOT)"]
    fn the_starting_kit_wins_against_an_ore_rat() {
        let Ok(root) = std::env::var("BARREL_ROOT") else { return };
        let root = std::path::Path::new(&root);
        let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).unwrap();
        let pe = Pe::parse(&exe).unwrap();
        let read = |p: &str| barrel_formats::table::parse(&std::fs::read(root.join(p)).unwrap()).unwrap();
        let (effects, chara, items) = (read("Saves_EN/item/item_effect.xml"), read("Saves/chara/Chara.xml"), read("Saves_EN/item/itemData.xml"));
        let kit = Tables::load(&pe, &effects, Some(&chara), Some(&items)).unwrap();
        let bare = Tables::load(&pe, &effects, Some(&chara), None).unwrap();
        assert_eq!((kit.kit[0][2], kit.kit[0][3], kit.kit[1][2], kit.kit[1][3]), (5, 5, 8, 9));
        let rat = kit.monsters.iter().position(|m| m.name == "Ore Rat").unwrap();
        let wins = |t: &Tables| (0..200u32).filter(|&s| {
            let mut rng = Rng::from_state([s, 0x2468_ace0, 0x0f1e_2d3c, 0x4b5a_6978]);
            fight_drawn(t, &[(0, 1), (1, 1)], &[rat], &mut rng).unwrap().winner == Some(0)
        }).count();
        let (with, without) = (wins(&kit), wins(&bare));
        // A session's own equipment replaces the kit: none at all fights as the bare stats.
        let mut stripped = Tables::load(&pe, &effects, Some(&chara), Some(&items)).unwrap();
        stripped.equip(0, &[-1; 4]);
        stripped.equip(1, &[-1; 4]);
        assert_eq!(wins(&stripped), without);
        eprintln!("Escha and Logy at level 1 against an Ore Rat: {with} of 200 won in their kit, {without} without");
        assert!(with > without);
        // A won battle's spoils: the Ore Rat's experience and money.
        let mut rng = Rng::from_state([1, 0x2468_ace0, 0x0f1e_2d3c, 0x4b5a_6978]);
        let won = fight_drawn(&kit, &[(0, 1), (1, 1)], &[rat], &mut rng).unwrap();
        let m = &kit.monsters[rat];
        assert_eq!(kit.spoils(&won), (m.exp, m.money));
        eprintln!("an Ore Rat gives {} experience and {} money", m.exp, m.money);
    }

    #[test]
    #[ignore = "needs the game (BARREL_ROOT)"]
    fn a_carried_item_is_thrown_by_command() {
        let Ok(root) = std::env::var("BARREL_ROOT") else { return };
        let root = std::path::Path::new(&root);
        let exe = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe")).unwrap();
        let pe = Pe::parse(&exe).unwrap();
        let read = |p: &str| barrel_formats::table::parse(&std::fs::read(root.join(p)).unwrap()).unwrap();
        let (effects, chara, items) = (read("Saves_EN/item/item_effect.xml"), read("Saves/chara/Chara.xml"), read("Saves_EN/item/itemData.xml"));
        let mut t = Tables::load(&pe, &effects, Some(&chara), Some(&items)).unwrap().with_traits(&read("Saves_EN/item/item_potential.xml"));
        // The first battle item thrown at the other side, with a damage effect row.
        let damage = t.effects.iter().position(|e| e.effects.iter().any(|x| x.act == 0)).unwrap() as i16;
        let row = t.item_battles.iter().find(|b| b.target == 2).unwrap();
        let name = t.item_names[row.item as usize].clone();
        t.carry(0, &[crate::session::BattleItem { item: row.item, quality: 50.0, uses: 2, effects: vec![damage], traits: Vec::new() }]);
        let rat = t.monsters.iter().position(|m| m.name == "Ore Rat").unwrap();
        let rng = Rng::from_state([1, 0x2468_ace0, 0x0f1e_2d3c, 0x4b5a_6978]);
        let mut commands = Vec::new();
        let mut thrown = false;
        for _ in 0..8 {
            let (o, turn) = fight_commanded(&t, &[(0, 1), (1, 1)], &[rat], rng.clone(), &commands).unwrap();
            if thrown {
                assert!(o.events.iter().any(|e| matches!(e, Event::Acted { who: 0, skill, .. } if *skill == name)), "{:?}", o.lines());
                // One of its two uses spent, back on the session's item 0.
                assert_eq!(t.uses_left(0, &o.items_left[0]), vec![(0, 1)]);
                return;
            }
            let Some(turn) = turn else { panic!("the battle ended first: {:?}", o.lines()) };
            if turn.who == 0 {
                assert_eq!(turn.items, vec![(name.clone(), 2, true)]);
                commands.push(Command::Item { item: 0, target: turn.foes[0] });
                thrown = true;
            } else {
                commands.push(Command::Wait);
            }
        }
        panic!("no turn for member 0");
    }
}
