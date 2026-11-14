//! The synthesis bonus descriptions (`Saves[_EN]/item/item_give_eff.xml`, 53 rows, loaded by
//! `0x1400ea5a0`, one of the item manager's loaders at `[0x140dfcce0]`, as 0x48-byte rows): the
//! row number (`No.`), a short name (`ItemGiveName`), the bonus kind it describes (`skillType`,
//! a `MIX_SKILL_BASE_TYPE_*` name), for 49 of them the item category it is for
//! (`ITEM_CATEGORY`), and the help text (`&#xA;` line breaks).

use barrel_formats::table::Element;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub number: i32,
    pub name: String,
    /// The bonus kind without its `MIX_SKILL_BASE_TYPE_` prefix.
    pub kind: String,
    /// The item category without its `ITEM_CATEGORY_` prefix, if any.
    pub category: Option<String>,
    pub text: String,
}

pub fn descriptions(t: &Element) -> Vec<Description> {
    t.children
        .iter()
        .filter(|e| e.name == "item_give_eff")
        .map(|e| Description {
            number: e.attr("No.").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            name: e.attr("ItemGiveName").unwrap_or("").to_string(),
            kind: e.attr("skillType").unwrap_or("").trim_start_matches("MIX_SKILL_BASE_TYPE_").to_string(),
            category: e.attr("ITEM_CATEGORY").map(|c| c.trim_start_matches("ITEM_CATEGORY_").to_string()),
            text: e.attr("text").unwrap_or("").to_string(),
        })
        .collect()
}

/// The description of a bonus kind for an item category: the row of that kind for the category,
/// else the kind's row with no category. Barrel's lookup: how the game's screens pick a row
/// (likely by `No.`) is not read yet.
pub fn find<'a>(rows: &'a [Description], kind: &str, category: Option<&str>) -> Option<&'a Description> {
    rows.iter()
        .find(|r| r.kind == kind && r.category.as_deref() == category)
        .or_else(|| rows.iter().find(|r| r.kind == kind && r.category.is_none()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_and_finds() {
        let x = br#"<Root><item_give_eff No.="0" ItemGiveName="CP+" skillType="MIX_SKILL_BASE_TYPE_INIT_COST_ADD" text="a"/><item_give_eff No.="2" ItemGiveName="b" ITEM_CATEGORY="ITEM_CATEGORY_BOMB" skillType="MIX_SKILL_BASE_TYPE_GET_PP_ADD" text="c"/></Root>"#;
        let r = super::descriptions(&barrel_formats::table::parse(x).unwrap());
        assert_eq!((r[1].number, r[1].kind.as_str(), r[1].category.as_deref()), (2, "GET_PP_ADD", Some("BOMB")));
        assert_eq!(super::find(&r, "GET_PP_ADD", Some("BOMB")).map(|d| d.number), Some(2));
        assert_eq!(super::find(&r, "INIT_COST_ADD", Some("BOMB")).map(|d| d.number), Some(0));
    }
}
