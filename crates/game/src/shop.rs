//! Shops, from `Saves[_<LANG>]/shop/ShopData.xml` (`docs/tables.md`, "Column types"): what
//! each shop sells, how many, at what price and quality, and between which events.
//!
//! The restock rules are [`pass_days`].

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

/// A ware's stock as the game keeps it (12 bytes per ware, 50 wares for each of 4 shops, at
/// `0x141078e14`, 0x25c bytes a shop; saved with the game).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stock {
    pub count: i32,
    /// Days counted towards the next refill.
    pub days: i32,
    /// For the shop's menu: -1 nothing to say, 1 sold out with a refill within 30 days, 2 sold
    /// out with one further off, 3 never refilled, 0 and 4 set elsewhere.
    pub state: i32,
}

impl Stock {
    /// A new game's stock: the table's `Stock`.
    pub fn new(w: &Ware) -> Stock {
        Stock { count: w.stock, days: 0, state: -1 }
    }
}

/// Days pass for a ware (`0x14035c560 (days)`, after every day advance; wares whose event window
/// is not open are skipped). With no `RefillCnt` it is never refilled; a ware at or above its
/// `Refill` counts no days; otherwise the days count up, and on reaching `DayAdd` they start
/// again and `RefillCnt` more come in, up to `Refill`. Returns whether it was refilled.
pub fn pass_days(s: &mut Stock, w: &Ware, days: i32) -> bool {
    if w.refill_count == 0 {
        if s.state == -1 || s.state == 4 {
            s.state = 3;
        }
        return false;
    }
    if s.count >= w.refill {
        if s.state == 4 {
            s.state = 0;
        }
        return false;
    }
    s.days = s.days.wrapping_add(days);
    let left = w.days - s.days;
    if left > 30 {
        if s.count <= 0 && s.state != 2 && s.state != 4 {
            s.state = 2;
        }
        return false;
    }
    if left > 0 {
        if s.count <= 0 && s.state != 4 && s.state != 1 {
            s.state = 1;
        }
        return false;
    }
    s.state = if s.state == 4 { 0 } else { -1 };
    s.days = 0;
    s.count = (s.count + w.refill_count).min(w.refill);
    true
}

/// What a sale pays (`0x140347f00`): each item's worth (`synthesis::worth`, the shop's sell
/// price for one, `0x14029dce0`) times how many, summed, then scaled by 100 plus the
/// sell-price bonus (bonus value 40) percent, cut to a whole number.
pub fn sale_total(items: &[(i32, i32)], sell_bonus: i32) -> i32 {
    let sum: i32 = items.iter().map(|&(worth, count)| worth * count).sum();
    sum * (100 + sell_bonus) / 100
}

/// A ware's price for one in the shop's list (`0x140340600`): its `Cost`, at least 1. The game
/// reads the buy-cost bonus (bonus value 41) just before and drops it, so that bonus changes
/// nothing; Barrel keeps it so.
pub fn list_price(w: &Ware) -> i32 {
    w.cost.max(1)
}

/// What buying `count` of a ware costs (`0x140346ff0`): the shop list's price for one times
/// the count.
pub fn purchase_cost(price: i32, count: i32) -> i32 {
    price * count
}

#[cfg(test)]
mod tests {
    #[test]
    fn selling_and_buying() {
        // Two Crafts (cost 30) at quality 50 are worth 22 each; a 10% sell bonus.
        let w = crate::synthesis::worth(30, 50.0, &[]);
        assert_eq!(w, 22);
        assert_eq!(super::sale_total(&[(w, 2)], 0), 44);
        assert_eq!(super::sale_total(&[(w, 2), (5, 3)], 10), 64);
        assert_eq!(super::purchase_cost(70, 3), 210);
        let mut w = super::wares(&barrel_formats::table::parse_str(r#"<Root><ShopData ShopTag="SHOP_NIO" ItemTag="ITEM_X" Cost="0"/></Root>"#).unwrap()).remove(0);
        assert_eq!(super::list_price(&w), 1);
        w.cost = 70;
        assert_eq!(super::list_price(&w), 70);
    }

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
    fn restocking() {
        let mut w = wares(&barrel_formats::table::parse_str(r#"<Root><ShopData ShopTag="SHOP_NIO" ItemTag="ITEM_X" Stock="5" Refill="5" RefillCnt="3" DayAdd="10" Cost="30" Quality="50"/></Root>"#).unwrap()).remove(0);
        let mut s = Stock::new(&w);
        s.count = 0;
        // Sold out: nine days on it is still out, flagged as coming back soon.
        assert!(!pass_days(&mut s, &w, 9));
        assert_eq!((s.count, s.state), (0, 1));
        assert!(pass_days(&mut s, &w, 1));
        assert_eq!((s.count, s.days, s.state), (3, 0, -1));
        assert!(pass_days(&mut s, &w, 10));
        assert_eq!(s.count, 5);
        // Full: no days are counted.
        assert!(!pass_days(&mut s, &w, 100));
        assert_eq!(s.days, 0);
        w.refill_count = 0;
        s.count = 0;
        assert!(!pass_days(&mut s, &w, 100));
        assert_eq!(s.state, 3);
    }
}
