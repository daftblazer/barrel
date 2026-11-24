//! The skills a character's Status page lists (its "Active" tab), from the game's tables: `saves/party/party_skill.xml` (a
//! section a character, a row a place in it) and `saves/skill/skill_data.xml` (a row a skill, in the same order: the first
//! row of a character's section is the row whose macro is `skill/SKILL_PARTY_<TAG>_0`). Docs: docs/tables.md, "Ryza DX: a
//! character's skills".

use barrel_formats::table;

/// `STR_ACTION_NAME_n` is the string numbered `ACTION_NAME + n`, `STR_ACTION_COMMENT_n` `ACTION_COMMENT + n`.
pub const ACTION_NAME: u32 = 19_922_945;
pub const ACTION_COMMENT: u32 = 19_988_481;

/// What the page shows of one skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skill {
    /// The skill's name and description (string numbers).
    pub name: u32,
    pub comment: u32,
    /// Its AP cost (0: the normal attack, which shows none).
    pub cost: i32,
    /// Its picture in the battle icons (`icon`).
    pub icon: usize,
    /// Its attribute icon (`base_att`: 4 a physical attack, 5 a magical one; the elements' pictures are 0 to 3, not tried).
    pub attribute: usize,
}

/// A skill's row of `skill_data.xml` and the party table's own columns for it.
#[derive(Clone, Debug)]
struct Place {
    /// The skill's row in `skill_data.xml`.
    data: usize,
    /// Level range it is learnt over (`lv`, `endLv`: -1 for none) and the tactics levels it stands for (`tactics_s`, `tactics_e`).
    levels: (i32, i32),
    tactics: Option<(i32, i32)>,
    /// A skill the story gives (`startEv`) or an empty place (`SKILL_NONE`).
    given: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Skills {
    data: Vec<Skill>,
    /// Each character's section, in the party tables' order.
    sections: Vec<Vec<Place>>,
}

impl Skills {
    pub fn read(party_skill: &[u8], skill_data: &[u8]) -> Result<Skills, String> {
        let number = |e: &table::Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<i32>().ok());
        let tail = |id: Option<&str>| id.and_then(|s| s.rsplit('_').next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        let root = table::parse(skill_data).map_err(|e| format!("skill_data: {e}"))?;
        let mut macros = Vec::new();
        let data = root
            .children
            .iter()
            .map(|e| {
                macros.push(e.attr("macro").unwrap_or("").to_string());
                Skill { name: ACTION_NAME + tail(e.attr("name_id")), comment: ACTION_COMMENT + tail(e.attr("comment")), cost: number(e, "cost").unwrap_or(0), icon: number(e, "icon").unwrap_or(0).max(0) as usize, attribute: attribute(e.attr("base_att")) }
            })
            .collect();
        let rows = table::parse(party_skill).map_err(|e| format!("party_skill: {e}"))?;
        let mut sections: Vec<Vec<Place>> = Vec::new();
        let mut base = 0;
        let mut at = 0;
        for e in &rows.children {
            if let Some(tag) = e.attr("charaTag") {
                let first = format!("skill/SKILL_PARTY_{}_0.ebd", tag.trim().trim_start_matches("CHARA_"));
                base = macros.iter().position(|m| *m == first).ok_or(format!("party_skill: no skill row for {tag}"))?;
                at = 0;
                sections.push(Vec::new());
            }
            let Some(section) = sections.last_mut() else { continue };
            let tactics = number(e, "tactics_s").zip(number(e, "tactics_e"));
            section.push(Place { data: base + at, levels: (number(e, "lv").unwrap_or(-1), number(e, "endLv").unwrap_or(-1)), tactics, given: e.attr("skillTag") == Some("SKILL_NONE") || e.attr("startEv").is_some() });
            at += 1;
        }
        Ok(Skills { data, sections })
    }

