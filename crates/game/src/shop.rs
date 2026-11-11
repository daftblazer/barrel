//! Shops, from `Saves[_<LANG>]/shop/ShopData.xml` (`docs/tables.md`, "Column types"): what
//! each shop sells, how many, at what price and quality, and between which events.
//!
//! The restock rules (how `Refill`, `RefillCnt` and `DayAdd` are applied over the days) are
//! not read from the executable yet; this keeps the numbers as the table gives them.

use barrel_formats::table::Element;

/// One item a shop sells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ware {
    pub shop: String,
    pub shop_name: String,
    pub item: String,
    pub item_name: String,
    pub stock: i32,
    pub refill: i32,
    pub refill_count: i32,
    pub days: i32,
    pub cost: i32,
    pub quality: i32,
    /// The item effects (`ItemEff_00`...) and traits (`Pot_00`...) it is sold with.
    pub effects: Vec<String>,
    pub traits: Vec<String>,
    /// The event after which it is on sale, and the one after which it no longer is.
    pub from_event: Option<String>,
    pub until_event: Option<String>,
}

/// The wares of a `ShopData` table, in its order.
pub fn wares(table: &Element) -> Vec<Ware> {
    let num = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let text = |e: &Element, k: &str| e.attr(k).unwrap_or("").to_string();
    let opt = |e: &Element, k: &str| e.attr(k).filter(|v| !v.is_empty()).map(str::to_string);
    let list = |e: &Element, prefix: &str| {
        (0..16).filter_map(|i| opt(e, &format!("{prefix}{i:02}"))).collect::<Vec<_>>()
    };
    table
        .children
        .iter()
        .filter(|e| e.name == "ShopData")
        .map(|e| Ware {
            shop: text(e, "ShopTag"),
            shop_name: text(e, "ShopName"),
            item: text(e, "ItemTag"),
            item_name: text(e, "itemName"),
            stock: num(e, "Stock"),
            refill: num(e, "Refill"),
            refill_count: num(e, "RefillCnt"),
            days: num(e, "DayAdd"),
            cost: num(e, "Cost"),
            quality: num(e, "Quality"),
            effects: list(e, "ItemEff_"),
            traits: list(e, "Pot_"),
            from_event: opt(e, "EvStart"),
            until_event: opt(e, "EvEnd"),
        })
        .collect()
}

/// Whether a ware is on sale, given which events have been seen.
pub fn on_sale(w: &Ware, seen: impl Fn(&str) -> bool) -> bool {
    w.from_event.as_deref().is_none_or(&seen) && !w.until_event.as_deref().is_some_and(&seen)
}

/// The shop rotation's generator (the object at `0x140dfc880`, English Escha executable): two
/// words, stepped like the battle's Xorshift but on two: `t = x ^ (x << 11)`, `x = y`,
/// `y = y ^ (y >> 19) ^ (t >> 8) ^ t` with the shift by 19 arithmetic and by 8 logical; a
/// number is the new `y`'s low 31 bits. Seeded on each redraw by [`crate::calendar::shop_seed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopRng {
    pub state: [i32; 2],
}

impl ShopRng {
    pub fn new(state: [i32; 2]) -> ShopRng {
        ShopRng { state }
    }

    pub fn next(&mut self) -> i32 {
        let [x, y] = self.state;
        let t = x ^ (x << 11);
        let y2 = y ^ (y >> 19) ^ (((t as u32) >> 8) as i32) ^ t;
        self.state = [y, y2];
        y2 & 0x7fff_ffff
    }

    /// A number below `n` (`0x14033c210`).
    pub fn below(&mut self, n: i32) -> i32 {
        self.next() % n.max(1)
    }
}

/// Picks one of `chances` (each candidate's percentage, the float at its +0x38) the way the
/// rotation does (`0x14033b836`): in order, each draws a number below 100 and is taken when it
/// is under `trunc(chance + 0.5)`; when none is, the candidates are shuffled (for each place
/// k from 1, swapped with the one at `below(k + 1)`, the standard library's `random_shuffle`)
/// and the first is taken. Returns the candidate's index; `None` when there are none.
pub fn rotation_pick(chances: &[f32], rng: &mut ShopRng) -> Option<usize> {
    if chances.is_empty() {
        return None;
    }
    for (i, &c) in chances.iter().enumerate() {
        if rng.next() % 100 < (c + 0.5) as i32 {
            return Some(i);
        }
    }
    let mut order: Vec<usize> = (0..chances.len()).collect();
    for k in 1..order.len() {
        let j = rng.below(k as i32 + 1) as usize;
        order.swap(k, j);
    }
    Some(order[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_wares() {
        let t = barrel_formats::table::parse_str(
            r#"<Root><ShopData ShopName="Nio's Shop" ShopTag="SHOP_NIO" itemName="White Flower" ItemTag="ITEM_MAT_WHITE_FLOWER" Stock="5" Refill="5" RefillCnt="1" DayAdd="10" Cost="70" Quality="50" EvStart="EVENT_MQ03_010" ItemEff_00="ITEM_EFF_A"/></Root>"#,
        )
        .unwrap();
        let w = wares(&t);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].stock, w[0].refill_count, w[0].days, w[0].cost), (5, 1, 10, 70));
        assert_eq!(w[0].effects, ["ITEM_EFF_A"]);
        assert!(!on_sale(&w[0], |_| false));
        assert!(on_sale(&w[0], |e| e == "EVENT_MQ03_010"));
    }

    #[test]
    fn the_rotation_generator() {
        let mut rng = ShopRng::new([1, 2]);
        // t = 1 ^ 2048 = 2049; y = 2 ^ 0 ^ (2049 >> 8 = 8) ^ 2049 = 2059.
        assert_eq!(rng.next(), 2059);
        assert_eq!(rng.state, [2, 2059]);
        // A sure candidate is taken at once; none gives a shuffled pick.
        let mut rng = ShopRng::new(crate::calendar::shop_seed(90, 0));
        assert_eq!(rotation_pick(&[100.0, 0.0], &mut rng), Some(0));
        assert!(rotation_pick(&[0.0, 0.0, 0.0], &mut rng).is_some());
        assert_eq!(rotation_pick(&[], &mut rng), None);
    }
}
