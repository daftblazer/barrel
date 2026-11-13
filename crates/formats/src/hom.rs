//! What the homunculi make (`Saves/hom/HomData.xml`, game mode `HOM`, loaded by `0x1400e1900`):
//! 348 rows, one per item they can be asked for: the item (`ItemTag`), `day` (5 to 50) and
//! `dec_day` (1 to 5), the material they are paid in (`mat`: Melty Gem for most, Baumkuchen,
//! Dry Potato Chips, Cloud Financier or Preserved Biscuit) and its `cost` (1 in all). The game
//! keeps each as 20 bytes, `{item, day, dec_day, mat, cost}`, the tags turned into item numbers
//! (`0x14003e9d0`). What `day` and `dec_day` count is not read yet: by the names, the days a
//! piece takes and how much each step down takes off.

use crate::table::Element;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub item: String,
    pub day: i32,
    pub dec_day: i32,
    /// The material paid (an item tag).
    pub payment: String,
    pub cost: i32,
}

/// The rows of `HomData.xml`, in order (rows missing a number are skipped).
pub fn works(t: &Element) -> Vec<Work> {
    let num = |e: &Element, n: &str| e.attr(n).and_then(|v| v.trim().parse().ok());
    t.children
        .iter()
        .filter(|e| e.name == "HomData")
        .filter_map(|e| {
            Some(Work {
                item: e.attr("ItemTag")?.to_string(),
                day: num(e, "day")?,
                dec_day: num(e, "dec_day")?,
                payment: e.attr("mat")?.to_string(),
                cost: num(e, "cost")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_row_reads() {
        let t = crate::table::parse_str(r#"<Root><HomData ItemTag="ITEM_MAT_ACORN" day="5" dec_day="5" mat="ITEM_MAT_MELTY_GEM" cost="1"/></Root>"#).unwrap();
        let w = super::works(&t);
        assert_eq!(w, vec![super::Work { item: "ITEM_MAT_ACORN".into(), day: 5, dec_day: 5, payment: "ITEM_MAT_MELTY_GEM".into(), cost: 1 }]);
    }
}