    /// The skills of character `id` (the party tables' number) at `level` and `tactics` level: the normal attack, then the
    /// active skills learnt by that level and standing for that tactics level (not the story's, not the empty places).
    pub fn active(&self, id: usize, level: u8, tactics: u8) -> Vec<Skill> {
        let (level, tactics) = (i32::from(level), i32::from(tactics));
        let Some(section) = self.sections.get(id) else { return Vec::new() };
        section
            .iter()
            .enumerate()
            .filter(|(n, p)| {
                !p.given
                    && (*n == 0
                        || p.tactics.is_some_and(|(s, e)| (s..=e).contains(&tactics)) && p.levels.0 > 0 && p.levels.0 <= level && (p.levels.1 < 0 || level <= p.levels.1))
            })
            .filter_map(|(_, p)| self.data.get(p.data).copied())
            .collect()
    }
}

/// The attribute icon of a skill's `base_att` (the pictures of `gen_a21_icon01_zokusei`).
fn attribute(base: Option<&str>) -> usize {
    match base {
        Some("ATT_A_PHY") => 4,
        Some("ATT_A_MAGIC") => 5,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARTY: &str = r#"<Root>
<party_skill charaTag="CHARA_REISA" skillTag="SKILL_REISA_NORMALATTACK_1" special="-1" lv="1" endLv="-1"/>
<party_skill skillTag="SKILL_NONE" lv="-1" endLv="-1"/>
<party_skill skillTag="SKILL_NONE" lv="-1" endLv="-1"/>
<party_skill skillTag="SKILL_REISA_ACTIVESKILL_1_1" lv="1" endLv="60" tactics_s="1" tactics_e="1"/>
<party_skill skillTag="SKILL_REISA_ACTIVESKILL_1_2" lv="1" endLv="60" tactics_s="2" tactics_e="3"/>
<party_skill skillTag="SKILL_REISA_ACTIVESKILL_2_1" lv="10" endLv="-1" tactics_s="1" tactics_e="2"/>
<party_skill skillTag="SKILL_REISA_ORDERSKILL_1" special="-1" lv="1" endLv="-1" startEv="EVENT_X"/>
</Root>"#;

    fn data() -> String {
        let row = |n: u32, cost: u32, m: &str| format!(r#"<skill_data name_id="STR_ACTION_NAME_{n:03}" icon="11" cost="{cost}" comment="STR_ACTION_COMMENT_{n:03}" macro="skill/{m}.ebd"/>"#);
        let rows = [row(0, 0, "SKILL_PARTY_REISA_0"), row(1, 0, "SKILL_PARTY_REISA_1"), row(2, 0, "SKILL_PARTY_REISA_2"), row(3, 3, "SKILL_PARTY_REISA_5"), row(4, 3, "SKILL_PARTY_REISA_6"), row(5, 5, "SKILL_PARTY_REISA_10"), row(6, 0, "X")];
        format!("<Root>{}</Root>", rows.concat())
    }

    #[test]
    fn a_level_three_character_at_the_first_tactics_level_has_her_attack_and_the_first_skill() {
        let skills = Skills::read(PARTY.as_bytes(), data().as_bytes()).unwrap();
        let list = skills.active(0, 3, 1);
        // (`field-menu-status`: "Ryza Special" with no AP, then "Summoning Star" at 3 AP)
        assert_eq!(list.iter().map(|s| (s.name - ACTION_NAME, s.cost)).collect::<Vec<_>>(), [(0, 0), (3, 3)]);
        assert_eq!(list[1].comment, ACTION_COMMENT + 3);
    }

    #[test]
    fn a_higher_tactics_level_swaps_the_skill_for_its_stronger_form_and_the_level_gates_the_next() {
        let skills = Skills::read(PARTY.as_bytes(), data().as_bytes()).unwrap();
        assert_eq!(skills.active(0, 3, 2).iter().map(|s| s.name - ACTION_NAME).collect::<Vec<_>>(), [0, 4]);
        assert_eq!(skills.active(0, 10, 2).iter().map(|s| s.name - ACTION_NAME).collect::<Vec<_>>(), [0, 4, 5]);
        assert!(skills.active(5, 1, 1).is_empty());
    }
}
