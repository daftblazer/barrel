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

What is not in the tables: the synthesis recipes and the monsters. Both are compiled into
the executable; see "Recipes" and "Monsters" below.

## Numbers behind the tags

The tables name things by tag, but the game works with numbers. The executable keeps each
enum's names as an array of string pointers in index order, so an entry's position is
its number. `tools/str_table.py EXE TAG` dumps the array that holds TAG. In Rust, `barrel_formats::pe::Pe::enum_names(TAG)`
finds it the same way: the string, then an 8-aligned pointer to it, then the neighbouring
pointers that also point at names.

| Enum | Escha & Logy DX (English exe) | Notes |
|---|---|---|
| items | 569 tags, `ITEM_MAT_ACORN` = 0 … `ITEM_REV_CLASS_ETC` = 568; `ITEM_MIX_CRAFT` = 195 | the rows of `itemData.xml` are in this order (row 195 is Craft) |
| use types, effect types, categories | one array: 8 `ITEM_USE_*`, 6 `ITEM_ESP_TYPE_*`, then 105 `ITEM_CATEGORY_*` from `ITEM_CATEGORY_PLANT` = 0 | a category's number is its position after the first 14 |

At `0x140754bc0` there is a table of 56-byte records (14 × i32), one for each of the first
226 items, starting with the item's number. Materials have records too, so it is not the
recipe list. The code that reads it (`0x14004ff90` and its neighbours) is next to the event
helpers, so it is probably for the items' 3D objects; it is still to be worked out.

## Recipes

The recipes are an array in the executable's `.data` section, one 288-byte record (72
little-endian i32) per synthesizable item, starting from Craft and mostly in item order (record 99 jumps back from item 326 to 259) (`0x140d19c70` in the
English Escha executable). `tools/recipes.py` lists them with their ingredients. Escha &
Logy DX has 265 records, covering items 195 to 559; the recipe books at the end have empty
slots.

| Words | What |
|---|---|
| 0 | the item's number |
| 1 | the item's kind (0..7) |
| 2..4 | not known yet (2 is -1 for the books) |
| 5 | cost (300..900) |
| 6 | synthesis time in days, as a float (2.0, 3.0, ...) |
| 7, 8 | 1, then a small count |
| 9..23 | five `{effect, 0, threshold}` triples, thresholds rising (10, 20, 30, 50, 75) |
| 24..71 | four ingredient slots of 12 words |

An ingredient slot is `{kind, id, count, slot number, then four (effect, up to) pairs}`.
Kind 1 is a category (any item of it) and kind 0 a specific item; an id of -1 marks an
unused slot. The pairs are an **effect line**, and line k belongs to element k (Fire,
Water, Wind, Earth), not to the ingredient in slot k: the game reads it at that element's
attribute value (see below). In the line, each effect holds up to the value
after it, -1 meaning no effect, and an "up to" of -1 ends the line. The effect numbers are
the item effect enum (`ITEM_EFF_*`, in the same order as the rows of
`item/item_effect.xml`, whose `ItemEffectName` gives the name). For example Craft's Water
line reads none up to 20, *Embedding Thorns* up to 80, *Shredding Thorns* up to 100, and
its Wind line none up to 20, *Bursting* up to 70, *Explosive* up to 100; Flam's Fire line
is *Fire Damage* all the way. `tools/recipes.py ... EFFECT_TAGS.txt` prints the
lines.

The five `{effect, 0, threshold}` triples in words 9..23 point at the same enum, but what
they are for is still open (Craft's name Earth Damage, Rain of Fire, Water Damage, Poison and
HP regen at 10, 20, 30, 50 and 75).

### How the game reads the recipes

The accessors around `0x1400e31a0` and `0x1402a2bd0` (English Escha executable) show what
the game does with a record:

- **Finding a recipe**: a linear search for the item's number in word 0, giving the record's
  index or -1 (`0x1400e31a0`). Every other accessor takes that index and rejects anything
  above 264.
- **Ingredient count**: the slots whose id is not -1 (`0x1402a2bd0`).
- **The effect a slot gets** (`0x1402a3bf0`, called with the record, the slot and a value):
  the value is capped at 99, and the result is the effect of the first pair whose "up to"
  is greater than the value. If no pair matches, the effect of the fourth pair is used. So an
  "up to" of 100 covers the rest of the line, and a line's values run from 0 to 99.
