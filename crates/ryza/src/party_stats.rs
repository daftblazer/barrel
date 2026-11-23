//! The numbers a party member shows on the Equipment and Status pages: HP, attack, defence, speed,
//! equip quality and dexterity, from the game's growth table (`saves/party/party_param.xml`), the worn
//! items' rows of `saves/item/itemdata.xml` and their qualities from the save. Docs: docs/tables.md,
//! "Ryza DX: a party member's numbers".

use barrel_formats::table;

use crate::save::Worn;

/// What a page shows of one member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub equip_quality: i32,
    pub dexterity: i32,
}

/// One item's bonuses (`itemData`'s `hp`, `atk`, `def`, `spd`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
}

/// The growth table: a row a level (from 1), in each row 12 columns (the characters) of 6 numbers
/// (`param000`, `param100` ... `param500`: HP, one not shown, attack, defence, speed, dexterity).
#[derive(Clone, Debug, Default)]
pub struct Tables {
    growth: Vec<Vec<[f32; 6]>>,
    items: Vec<Bonus>,
    /// Each item's picture number (`imgNo`).
    images: Vec<usize>,
}

/// A quality of this many points is one point of equip quality.
const QUALITY_PER_POINT: i32 = 10;

impl Tables {
    pub fn read(party_param: &[u8], item_data: &[u8]) -> Result<Tables, String> {
        let growth = table::parse(party_param).map_err(|e| format!("party_param: {e}"))?;
        let growth = growth
            .children
            .iter()
            .map(|row| {
                (0..12)
                    .map(|c| {
                        let mut v = [0.0; 6];
                        for (h, v) in v.iter_mut().enumerate() {
                            *v = row.attr(&format!("param{h}{c:02}")).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                        }
                        v
                    })
                    .collect()
            })
            .collect();
        let items_root = table::parse(item_data).map_err(|e| format!("itemdata: {e}"))?;
        let items = items_root
            .children
            .iter()
            .map(|e| {
                let n = |k: &str| e.attr(k).and_then(|t| t.parse().ok()).unwrap_or(0);
                Bonus { hp: n("hp"), attack: n("atk"), defense: n("def"), speed: n("spd") }
            })
            .collect();
        let images = items_root.children.iter().map(|e| e.attr("imgNo").and_then(|t| t.parse().ok()).unwrap_or(0)).collect();
        Ok(Tables { growth, items, images })
    }

    /// An item's picture number in the item pictures' list.
    pub fn image(&self, item: u16) -> usize {
        self.images.get(usize::from(item)).copied().unwrap_or(0)
    }

    /// The numbers of character `id` at `level` wearing `worn`.
    ///
    /// The growth row is `level - 1`; the shown HP is its first number and the shown attack, defence
    /// and speed its third to fifth, each raised by the equip quality (the mean quality of the worn
    /// items over `QUALITY_PER_POINT`, rounded down) and by the worn items' own bonuses; dexterity is
    /// its sixth number. (STAND-IN: the +3 on attack, defence and speed in the capture is read as the
    /// equip quality, which is 3 there, but the level is 3 too; the second number is not shown;
    /// fractions in the table round down.)
    pub fn stats(&self, id: u8, level: u8, worn: &[Option<Worn>]) -> Stats {
        let g = self.growth.get(usize::from(level.max(1)) - 1).and_then(|r| r.get(usize::from(id))).copied().unwrap_or_default();
        let g = g.map(|v| v.floor() as i32);
        let on: Vec<&Worn> = worn.iter().flatten().collect();
        let quality = if on.is_empty() { 0 } else { on.iter().map(|w| i32::from(w.quality)).sum::<i32>() / (on.len() as i32 * QUALITY_PER_POINT) };
        let bonus = on.iter().map(|w| self.bonus(w.item)).fold(Bonus::default(), |a, b| Bonus { hp: a.hp + b.hp, attack: a.attack + b.attack, defense: a.defense + b.defense, speed: a.speed + b.speed });
        Stats { hp: g[0] + bonus.hp, attack: g[2] + quality + bonus.attack, defense: g[3] + quality + bonus.defense, speed: g[4] + quality + bonus.speed, equip_quality: quality, dexterity: g[5] }
    }

    /// The bonuses an item row gives.
    pub fn bonus(&self, item: u16) -> Bonus {
        self.items.get(usize::from(item)).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level 3's rows for Ryza, Lent and Tao as the game's table has them, and the item rows they wear.
    fn tables() -> Tables {
        let mut growth = vec![vec![[0.0; 6]; 12]; 3];
        growth[2][0] = [37.0, 11.0, 11.0, 8.0, 11.0, 5.0];
        growth[2][2] = [57.0, 8.0, 14.0, 14.0, 8.0, 2.0];
        growth[2][3] = [31.0, 3.0, 8.0, 6.0, 13.0, 3.0];
        // Rows 406 (Ryza's staff), 416 (Lent's weapon) and 456 (the first armour) of itemdata.xml.
        let mut items = vec![Bonus::default(); 457];
        items[406] = Bonus { hp: 0, attack: 6, defense: 0, speed: 2 };
        items[416] = Bonus { hp: 0, attack: 9, defense: 2, speed: 0 };
        items[456] = Bonus { hp: 0, attack: 0, defense: 4, speed: 0 };
        Tables { growth, items, images: Vec::new() }
    }

    fn worn(weapon: u16) -> [Option<Worn>; 4] {
        [Some(Worn { item: weapon, quality: 30 }), Some(Worn { item: 456, quality: 30 }), None, None]
    }

    #[test]
    fn ryza_at_level_3_with_the_staff_and_the_first_armour() {
        let s = tables().stats(0, 3, &worn(406));
        assert_eq!(s, Stats { hp: 37, attack: 20, defense: 15, speed: 16, equip_quality: 3, dexterity: 5 });
    }

    #[test]
    fn lent_at_level_3() {
        let s = tables().stats(2, 3, &worn(416));
        assert_eq!(s, Stats { hp: 57, attack: 26, defense: 23, speed: 11, equip_quality: 3, dexterity: 2 });
    }

    #[test]
    fn tao_has_31_hp() {
        assert_eq!(tables().stats(3, 3, &worn(406)).hp, 31);
    }

    #[test]
    fn a_member_with_nothing_worn_has_the_growth_row_alone() {
        let s = tables().stats(0, 3, &[None; 4]);
        assert_eq!((s.hp, s.attack, s.defense, s.speed, s.equip_quality), (37, 11, 8, 11, 0));
    }

    #[test]
    fn a_missing_row_gives_zeros() {
        assert_eq!(tables().stats(0, 50, &[None; 4]), Stats::default());
    }

    /// The real tables against the game's Lv 3 numbers (`BARREL_RYZA_INSTALL` names the install; not in the repository).
    #[test]
    fn the_real_tables_give_the_games_numbers() {
        let Some(root) = std::env::var_os("BARREL_RYZA_INSTALL") else { return };
        let install = crate::install::Install::open(std::path::Path::new(&root)).unwrap();
        let t = Tables::read(&install.read("\\saves\\party\\party_param.xml").unwrap(), &install.read("\\saves\\item\\itemdata.xml").unwrap()).unwrap();
        assert_eq!(t.stats(0, 3, &worn(406)), Stats { hp: 37, attack: 20, defense: 15, speed: 16, equip_quality: 3, dexterity: 5 });
        assert_eq!(t.stats(2, 3, &worn(416)).defense, 23);
        assert_eq!(t.stats(3, 3, &worn(421)).hp, 31);
        assert_eq!(t.bonus(406), Bonus { hp: 0, attack: 6, defense: 0, speed: 2 });
    }
}
