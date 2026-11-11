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
}