- **Where the value comes from**: the preview at `0x14010fe20` adds up the element values
  of the materials so far, per element (each material adds its value to every element it
  has), and looks up line k at element k's total. So line k follows element k, and the
  fourth line counts even for a recipe with three ingredients.
- **The step check**: on the synthesis screen, each slot's line value is a float
  at +0x70 + 20 × slot in a 0x4e0-byte synthesis state. The game keeps one state per step,
  as an undo history. The value is truncated toward zero before the lookup, and when a step
  changes a slot's effect the screen shows a message (`0x1402fc4b9`). It reads +0x70 + 20 × line,
  one record past the attribute values at +0x5c + 20 × element that the other code uses;
  why is still open.
- **Words 3 and 4, and the levels table**: `0x14029da50` builds `{word 3, value, 2}`, where
  the value comes from a second table at `0x140d196f0`. That table holds 28-byte records:
  `{item, level, id, value, percent, 1.0f, base id}`, ended by -1. There are two levels per
  attack item (1 and 2). The percent is 100, and 125 for level 1 of the `_S` versions;
  `ITEM_MIX_SLUG_SELF_DEFENCE` also has a level 0. The value is word 4 when the item has no
  entry for that level (`0x1400e34e0`). The ids run 0..59 with a gap per item; the base id is
  level 1's id minus 1, and the `_S` versions' level 2 ids are 560..563. These are not the
  skill enum (`SKILL_*`, 543 entries at `0x140cf73c0`, all battle actions); what they index
  is still open.
- **Words 5 and 6**: cost and time, read by their own getters (`0x1402a263f` and
  `0x1402a3ea0`, which reads the time as a float). Words 3 and 4 have plain getters too
  (`0x1402a25bf`, `0x1402a25ff`).
- **The header triples**: `0x1402a3510` hands each of the five `{a, 0, threshold}` triples
  to `0x1402b49f0`, which adds them to a list. What the list is for is still open.

For example:

| Item | Ingredients |
|---|---|
| Craft | (Thorny nuts), (Thorny nuts), (Powder) |
| Flam | (Fuel), (Powder), (Paper) |
| Healing Salve | (Plant), (Oil), (Liquid) |
| Drifting Cloud | Sky Gas ×2, (Medical), (Nature), (Perfume) |

Since the recipes are not in a data file, Barrel has to read them from the player's
executable (`barrel_formats::recipe`; `barrel recipes EXE` lists them). The address differs between builds, so the engine should find the table by its
shape (a record whose word 0 is the first synthesizable item, repeated every 288 bytes)
rather than by a fixed address.

## Battle skills

Also compiled into the executable: 96-byte records at `0x140d5ab10` (English Escha
executable), 158 skills and a `DUMMY` that ends them. `tools/skills.py EXE` lists them. They
are the party's support skills ("Hero AC Attack", "Heroine AC Guard", "... AC Special") and
the bosses' skills ("True Boss Skill 3_Variation 1_Top", "Flameu_Charge"), each with its
English name and description; not the 543-entry `SKILL_*` enum, which numbers every battle
action.

| Bytes | What |
|---|---|
| 0 | pointer to the name ("Stun Blow") |
| 8 | pointer to the category ("Hero AC Attack") |
| 16 | i32: 1 for guards and self-targeted skills, 2 for attacks, 3 for boss charges |
| 24 | i32: 0 for the party's skills, 650 to 900 for bosses' (still to be named) |
| 40 | first effect: i32 effect number (-1 none), i32, i32 minimum power, i32 maximum power |
| 56 | two i32 (0, or 2 and 2) |
| 64 | second effect: number, i32, minimum, maximum |
| 80 | two i32 more |
| 88 | pointer to the description |

So Stun Blow is effect 6 at 54 to 60 power with effect 201 at 50, and Hard Guard effect 86
at 50 ("reduces damage by 50%"). Which table the effect numbers index is still open. All of
this is from the values' patterns and should be checked against the battle code.

## Synthesis skills

Escha's and Logy's synthesis skills are also compiled into the executable: 68 records of 56
bytes at `0x1407b5f40` (English Escha executable), read by the getter at `0x1400fd480`.
Each skill appears twice, once for Escha (user 0) and once for Logy (user 1).
`tools/mix_skills.py EXE` lists them.

