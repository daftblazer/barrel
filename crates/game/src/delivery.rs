//! Deliveries, from `Saves/delivery/DeliveryData.xml` (`docs/tables.md`, "Deliveries"): what
//! an assignment card asks to be handed in and how a hand-in is scored. Each row names up to
//! three items with a score each, a pass score, and optional bonuses: an item category, the
//! four element values, an effect, an effect level (`AtVal`) and a quality, each with its
//! score. How the game adds these up is not read from the executable yet.

use barrel_formats::table::Element;

/// A bonus a delivered item can earn: the condition's value (a category tag, an effect tag,
/// or a threshold) and the score it is worth.
#[derive(Clone, Debug, PartialEq)]
pub struct Bonus<T> {
    pub value: T,
    pub score: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    /// `DELIVERY_CARD_02_01` ...; `DELIVERY_TEST_*` rows are the developers' tests.
    pub tag: String,
    /// The subject as the card shows it (Japanese).
    pub subject: String,
    pub pass_score: i32,
    /// The items asked for, each with its score.
    pub items: Vec<(String, i32)>,
    /// `CategoryTag`: an item of this category counts (with no score of its own).
    pub category: Option<String>,
    /// `Effect`/`EffScore`.
    pub effect: Option<Bonus<String>>,
    /// `AtVal`/`AtScore`: an effect level at least this.
    pub level: Option<Bonus<i32>>,
    /// `Quality`/`QualityScore`.
    pub quality: Option<Bonus<i32>>,
    /// `Fire`, `Water`, `Wind`, `Earth`: element values asked for (no score of their own).
    pub elements: Option<[i32; 4]>,
}

/// Every row of the delivery table, in order.
pub fn deliveries(table: &Element) -> Vec<Delivery> {
    let int = |e: &Element, k: &str| e.attr(k).and_then(|v| v.trim().parse::<i32>().ok());
    let text = |e: &Element, k: &str| e.attr(k).filter(|v| !v.is_empty()).map(str::to_string);
    table
        .children
        .iter()
        .filter(|e| e.attr("DeliveryTag").is_some())
        .map(|e| Delivery {
            tag: e.attr("DeliveryTag").unwrap_or("").to_string(),
            subject: e.attr("SubjectName").unwrap_or("").to_string(),
            pass_score: int(e, "PassScore").unwrap_or(0),
            items: (0..3).filter_map(|n| Some((text(e, &format!("ItemTag{n}"))?, int(e, &format!("ItemScore{n}")).unwrap_or(0)))).collect(),
            category: text(e, "CategoryTag"),
            effect: text(e, "Effect").map(|value| Bonus { value, score: int(e, "EffScore").unwrap_or(0) }),
            level: int(e, "AtVal").map(|value| Bonus { value, score: int(e, "AtScore").unwrap_or(0) }),
            quality: int(e, "Quality").map(|value| Bonus { value, score: int(e, "QualityScore").unwrap_or(0) }),
            elements: match (int(e, "Fire"), int(e, "Water"), int(e, "Wind"), int(e, "Earth")) {
                (None, None, None, None) => None,
                (f, w, a, t) => Some([f.unwrap_or(0), w.unwrap_or(0), a.unwrap_or(0), t.unwrap_or(0)]),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_card() {
        let xml = r#"<Root><DeliveryData DeliveryTag="DELIVERY_CARD_02_01" SubjectName="s" PassScore="100" ItemTag0="ITEM_MIX_APPLE_TART" ItemScore0="40" CategoryTag="ITEM_CATEGORY_FOOD" AtVal="1" AtScore="10"/></Root>"#;
        let table = barrel_formats::table::parse(xml.as_bytes()).unwrap();
        let d = &deliveries(&table)[0];
        assert_eq!((d.tag.as_str(), d.pass_score), ("DELIVERY_CARD_02_01", 100));
        assert_eq!(d.items, [("ITEM_MIX_APPLE_TART".to_string(), 40)]);
        assert_eq!(d.category.as_deref(), Some("ITEM_CATEGORY_FOOD"));
        assert_eq!(d.level, Some(Bonus { value: 1, score: 10 }));
        assert_eq!((d.quality.clone(), d.elements), (None, None));
    }
}
