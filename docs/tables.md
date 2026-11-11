# Tables

The game data the executable reads from `Saves*/**/*.xml`. `barrel_formats::table` reads
them (see `formats.md` for how lenient it has to be); `barrel xml-scan DIR` reads every one
and lists the rows. All 312 of Escha & Logy DX's tables read.

## Layout

- `Saves/` holds the Japanese tables and every table that has no text to translate.
- `Saves_EN/`, `Saves_SC/` and `Saves_TC/` hold the translated copies of the tables that
  do: items, shops, characters, costumes, assignments, bonuses, deliveries, posters, ranks,
  system messages and work. They have the same rows in the same order; only the text
  differs.
- Each table is a `<Root>` with one element per row; the values are attributes. A row
  refers to other rows by tag (`ITEM_MAT_THORNY_NUTS`, `SHOP_NIO`), never by number,
  except in tables that are parallel to another, such as `item_status`.
- Some attributes have Japanese names (`レベル`, `元効力値`): designers' working columns,
  some of which the game reads.

## The tables the game's systems start from

| Table | Rows | What a row is |
|---|---|---|
| `item/itemData.xml` | 560 | an item: name, image, cost, use type, stacking, kind, level, effect value, element and element values, up to four categories |
| `item/item_status.xml` | 560 | the potentials (`POT_00`…) and effects (`ESP_00`…) an item starts with; row *n* is item *n* of `itemData`, many rows empty |
| `item/item_effect.xml` | 698 | item effects |
| `item/item_potential.xml` | 400 | potentials (traits) |
| `item/item_mat_pot.xml` | 90 | material effects: type, which item kinds they apply to (`mix`, `atk`, `heal`, … as -1/0/1), condition and result tags |
| `item/item_book.xml` | 73 | recipe books: the book's tag and the recipes (`ItemTag`…`ItemTag7`) it teaches |
| `item/item_analyze.xml` | 52 | breaking an item down: what it gives and how likely |
| `shop/ShopData.xml` | 200 | one item in a shop: the shop's tag, the item, stock, refill amount and count, days between refills, cost, quality |
| `shop/ShopList.xml`, `ShopCmd.xml` | 4, 12 | the shops and their menu commands |
| `quest/QuestAlc.xml` | 93 | synthesis requests: the item wanted, points, prizes, the days it is open (`StartDate`, `EndDate`), priority |
| `quest/QuestBtl.xml` | 124 | monster-hunting requests |
| `quest/QuestGat.xml` | 305 | gathering requests |
| `subject/SubjectData.xml` | 457 | assignments: name, category, score, the card and square they sit on, the condition and its parameters, the two help texts |
| `field/AREA_*.xml` | one per area | field settings: camera, fog, light, wind and the like |

What is not in the tables: the ingredient lists of the synthesis recipes, and monster
stats. The recipe item tags do not occur in the executable as plain text either, so the
recipes are in some other file or encoded; that is still to be found.