| Bytes | What |
|---|---|
| 0 | pointer to the name ("Power+ 1") |
| 8 | type, the case of the skill switch below |
| 12, 16 | two parameters (Power+ 1: 100; Full Power: 50 and 50) |
| 20 | not known yet (0) |
| 24 | user: 0 Escha, 1 Logy |
| 28 | probably the level it is learnt at (8, 14, 31, ...); -1 for Full Power and Strengthen Material |
| 32..47 | the cost in Fire, Water, Wind and Earth points, in that order |
| 48 | pointer to the description |

The executable's own text is sometimes wrong about a skill. Quantity 2's description says
it costs 3 Earth points, but its record says 5. Barrel follows the record and shows the
game's text as is.

**Using a skill** (`0x140303620`): the game copies the current synthesis state into a new
one, subtracts the skill's cost from the four element point counts (never below 0), then
switches on the type (jump table at RVA `0x304060`, 26 cases; types 7..17 do nothing here,
since they are the passive skills: Expert Skill, Quick Synthesis, Freestyle, Inherit ...).
Three cases read so far:

- Type 4, Reset Attribute: the selected attribute's value v goes to 0, and each of the other
  three gains v / 3 × param 1 / 100.
- Type 23, Full Power: each attribute value grows by value / 100 × param 1 (cut to an
  integer), and the effect value gains param 2.
- Type 0, Conversion: param 1 is added to the selected element's point count, up to 5.

`barrel_game::synthesis` follows these rules (materials and their cost points, Conversion,
CP Recovery, Crafting Help, Reset Attribute, Full Power), with the cauldron's state kept in its own terms rather than the game's layout. A
material's element value is `itemData`'s `value` (the loaded row keeps it at +0x48, getter
`0x1402a1530`). `barrel synth INSTALL ITEM MATERIAL...` runs one with item numbers and
prints each line's effect (names from `Saves_EN/item/item_effect.xml`, whose rows are the
effect numbers): Craft from Buried Bomb, Bubbling Water and Dragon Core reaches Fire 60,
Water 40, Wind 20, Earth 40, giving Embedding Thorns, Bursting and Phy Damage L.

Without Freestyle ("allows materials to be added in any order"), the materials go in in
slot order, each slot as many times as its count; `barrel_game::synthesis::order` lists
that sequence and `fits` checks a material against an entry. A material fits a category
slot when one of its `itemData` columns `category_00`..`category_03` is that category
(the tags turn into numbers through the executable's category enum). `barrel synth` warns
when a material does not fit.

**The synthesis state** (0x4e0 bytes, one per step):

| Offset | What |
|---|---|
| +0x44 | effect value (float), at most 120 |
| +0x4c | effect value bonus from skills |
| +0x58 | the sum of the four attribute values (integer) |
| +0x5c + 20 × e | attribute value e (Fire, Water, Wind, Earth) as a float, at most 999, then the last change |
| +0x4b8 + 4 × e | element point count e, at most 5 |

**Adding a material** (`0x140304eb0`, called with the state and the material's item
instance):

- The state keeps a running sum of the materials' effect values (+0x40) and their count
  (+8). The effect value (+0x44) is that sum divided by the count, plus the skill bonus
  (+0x4c) and another bonus (+0x50), at most 120. A material's effect value starts from
  its row's `effect` in `itemData.xml`.
- For each of the four elements the material has (`itemData` columns `fire`, `water`,
  `wind`, `soil`; the loaded row keeps them at +0x4c, and the getter `0x1402a33c0` asks if
  one is above 0), that attribute grows by the material's element value. The value is kept
  in a per-material entry of the state (+0x42c + 40 × entry: the value, then a bonus), and
  it starts from the row's `value`. Elements the material lacks gain nothing.
- Each attribute is then capped at 999, and +0x58 becomes their sum.

So a Fire-and-Water material with element value 10 adds 10 to the Fire attribute and 10
to the Water attribute.

**The whole add-material step** (`0x1403040d0`): the new state starts as a copy of the
previous one, and counts the material (+4, +8, +0xc). The state keeps one 40-byte entry per
ingredient slot at +0x418 + 40 × slot, holding a pointer to the material's item
instance (+0), its cost points (+8), its element value (+0x14) and a bonus (+0x18). The step marks the slot used (+0x18) and copies
the item's two traits into +0x38. Then come the cost points:

- used = the slot's cost × a percentage the synthesis screen passes / 100;
- cost points left (+0x10, copied to +0x14) drop by that, and +0x1c records it;
- if the material costs something and the points left would be below 0, the state is
  marked invalid (+0x4d8 = 0) and the material is not added. Otherwise the elements are
  added (above), then the element points (below), and +0x3e0 counts the cost.

Where a slot entry's cost and element value are first set is still open. The state's
constructor (`0x140313690`) clears all four entries; apart from it only the skills write
them, so they are copied in whole somewhere. The synthesis screen reads them for display
(`0x140314b90` copies each entry's item, cost and value into its own lists, through
`0x140315d10`), not to compute them. The
caller at `0x140303420` passes 100 as the percentage. The skills change the entries in
place. Power+ (type 2) and Strengthen Material (24) raise the element value, and
Crafting Help (20) takes param 1 percent off the cost, never below 0.

Other skill cases found here:

| Type | Skill | What it changes |
|---|---|---|
| 5 | Draw Property | +0x3f4 gains param 1 |
| 18 | Compression | +0x4c8 gains param 1 (days), +0x4d0 gains param 2 |
| 19 | Effect+ | the effect value bonus gains param 1 |
| 21 | Quantity | +0x4d4 (uses) or +0x4cc (count) gains param 1, depending on the recipe |
| 22 | CP Recovery | cost points left (+0x10, +0x14) gain param 1, no limit |

**Element points** (`0x140305040`, only once Bonus Generation, skill type 15, is learnt):
each element the finished item has gives 1 point. Then up to two of the state's traits
(+0x38, +0x3c) are applied, from a compiled table of 90 records of 0x78 bytes (getter
`0x1402a36f0`; `item_mat_pot.xml` also has 90 rows: "火属性ボーナス×１", Fire bonus ×1,
...). Each record holds five `{type, a, b}` entries:

| Type | Effect |
|---|---|
| 0 | attribute a gains b (all four if a is -1) |
| 1 | element a gains b points (all four if a is -1) |
| 2 | +0x3f8 gains a |
| 3 | the effect value bonus gains a |
| 4 | +0x3f4 gains a |

Finally each point count is kept between 0 and 5.

## Monsters

Compiled into the executable like the recipes. The monster enum has 153 tags, from
`MONSTER_FRUITRAT_01` = 0 (the tags are in two string arrays; `tools/str_table.py` dumps
them). The getters at `0x1401118c0` and its neighbours index a table of 48-byte records at
`0x140d34810` (English Escha executable) by that number. `tools/monsters.py` lists them.

| Bytes | What |
|---|---|
| 0 | pointer to the name in the executable's language ("Poison Squirrel") |
| 8 | family (4 for the squirrels and rats, 7 for the dragons, ...) |
| 12 | 1 for bosses |
| 16 | experience |
| 20 | money |
| 24, 28 | not known yet |
| 32 | pointer to the drop list |
| 40 | pointer to the stat blocks |

The **stat blocks** are 20 i32 each, one after another, ended by a block whose level is
-1 or 0. Most monsters have one; bosses have more (the same monster at other resistances,
or stronger later fights):

| Words | What |
|---|---|
| 0 | level |
| 1..4 | HP, attack, defence, speed |
| 5 | not known yet (1 or 2) |
| 6..9 | the four element resistances |
| 12..19 | four pointers, not followed yet |

The **drop list** starts with up to four `{item, chance in percent, count}` triples, then
entries of another shape (skills, probably). Some drops have numbers above the item enum's
568; what those are is still open.

| Monster | Level | HP | ATK | DEF | SPD | Drops |
|---|---|---|---|---|---|---|
| Poison Squirrel | 6 | 35 | 15 | 8 | 13 | Apple 50%, Acorn 35%, Poison Apple 25%, Blue Nuts 10% |
| Golden Rat | 30 | 351 | 52 | 27 | 37 | Golden Apple 50%, ... |
| Altegewalt | 60 | 35,000 | 329 | 187 | 139 | resistances 40, and 75 in its second block |
