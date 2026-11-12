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
| `item/item_effect.xml` | 698 | item effects: up to two battle effect slots each ("Item effects in battle") |
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

## Column types

The executable reads each table through a schema its loader builds column by column:
`0x1404de950 (schema, type, record offset, name)` (English Escha executable). There are 40
such loaders and 401 columns; `tools/xml_schemas.py EXE ASM` lists them (from an objdump
listing of the executable). The types, by the columns that use them:

| Type | What | Columns |
|---|---|---|
| 1 | a flag | `exclude`, `PC00` to `PC04` |
| 2 | an integer | `Stock`, `Refill`, `Cost`, ... (107 columns) |
| 4 | a float | `position_x`, `day`, `dec_day` |
| 5 | three floats | `scale`, `rotation`, `offset`, `Size`, `Color` (written "256,256,256") |
| 6 | four floats | `Color` |
| 7 | a string (a tag, a name) | 236 columns |

A few registrations pass the type in a way the tool does not follow (types it prints above 9).
`ShopData`'s loader (`0x14033f840`), for example: `ShopName`, `itemName`, `ShopTag`,
`ItemTag` as strings; `Stock` (+0x4c), `Refill`, `RefillCnt`, `DayAdd`, `Cost`, `Quality`
(+0x60) as integers; then `ItemEff_00`... and `Pot_00`... (the effects and traits the item is
sold with) and `EvStart`, `EvEnd` (the events between which it is on sale) as strings.

`barrel_game::shop` reads the wares (`barrel shops INSTALL [LANG]` lists them). The rows
(0x88 bytes in the list at `0x1410782d8` +0x18; `Stock` +0x4c, `Refill` +0x50, `RefillCnt` +0x54,
`DayAdd` +0x58, `Cost` +0x5c, `Quality` +0x60, the item +0x48, the shop +0x20, the events
+0x80 and +0x84) are the shops' defaults; what is sold stays in a block of its own (four shops
of 50 wares, 12 bytes each: the count, the days counted, a state for the menu; at `0x141078e14`,
0x25c bytes a shop, saved with the game, and on a new game the count is `Stock`). After every
day advance, `0x14035c560 (days)` goes over each shop's wares whose event window is open: with
`RefillCnt` 0 a ware never comes back (state 3); at or above `Refill` it counts no days;
otherwise the days count up, and once they reach `DayAdd` they start again and `RefillCnt`
more come in, up to `Refill` (the shop is flagged as restocked). Sold out and further than 30
days from a refill gives state 2, within 30 days state 1 (`shop::Stock`, `pass_days`). Nio's
first ware, the thorny nut (`Stock` 5, `Refill` 5, `RefillCnt` 3, `DayAdd` 10), sold out comes
back as 3 ten days later and 5 after another ten.

`itemData`'s schema also says what equipment gives: `HP`, `MP`, `ATK`, `DEF`, `SPD` and the
four resistances `RES_FIRE`, `RES_WATER`, `RES_WIND`, `RES_EARTH` (integers), and which party
members can use the item: flags `PC00` to `PC10`, one per playable character.

## Requests

`QuestAlc` (synthesis), `QuestBtl` (monsters) and `QuestGat` (gathering) share one schema
(loader `0x14011f6b0`): `TargetTag` (the item or monster), `GetPoint` (a float), two prizes
(`PrizeItemTag1`/`2` with a float count `PrizeItemNum1`/`2`), `StartEvent`/`EndEvent`,
`isClearSubject` (strings), `StartDate`/`EndDate` (integer days), `Priority` (a float),
`Table` and `Num` (integers; `QuestAlc` writes `num`), `PotTag`. The points and prize counts
are fractions in the tables (5.12 points, 1.7067 of a prize): how they become whole numbers is
not read yet. `barrel_game::quest` reads the requests and says whether one is offered on a
day; `barrel quests INSTALL` lists them (93 synthesis, 124 monster, 305 gathering).

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

Also compiled into the executable: one 96-byte record per skill of the `SKILL_*` enum, in
its order, at `0x140d51ed0` (English Escha executable): 533 records (the enum's first 533
tags) and a `DUMMY` that ends them. `tools/skills.py EXE` lists them. Record 0 is "Attack"
(`SKILL_ATTACK_PT`), 1 the enemies' "Attack", 2 "Run" (`SKILL_ENEMY_ANIMAL_RUNAWAY`, category
"Hamster Escape"), 200 Reyfer's "Wide Shot" (`SKILL_PT_REYFER_SKILL_1`, "Treasure Hunter
Skill 1"), 260 Katla's "Merchant Punch"; the party's support skills ("... AC Attack",
"... AC Guard", "... AC Special") and every enemy's and boss's skills are in it, each with its
English name and description.

Each party member's skills are listed at `0x140d96f90` (English Escha executable): 11
pointers, one per member in the party tables' order (Escha, Logy, Awin, Threia, Reyfer,
Lucille, Linca, Wilbell, Micie, Katla, Nio), each to 12-byte entries ended by skill -1:

| Bytes | What |
|---|---|
| 0 | the skill (an index into the skill table) |
| 4 | its place among the member's numbered skills (1 to 10; -1 for the normal attack, the support actions and Double Draw) |
| 8 | the battle command it sits on: 0 the normal attack (the member's own `SKILL_PT_<MEMBER>_ATTACK`, not record 0), 1 to 3 the skills, 4 the AC special, 5 and 6 the passives, 7 the finishing attack, 8 and on Support Attack, Support Guard, Move, Swap, Double Draw and the strengthened supports |
| 9 | the level it is learnt at (Escha: Ether Shoot 7, Super Energetic 10, Protection 15, Ancient Alchemy 20, Ether Spear 25; finishing attacks 30), 0xff when not learnt by level |
| 10 | its MP cost, a signed byte (Escha: 15, 27, 28) |

A party member's MP cost (its virtual +0xb8, `0x1401b82e0` → `0x14032f200`): the listed cost
times `(100 - x) / 100` where x sums the equipment's effects of kind 0x16 (`0x14032f7c0`, three
effects on each of four pieces), less a value (`0x14002f2f0 (0x10)`) when `0x14035ebb0` names
the member, not under 0; then times `(100 + the CHANGE_SKILLCOST values) / 100`. It is taken
from MP (`0x1401af970`, not under 0) before the action (`0x1401b8440`); the MP power of "MP
cost and power" is then worked from the `CONSUME_MP` effects alone against the MP left, and
since no skill has one it stays 100
(`battle_data::party_skills`, `battle::party_mp_cost`, `barrel party-skills EXE`).

| Bytes | What |
|---|---|
| 0 | pointer to the name ("Stun Blow") |
| 8 | pointer to the category ("Hero AC Attack") |
| 16 | i32: 1 for guards and self-targeted skills, 2 for attacks, 3 for boss charges |
| 24 | i32: 600 for the normal attacks and escape, 500 to 900 for many enemy skills, 0 for most of the party's (a wait or speed value, probably) |
| 40 | first effect, six i32: an `ACT_*` number (-1 none), the element (-1 physical, 0 to 3 Fire, Water, Wind, Earth: Bomb 0, Ice Bomb 1, Lightning Bomb 2; the battle's effect +4), minimum power, maximum power, two i32 (0 and 0, or 2 and 2) |
| 64 | second effect, the same six |
| 88 | pointer to the description |

The effect numbers are the executable's `ACT_*` enum (234 names, `ACT_DAMAGE_HP` first),
and they fit the descriptions: "Attack" is `ACT_DAMAGE_HP` at 64 to 80, "Run" `ACT_RUNAWAY`
at 95 to 100, Hard Guard `ACT_ASSIST_GUARD` at 50 ("reduces damage by 50%"), Stun Blow
`ACT_ASSIST_ATTACK` at 54 to 60 with `ACT_RETURN_WT` at 50 ("delays the target's turn"),
Plasma Break `ACT_DAMAGE_HP` with `ACT_PERCENT_QUICK` ("allows you to take another turn"),
Wide Shot `ACT_DAMAGE_HP` with `ACT_DAMAGE_HP_TURN_DIFFERENTIAL` at 1 ("continuous damage for 1
turn"). `tools/skills.py` prints the effects by name. An unused slot can keep stale numbers
after its -1. The other fields are named from the values' patterns only and should be checked
against the battle code.

### Building an action's effects (executable)

`0x14015f560 (skill, list)` (rejects skills above 531) appends a skill's two effect slots to
an action's effect list, one 0x34-byte entry each: the slot's six i32 copied as they are
(`ACT_*`, i32, minimum, maximum, two i32), then `0, -1, 0, 0`, a float scale of 1.0 (+0x28),
a zero byte and -1 (+0x30). An empty slot is appended too, with its -1 number.

Its one caller, `0x14032f420`, then raises the effects when the character has a bonus of kind
0x17 (summed over 3 parts of each of 4 slots at character +0x194, stride 0x24; equipment
traits, most likely) and the skill is not one of 0, 4, 7 and 8 to 16: each of the four values
from +8 (minimum, maximum and the two after) becomes `value × (min(bonus, cap) + 100) / 100`
(integer division), where `cap` is the effect's limit from the `ACT_*` property table.

The `ACT_*` property table at `0x140ce9590` (English Escha executable): 20 bytes per number
from 0 to 233: 8 flag bytes (`0x14002aa10 (act, k)` reads flag k), then i32 (1 for the damage
kinds, 0 otherwise), i32 the limit on bonuses in percent (9999 for most; 0 for `DAMAGE_FIX`,
`DAMAGE_PERCENT`, `RUNAWAY`, `PERCENT_QUICK`, so no bonus; 100 for `RETURN_WT`, 500 for
`ADD_MAXHP`; `0x14002aa40` reads it, 9999 out of range), i32 (5 for `DAMAGE_HP`, 25 for
`ASSIST_ATTACK`). The readers also accept -1, whose 20 bytes are whatever precedes the table.

| Flag | Set for | What the amount roll does with it |
|---|---|---|
| 0 | 78 numbers: `RUNAWAY`, `RETURN_WT`, `ASSIST_GUARD`, `PERCENT_QUICK`, ... | when clear, the scale is also multiplied by the roll's float argument |
| 1 | damage and heals | x (A's value 14 + 100) / 100 when B has state 7 |
| 2 | damage | x 0.5 when B has state 3 |
| 3 | damage, heals, `ASSIST_*` | (items) the item's quality terms are added to the bonus |
| 4 | `DAMAGE_HP`, `DAMAGE_REDUCE`, heals | (items) a flat addition from the item's value 0x27 |
| 5 | damage | x percent 1 / 100 |
| 7 | damage and heals | x percent 2 / 100 |


### Rolling the amounts (executable)

`0x1401bd520` (English Escha executable) then goes over the action's effects: for each
effect with a number, the amount (+0x20) is a random number from the minimum to the maximum
(+8, +0xc), and the second value (+0x24) one from the two values after (+0x10, +0x14). Then, for an item (action
kind 1) whose effect has flag 3, a bonus is summed from the item's values (quality and other
item properties, `0x14029db00` on the item at A +0x31c); for a skill (kind 0) the bonus is
one of A's values 0x6f, 0x70 or 0xde when the skill's kind (+0xc) matches. With flag 4
an item adds `extra`: its value 0x27 if the amount is at most that, else 0x27² / amount.

The item's "values" are its traits' (`0x14029db00 (item, kind, average)`: over its three
traits, item +0x8, each a 0x58-byte record of `item_potential.xml` with two slots {`PotBase`
kind, `min`, `max`} at +0x3c, the slots of that `ITEM_SUB_*` kind summed, each rolled from
minimum to maximum, or their middle when `average`). Kind 39 (`CHANGE_ACTUAL_POWER`) is the
flag-4 value. The flag-3 bonus (`0x1401bd8ee`) is the sum of: kind 0 `ITM_CHANGE_POWER`; 25
`BTL_RANK_BONUS` × (5 − `0x14029dd10 (item)`); 31 `BTL_COST_BONUS` × `0x14029dc50 (item)`; 28
`BTL_LAST_BONUS` when the item's i16 +0x1c is 1 or under `FIELD_ITEM_SINGLE_USE` (state 0x4f);
35 `BTL_COUNT_POWER` × (+0x1e + +0x20); 34 `BTL_FORCED_LASTUSE` × +0x1c; against a monster
target, 29 `BTL_BIG_ENEMYBONUS` × s and 30 `BTL_SMALL_ENEMYBONUS` / s with s =
`0x140111a80 (monster +0x78)` + 1; 26 `BTL_LARGE_ENEMYBONUS` × and 27 `BTL_SHORT_ENEMYBONUS` /
(unsigned) the number of targets; 41 `COST_PROPORTION_BONUS` × +0x18; and 42
`WT_PROPORTION_BONUS` × `0x140190b90 (timeline, user)` / 100 added in floats. Each trait slot
is rolled as its kind is asked for, in that order. `battle_data::traits` reads `item_potential.xml`'s rows (name, group and the two slots with their `ITEM_SUB_*` tags; `ITEM_SUB_BASE_NONE` left out) and `battle::trait_value` sums one kind over an item's traits as `0x14029db00` does. `fight::Battle::use_item_boosted` applies the bonus: a `Boost` (the item's traits, the `ITEM_SUB_*` names, its `ItemBonusInputs` and the act table) raises each flag-3 act's rolled amounts through `boost_amounts` and adds `actual_power_extra` on flag-4 acts, right after each roll as the game does. With a bonus b or an extra
(`0x1401bdbcc`): b is cut to the act's bonus limit (the per-act table at `0x140ce9590`: 20-byte records by act number, 12 flag bytes asked by `0x14002aa10(act, flag)`, the limit asked by `0x14002aa40`, 9,999 for most and 0 for some, e.g. `DAMAGE_FIX`, a last word; flag 3 is set on 77 acts; `battle_data::acts`, `barrel acts EXE`), and the amount becomes `trunc(amount ×
(100 + b) / 100) + extra` and the second value `trunc(second × (100 + b) / 100)`
(`battle::ItemBonusInputs`, `item_bonus`, `boost_amounts`, `actual_power_extra`; what
`0x14029dd10`, `0x14029dc50`, `0x140111a80`, `0x140190b90` and the item's +0x18 to +0x20 are is
taken from the trait names, not read). Then

    amount = amount × (min(bonus, limit) + 100) / 100 + extra     (integers, truncating)
    second = second × (min(bonus, limit) + 100) / 100

and the float scale (+0x28) starts at 1 (or character A's value 0x2b4 / 100 when positive, for
action kinds other than 2) and is multiplied by the flag-gated factors in the flag table above, then
by `(global 0x141077e70 + 100) / 100` (a running bonus: `0x1401d07a0` adds to it, `0x1401d08b0`
clears it; fed by 10, 25 and 5 point steps in `0x1401bd1c0`, a chain bonus by the look of
it) and `global 0x141077e50 / 100` (the MP power, below), then by `1 - clamp(
character A's state-12 value × 0.1 / 100, 0, 1)`, and is kept at least 0.15. The function takes two characters, A (its second argument), the user, and B (its third), the
target. `0x1401bd000 (battle, action)` looks the user up from the action's actor (+0x10) and
calls `0x1401bd410 (battle, user, target, action)`, which rolls (`0x1401bd520`) and applies
(`0x1401bde10`) the effects, once per target as the action's +0x38 says: 0 and 2, every
character in the battle's target list (+0x40 to +0x48); 1, the one character at
battle +0x938; 3, one target of the list drawn at random (`0x1404ca740`). The numbers it asks a character about come in
three kinds:

- passive traits, through the character's virtual "has" (+0x48) and "value" (+0x50): these
  numbers are `ACT_*` numbers and fit the passive names: 0x72 `PASSIVE_GREAT_BATTLE`, 0x71
  `PASSIVE_INNER_FIRE`, 0x6f `PASSIVE_ASSIST_ALLUP`, 0x70 `PASSIVE_ASSIST_DEFUP`, 0x9c
  `PASSIVE_CHANGE_ITEM_POWER`, 0xe1 `PASSIVE_CATEGORY_UP`, 0xde `FIX_SKILL_BONUS`;
- timed states (`0x1401afd10` has, `0x1401afc70` value: two lists at character +0x1a0 and
  +0x1c0), numbered by the state table below: 0x1e `MITIGATE_DAMAGE`, 8 `PROTECT_DAMEGE_NUM`
  (the roll also uses up one of its charges), 0x2f `CHANGE_DAMAGE`, 0x30 `CHANGE_ITEMEFFECT`,
  0x33 `CHANGE_ITEM_POWER`, 0x47 `FIELD_RACE_STRENGTH`;
- bad-status bytes at character +0x280 (`0x1401ae380 (c, k)`), where k = 4 instead means "HP at
  most 25% of maximum".

Percent 1 starts at 100, or at 100 + A's `PASSIVE_INNER_FIRE` value when A's stat 0 is at most
that passive's second value above B's; it adds A's `PASSIVE_GREAT_BATTLE` (when B qualifies),
subtracts B's `PASSIVE_INNER_FIRE` under the mirrored condition, and subtracts B's
`MITIGATE_DAMAGE` and `PROTECT_DAMEGE_NUM`; percent 2 starts at 100 + A's
stat 15 (stats are i32 at character +0x88) and adds B's `CHANGE_DAMAGE` (skills),
`FIELD_RACE_STRENGTH`, `CHANGE_ITEMEFFECT` and `PASSIVE_CHANGE_ITEM_POWER` (items), less
`CHANGE_ITEM_POWER`. Some of these conditions are read only roughly and should be checked.

Timed states are added by `0x1401bf1e2` (the shared handler of the timed effects), which maps
the effect's `ACT_*` number to its state number with `0x1401c0e80`:

| State | `ACT_*` | State | `ACT_*` |
|---|---|---|---|
| 1 | `AUTO_REVIVE` | 0x26-0x28 | `MINUS_ATK`, `MINUS_DEF`, `MINUS_SPD` |
| 2 | `HEAL_HP_TURN` | 0x2e | `CHANGE_SKILLCOST` |
| 4 | `ADD_GUTS` | 0x2f | `CHANGE_DAMAGE` |
| 5 | `ADD_ACTION` | 0x30 | `CHANGE_ITEMEFFECT` |
| 6 | `REDUCE_WT` | 0x31 | `COST_HP_BY_WT` |
| 7 | `ADD_PERCENT_QUICK` | 0x32 | `REDUCE_WT_ITEM` |
| 8 | `PROTECT_DAMEGE_NUM` | 0x33 | `CHANGE_ITEM_POWER` |
| 0x11 | `PLUS_LEVEL_STATUS` | 0x34 | `DAMAGE_HP_TURN` |
| 0x12-0x14 | `PLUS_ATK`, `PLUS_DEF`, `PLUS_SPD` | 0x35 | `DAMAGE_HP_TURN_DIFFERENTIAL` |
| 0x1a | `ADD_MAXHP` | 0x46 | `AVOID_TARGET` |
| 0x1b | `CHANGE_CRITICAL` | 0x47 | `FIELD_RACE_STRENGTH` |
| 0x1c | `ADD_EVADE` | 0x48-0x4a | `FIELD_ACUMULATE_DAMAGE`, `_WEAK`, `_HEAL` |
| 0x1d | `MITIGATE_BADSTATUS` | 0x4c-0x4e | `FIELD_INVALID_BUFF`, `_REVIE`, `_HEAL` |
| 0x1e | `MITIGATE_DAMAGE` | 0x4f-0x52 | `FIELD_ITEM_SINGLE_USE`, `FIELD_NO_ITEM_USE`, `_LIVING`, `_TIME` |
| 0x24 | `INCREASE_CRITICAL_DAMAGE` | | |

The other state numbers (3, 9 to 0x10, ...) are not reached from this mapping (the bad
statuses and others, presumably).

The shared handler builds a state event (vtable `0x1408090e0`) holding the state number, the
rolled amount and the effect's second value; a target under field state 0x4c
(`FIELD_INVALID_BUFF`) gets value 0 for the buffs (states 0x12 to 0x1a and 0x26 to 0x2d).
Its apply (`0x1401c0dd0`) adds the state through `0x1401afb80` to the list at character
+0x1a0 (`0x140191340`; +0x1c0 in another battle mode). A state is a 0xa4-byte record:
+0 the state, +4 its count, +8 the value as a float, +0xc a further argument, +0x9c its
exclusion group (`0x140191db0`), +0xa0 an active byte. The count (`0x140191d00`) is the
effect's second value for the counted states (2 `HEAL_HP_TURN`, 3, 5 `ADD_ACTION`, 6
`REDUCE_WT`, 0x34 `DAMAGE_HP_TURN`, 0x35 its differential form: Bomb's poison, value 10,
lasts 2), 5 for the buffs and debuffs (7 and 0x12 to 0x2d), and -1 (no count) for the
rest. The exclusion group (`0x140191db0`) pairs each `PLUS_*` stat state with its `MINUS_*`
(0x12/0x26 group 1, 0x13/0x27 2, 0x14/0x28 3, 0x15/0x29 to 0x19/0x2d groups 5 to 9); 0 and 1
are group 10, 0x1d/0x1e 11, 0x1a 12, 0x11/0x2f 13, the rest none. Adding a state meets the
first state of its group (or of the same number, without a group): a meeting state of the same
number with no count stays when the new value is not stronger (compared as floats); otherwise
it goes and the new one is added at the end. So a buff wipes the matching debuff, and a poison
put on again starts afresh with the new value and count. `battle::state_count`, `state_group`, `TimedState` and `add_state` follow this.

At the start of a character's turn (`0x1401ae4f0`; when +0x282 is set) `0x1401b0100` works
out its HP change: the `HEAL_HP_TURN` states (2) and two traits (0x65, 0x83) add to it, and
the poisons take from it. State 0x34 (`DAMAGE_HP_TURN`) takes `0x1401b9050 (sum of the 0x34
values, defence)`: with `cut = clamp(trunc(defence / 10) + trunc(defence / 3), 0, 999)`, the
damage is `trunc((100 - cut) × value / 100)`, or 1 when `(100 - cut) × value` is below 200.
State 0x35 (`DAMAGE_HP_TURN_DIFFERENTIAL`) takes `0x1401b90c0`: `(100 - cut) × (max HP - HP)
× value / 10000`, or 1 below 20000 (value percent of the HP missing). Field states 0x48 and
others follow (not worked). The parts are netted into one change: when it is positive a heal
event (`0x1401c1d00`) gives it back, when negative its opposite is dealt as an ordinary damage
event (`0x1401c1970`, element slot 4: the target's +0x2b8 cut, at least 1), and at 0 nothing
happens; a heal-over-time thus cancels a poison before either is shown (`battle::
turn_hp_change`).

A state changes stats through a 35-stat block in its record (+0x10), filled when it is added
(`0x140191e90`) with its value on the stats it touches: attack (5), defence (6) and speed (7)
for `PLUS_*` (0x12-0x14), `MINUS_*` (0x26-0x28) and two further families (0x36-0x38,
0x3e-0x40); stats 8 to 12 for the next five of each family (0x15-0x19, ...); 13 (critical) for
0x1b, 14 for 0x24, 18 (evade) for 0x1c; all of attack, defence and speed for 0x49. 0x11
(`PLUS_LEVEL_STATUS`) works from the attack (not read further). The amounts are flat: the
`MINUS_*` acts (45-47) have their own entry (`0x1401bf1da`) that negates the amount and then
joins the shared handler, so Siren (9 to 10) takes 9 or 10 attack off and Howling (14 to 15)
adds 14 or 15. Whenever a state is added or goes, `0x1401af440` recalculates: the 35 base
stats (+0x114) copied over the stats (+0x88) but the current HP and MP kept, each list's
blocks added (`0x140191a30` → `0x1401a8e00`), maximum HP raised by the sum of `ADD_MAXHP`
(0x1a) percentages, then maximum HP and MP kept to 1..999 (999,999 when the character's
vtable +0x10 says 1, monsters), HP and MP to 0..the maximum, and attack, defence and speed to
1..999 (`battle::state_stats`, `stats_with_states`, `stat_state`). Then `0x140191120` ages both state lists by the character's
+0x2d8 (taken as one turn): counted states go down and are dropped at 0. `battle::
poison_damage`, `poison_by_missing_hp` and `age_states` follow this, and the duel poisons with
`DAMAGE_HP_TURN` effects (Bomb: 10 for 2 turns, 5 a turn against the Grand Dragon's defence).

### MP cost and power (executable)

A skill's MP cost is `0x14015f6a0 (skill, CONSUME_MP, 0)`: the sum, over its effect slots whose
number is `ACT_CONSUME_MP` (11), of a number rolled from the slot's minimum to its maximum (a
draw only when they differ). A user with less MP than the cost still acts, at less power:
global `0x141077e50` becomes `min(100, MP × 100 / cost)` (`0x1401b8475`; MP is stat 2), and
every effect's scale is multiplied by it / 100. After each action (`0x1401c0676`) it is set
back to 100. `barrel_game::battle::mp_cost` and `mp_power` do the above.

### Turn order (executable)

The battle's timeline (battle +0xa28) is a list of entries in turn order, each with a wait
(+0x14, kept from 0 to 1000) and the character it belongs to (+0x18, matched against the
character's +0x70; entries flagged at +0xe, or whose +0x20 object is not of kind 3, are
skipped when looking a character up). Two functions move a character:

- `0x14018d490 (timeline, character, amount)` adds to its wait and moves it back behind
  every entry whose wait is no more than its new one (on a tie, it goes last);
- `0x14018d5e0 (timeline, character, amount)` takes from its wait and moves it forward
  ahead of every entry whose wait is no less than its new one (on a tie, it goes first).

Time moves on in `0x14018c7e0 (timeline)`: it sorts the entries by wait, pushes the entries
at the front whose +0x10 is negative back by 1000 (at most 1000; +0x10 is -1 for a fallen character, "Falling and guts"),
takes the first other entry's wait w off it and every entry after it (not under 0), tells the
battle that w passed (`0x1401b5b50 (battle +0x8d0, w)`, for the timed states, presumably) and
sorts again. The sort (`0x1401a8400`, the standard library's introsort) compares waits only;
for lists under 32 entries it is an insertion sort, so entries with equal waits keep their
order, which a reimplementation must keep too.

Entries whose +0x10 (the character's virtual +0x20 value) is negative are free slots.
`0x14018cf20 (timeline, character)` puts a joining character in a free slot with wait 500
(1000 when its +0x10 value is negative). After a character acts, `0x14018d180 (timeline,
character, wait)` gives it the action's wait (the action's virtual +0x30):

1. with timed state `ADD_PERCENT_QUICK`, a chance roll on its value (`0x1404ca5f0`: the next
   number's low 31 bits modulo 100, under the value) makes the turn quick, a 25% cut;
2. with passive `PASSIVE_PERCENT_QUICK`, a chance roll on its first value makes the cut its
   second value instead;
3. a quick turn's wait is `trunc(wait × (100 - cut) / 100)` in floats; a character for which
   `0x1401b07c0` is false gets 0; the wait is at most 1000;
4. the entry gets the wait (and the character's +0x2d8 a copy); a scheduled entry of the
   character's is freed; the character's +0x84 counts up (its turns, presumably).

Each roll draws only when the character has the state or passive, so they must be made in
that order. Scheduled entries (+0xd set, an action object at +0x20; `0x14018db00` and on)
hold actions that happen later at their own wait, such as charged skills.

`ACCELARATE_WT` calls the second with the effect's amount. `RETURN_WT` (`0x1401c0313`) first
counts the push on the target (+0x30c) and pushes it back by `trunc(1 / count × amount ×
(100 - RESIST_KNOCKBACK) / 100)` in floats, so each push on the same target is weaker than the
last. The action's wait comes from the skill's (+24 in the record; 600 for the normal attack,
`0x14015f9c0`; a monster's slot k through its virtual +0xc0, `0x1401b7690`) by
`0x1401b9b70 (character, wait)`:

    w = trunc((100 - min(speed / 5, 25)) × wait / 100)     in floats; speed is stat 7
    with timed state 0xb:      w += value / 2 + 50
    with timed state REDUCE_WT: w = (100 - min(value, 50)) × w / 100
    if global 0x141077e54 is not negative: w += it, and it becomes 0
    w = max(w, 0)

So speed takes up to a quarter off every wait (at speed 125 and more). When `RETURN_WT`'s
count resets is still to be read. `barrel_game::battle::Timeline` (`advance`, `delay`, `hasten`), `action_wait`,
`wait_after_action` and `knockback` do the above.

### Falling and guts (executable)

After each effect on a target (`0x1401c0676` and on) the game sets the MP power back to 100,
runs the target's virtual +0x130, updates its standing flag (+0x282, `0x1401b0540`: false
when its maximum HP is positive and its HP is 0 or less) and, when it has just gone down, its
virtual +0xf0. A party member's +0x130 (`0x1401b8850`, the party vtable `0x140808540`) is
**guts**: with HP at 0 and +0x28d clear, the chance is stat 25 percent at most 75, or 100 under
`ADD_GUTS` (state 4, removed before the roll whatever it gives) or while +0x2b8 or +0x2c0 is
positive; the roll (`0x1404ca5f0`) is always drawn, and on success the member stays at 1 HP and
+0x28d is set until a heal clears it. The down handler (`0x1401b0580`) clears the character's
timed states (`0x1401919a0`), drops its pending action (+0x1e0) and marks its timeline entry
(+0x10 = -1, `0x14018ccf0`); time's advance then pushes such entries at the front back by 1000
(no further than 1000) and takes the time from the first entry still in the running, so a
fallen character never gets a turn until revived (`battle::guts_chance`,
`Timeline::advance_with`, `fight::Battle`).

The after-effect step tells the two changes apart: a target that has just fallen gets virtual
+0xe8 (`0x1401b0570`, every class: the down handler above), one that stands again +0xf0 (the
party's `0x1401b7ed0`, the monsters' `0x1401b0660`): it clears its timed states again, and for
a party member updates its timeline icon (`0x14018ce80` → `0x1401cddd0`; where the entry's -1
is reset is not read). **Reviving** is `CURE_REVIVE` (81, `0x1401bfe36` → the revive event
`0x1401c1e90`): `trunc(maximum HP × value / 100)`, nothing under `FIELD_INVALID_REVIE` (state
0x4d) nor under 0x4e for a target still standing, given through the heal (`0x1401afa20`), so it
also adds HP to a standing target (Vital Check: about three quarters of the attack plus 14 to
15% of maximum HP) and raises a fallen one from 0 (`battle::revive_amount`).

### Item effects in battle

A battle item's effects are the item effects its synthesis gave it, and each row of
`item/item_effect.xml` holds up to two effect slots in a skill's shape. The loader
(`0x1400e8b00` and on) reads `ItemEffectName`, `g_tag`, `value`, `actTag` with `attributeTag`,
`min_1`..`max_1` (its amount) and `min_2`..`max_2` (its second value), then `actTag2` with
`attributeTag2` and `min_3`..`max_4`; the rows' unsuffixed `min` and `max` and the Japanese
design columns (`Lv想定`, `コスト`, ...) are not read. Element tags are Fire, Water, Wind and
Earth (0 to 3), anything else physical; 613 rows have no second act. For an item action the
damage leaves out attack and defence and puts the effect entry's +0x18 in place of the user's
attack: `0x14029d270`, which builds a held item's effect entries (0x34 bytes each) from the
effect definitions (0x58-byte records, two slots 0x18 apart from +0x28), sets +0x18 to the
item's value at +4 (a float after its 16-bit id at +2; its quality, presumably) × 0.5 + 75,
truncated (`0x14071a9a0` = 0.5, `0x1407b9b04` = 75; `battle::item_power`: quality 50 gives
100). The entries come from the instance's four effect ids (16-bit at +0x10, -1 none): each
names an `item_effect` row (up to 0x2b9), whose used slots (act ≥ 0) follow in order
(`battle::item_action`). Two refinements are not followed yet: an override record some items
have (`0x1400e3480` by item id: the first slot, or the slot equal to its +0x18, takes its +0x8
instead), and traits of kind 7 (two 16-bit trait ids at +0x8 of the instance when +0xe is 0;
the trait table's records +0x3c) adding effects: each `ITEM_SUB_BTL_ADD_USEEFFECT` slot is
rolled from its minimum to maximum (`0x1402a1e20` reads the slot's bounds and rolls them) and
names an `item_effect` row whose used slots follow the item's own (`battle::trait_effects`).
In `item_potential.xml` those bounds are effect tags (Fatal Blow: `ITEM_EFF_YOBI_072`, Mortal
Blow, `ACT_PERCENT_DEATH` 100; Recycle: `ITEM_EFF_YOBI_104`, Reduce Gauge L), which
`battle_data::traits` reads as row numbers. The quality bonus of
flag 3 is not read yet
(`battle_data::item_effects`, `barrel item-effects EXE`).

`barrel_game::days::World` keeps what the days touch (the date, the main generator, the
board's offset, the characters' friendship and list slots, the rank, the sixty month reports (one per month since the start),
the board, the last day's rolls) and `advance` works each new day in the game's order: the
rolls, the friendship gains, closing the month just ended on a month's first day, and the
board redraw on the 1st, 11th and 21st (`World::new_game` starts on April 1st and draws the
board's offset first), then, once for all the days, the shops' restock. The request expiry pass and the manager's monthly work are left out. `barrel days
INSTALL N [--events]` runs a new game's first N days on it (a fixed generator state, everyone in
the list in slot order) with every shop emptied at the start, and prints each board redraw, month close and restock
(22 wares, those that come back every 10 days, on April 11th and 21st): boards on April 11th,
21st, May 1st, ..., April's report closing on May 1st. The days a world-map trip takes (reason
4) are the trip object's +0x24 (vtable `0x140772c48`, built by `0x14007b920` with 1 there);
what sets it is still not read.

`World::from_save` sets the calendar from a save (`docs/formats.md`, "Saves"): the date, the
board's offset (`quest/randseed`), each character's friendship (its `Party` record), the
11-slot list (`party/m_squad`, whose characters are also those the daily gain goes over, its
first the leader), the rank with its points by period (`subject/job_rank`, `job_point`,
`job_point_interval`, which sum to the points), the month reports (`monthrep/data`) and the
shops' stock (`shop/shop_data`, each ware found by its shop's order of first appearance in
the ware table and its place among that shop's wares). The main generator is not in the save.
The last day's rolls come from the event records, and `barrel days --from` takes the save's seen events for the event windows (with 94 seen, the redrawn boards hold monster and gathering requests too). The board comes from `quest/now` through `quest::board_from_save` (Craft, Healing Salve and a
Minidragon request in one save, slots 0, 1 and 6).
`barrel days INSTALL N --from SAVE` runs the days from a save: from one at 20 August of year 1
the board is redrawn on the 21st and on 1 September, August's report closes that day, and the
friendship gains follow each character's squad slot (0.08 a day in slot 4, 0.11 in slot 2).

### The adventurer rank

`Saves/rank/RankData.xml` has 21 rows (`point`, `coal`, `percent`, the rates `c_btl`, `c_gat`,
`c_bou`, `c_mix`, `c_tra`, `c_rel`, `c_que`, and `rank`, the number shown), loaded as 0x28-byte
rows at `[0x140e00f00]` +0x18. The rank is a row index (global `0x14109c1e4`) and the points an
integer (`0x14109c1e0`). When the points change (`0x14035fdc0`) the rank climbs from where it
is through every next row whose `point` is at most the points, no further than the last row,
and never goes down; if the climb passes the top of the current tier of three (`0x14035fd90`:
`(trunc((rank - 1) / 3) + 1) × 3`) the manager at `0x140dfc220` is told first (`0x1402681b0`).
The month-end report keeps the rank. Points are added by `0x14035fc90 (n)`: to the
total, to the current assignment period's count (`0x14109c1bc` + 4 × the period at
`0x141099e04`) and to the month's report (+0x138), then the promotion above. They come from
requests done (`trunc(GetPoint)` when above 0, `0x14033b24d`), assignments cleared (their
`Score`, `SubjectData` +0x24, `0x14035775c`), lines completed on an assignment card
(`lineScore`, with `lineBonus` applied through `0x14002f310`, `0x1403605c6`), event commands
(`0x14002f410`), and are taken back by the negated sum of 25 assignments' scores
(`0x1400cd9a2`). The other columns are the **month's pay** (the report screen, `0x140114530`;
rows loaded as `point` +0, `coal` +4, `percent` +8, `c_btl` ... `c_que` +0xc to +0x24): a base of
`coal`, a bonus of `trunc(the month's points × percent / 100)` (at least 0), and each `c_*` rate
times the month's count of that activity (the month report's +0x10c to +0x124, copied to the
screen's +0x14 to +0x2c, each raised by one through its own function, `0x1403639c0` + 0x50 × k:
+0x10c from battle code (`0x1401d7edc`, battles), +0x110 from three field places (`0x14008d243`
and on, gatherings), +0x114 directly (`0x14035dcaa`, `bou`), +0x118 and +0x11c from synthesis
(`0x14030e1b5`, `0x14030e1c2`: syntheses and `tra`), +0x120 from four field places beside the
gathering ones (`rel`), +0x124 from requests done (`0x14033b387`)), all added; the
report keeps the base (+0x128) and the bonus (+0x12c) and is marked paid (+0x140 = 2)
(`rank::ranks`, `tier_top`, `promote`, `RankState`, `request_points`, `pay`).

### The calendar

The date is one integer, the global at `0x1410a42cc` (read by `0x14035c050`; English Escha
executable), counting days from 0 in 30-day months and 360-day years: `v = 360 × (calendar
year − 1) + 30 × (month − 1) + (day − 1)`. The game shows it in school years from April
(`0x1402f0300`: year `v / 360 + 1` for April to December, `v / 360` for January to March;
month `v / 30 mod 12 + 1`, day `v mod 30 + 1`) and builds it back with `0x1402f02d0 (school
year, month, day)` = `(year' × 12 + month) × 30 + day − 391`, `year'` being the school year
plus one for January to March. April 1st of school year 1 is 90.

The nine assignment cards of `subject/SubjectCardData.xml` are four-month periods from there
("Year 1 April to July", "Year 1 August to November", "Year 1 December to March", ... "Year 3
December to March"). Every `StartDate` and `EndDate` of `quest/QuestAlc.xml` is 91, 211, 331,
..., 1171 or 1530: one more than a period's first day (April 2nd, ...; how requests compare
them with the date is not read). `barrel_game::calendar` follows this, and `barrel quests`
shows the dates.

A new game sets the date to April 1st of school year 1 (`0x14035be60`, and `0x1410782e4` to
0); the save holds both. Days pass only through `0x14035bf10 (days, reason)`, called with
reason 1 (`0x140052b5a` and two more), 2 (synthesis, its days, `0x14030d8a6`), 3 (one day,
`0x14003412b`), 4 (a record's +0x24 days, `0x14007bf51`: world-map travel, presumably), 7 (one
day, field code `0x1400795a8`, `0x14007ab66`) and 8 (one day, `0x14039b3d2`, `0x14039ba9c`).
For each day it adds one (`0x1402f03a0`), works the new day (`0x14035bb50`) and queues a
message (kind 8, the reason) at `0x140df9988`; then `0x14035b930 (days, reason)`. A new day:
1165 records of 0x24 bytes from `0x1410a42e0`, one per event (`EVENT_*` order; the saved event records, `docs/formats.md`), each get a roll from 0 to 100 (each a draw from
the random generator), then every character but one (`0x140330120 (0x141098a48)`, the
leader presumably) adds to its record's float +0x2a4 (records of 0x2b0 bytes at
`0x1410bcb60`) a gain by its place in the 11-slot list at `0x141093530`: 0.11 in slots 0 to 2
(or not in the list), 0.08 in 3 to 5, 0.03 below, kept at 100 at most (friendship points,
presumably). On a month's first day (date divisible by 30) `0x140363860` closes the month just ended: the
months from the game's first day (`0x1402f02d0 (year 1, April, 1)`) to the first of the new
month, less one, so 0 for the first April, 12 for the second (`calendar::ended_month`; sixty
reports, five years, are kept and saved, `monthrep/data`): its report (`0x1403637e0`, 0x144-byte records at
`0x141093570`) sums the month's syntheses, a byte per recipe for the 265 recipes (+0 to +0x108),
each times the price (`itemData` `cost`, +0x24) of the item the recipe makes, into +0x134,
stores the rank (global `0x14109c1e4`, which only rises, through `0x14035fec0` and the
manager's `0x1402681b0`) at +0x13c and marks the report new (+0x140); then `0x140267f50` (the
manager at `0x140dfc220` +0x120) runs (`calendar::ended_month`, `month_total`). Then, for each 0x34-byte record of a list from
`0x140168990`, one whose `0x140168710` (a float) is 0 or less and that is not yet flagged
(+0x31) is flagged and handed to `0x140268290 (manager +0x120, its i16 +2)` (expiring
requests, presumably). Last the **request board** (`0x14033aaa0` on `0x140dfc880`): on the 1st,
11th and 21st of each month (day of month mod 10 is 1) global `0x1410989e8` is cleared and
`0x14033ab20` redraws its ten 0x34-byte slots at `0x1410985e0`: three synthesis requests
(`quest/QuestAlc.xml`, `0x14033b5e0`), four battle requests (`QuestBtl`, `0x14033b990`; their
count `min(n / 2, 10)`, at least 1) and three gathering requests (`QuestGat`, `0x14033bd10`) with a generator of its own, seeded from the date plus global
`0x1410989f0` by the Mersenne Twister seeding step: `s0 = (x ^ (x >> 30)) × 0x6c078965 + 1`
with `x` that sum, `s1` the same of `s0` plus 2 (the shift arithmetic). The board's generator is the object's own two words (`0x140dfc880`), stepped like the
battle's Xorshift on two words (`t = x ^ (x << 11)`, `x = y`, `y = y ^ (y >> 19) ^ (t >> 8) ^ t`,
the 19 arithmetic, the 8 logical; a number is the new `y`'s low 31 bits, `0x14033c210` takes
it modulo n). Each of the first three slots (`0x14033b5e0`, one per group 0 to 2) gathers
candidates from its table's rows (`0x140dff518` for the synthesis requests): rows whose +0x3c
(`Table`) is the slot's number (or -1; the rows with `Table` 3 are not placed yet), whose counter in the array at `0x141079fe0` (by +0x4) is at least 6, that pass
`0x1400e31a0`/`0x14033b450`; then, in order, each draws a number modulo 100 and is taken when it
is below `trunc(Priority + 0.5)` (`Priority`, the float at +0x38); if none is, the list is
shuffled (each place k from 1 swapped with `below(k + 1)`) and the first taken
(`quest::BoardRng`, `board_pick`). Every candidate first passes `0x14033b450`
on its row (+0x24 `StartEvent`, +0x28 `EndEvent`, +0x2c `isClearSubject`, +0x30 `StartDate`,
+0x34 `EndDate`): the start event seen and the end event not, each when given; the assignment
cleared (its state in the 0x14-byte records at `0x141099e10` at least 7); and, when either date
is above 0, `StartDate <= today <= EndDate` (an end below 0 meaning none). So the April 1st
draw (day 90) holds none of the requests that start on 91; they first come up on April 11th.
Synthesis candidates also need a count of at least 6 for their target in `0x141079fe0` and its
recipe flagged (`0x1400e31a0`, `0x141079fe0` + 0x8c0); the four battle slots (board places 3 to 6, groups 0 to 3; no row
has `Table` 3, so the fourth takes only the -1 rows) come from the manager's second list (+0x18,
0x40-byte rows), need a count of at least 2 for the monster in `0x141079fe0`, and are sorted by
`0x14035cc80 (target)` from highest to lowest (`0x14033ecc0` with `0x14033c1d0`; the monster's
level, presumably) before the same draw and shuffle (`0x14033eb80` with `0x14033c210`);
the three gathering slots (board places 7 to 9, `0x14033bd10`, groups 0 to 2) come from the
third list (+0x30), skip a target already on an earlier gathering slot and ask for no count
(`quest::board_eligible`, `draw_synthesis`, `draw_battle`, `draw_gathering`, `draw_board`).
The seed offset (`0x1410989f0`) is drawn once, when the board is reset for a new game
(`0x1403644f9`: the main generator's next number, `0x1404ca7a0`, after the slots are cleared),
and kept in the save (`0x1403640d0`), so each playthrough has its own boards. The counts the
synthesis and battle candidates are tested against are a per-item state (an i32 for each of the
560 items at `0x141079fe0`; menus test it against 0, 1, 2 and 10), raised by `0x1403692f0
(item, state, force)` and `0x140369840` (to 2 or 10) and never lowered unless forced; what each
state means is not read yet (the battle rows' +0x4 index it too). `barrel board INSTALL DAY
[--events] [--offset=N]` draws a day's board (no assignment cleared, the item states taken as
high enough): April 1st (day 90) is
empty, and the April 11th board (100, every start event seen) holds three synthesis, three
battle (the fourth battle slot takes only `Table` -1 rows, and none has it) and three gathering
requests. The day ends by clearing
`0x1410782e2` (`calendar::daily_rolls`, `daily_gain`, `add_gain`, `month_start`, `board_day`,
`board_seed`).

### Skill targets (executable)

A skill record's first word (+0x10, `0x14015f900`) says whom it is aimed at. The target
picker (`0x1401abf70`) turns it into a side, comparing with each candidate's side (virtual
+0x10, 0 or 1): 1 and 3 the user's own side, 2 the other side, anything else no side (-1).
Over the 534 skills: 0 on passives (64), 1 on heals and Wind Shell (28), 2 on attacks and
debuffs (most), 3 on Howling-like self-buffs (49), 4 on Lucille's Vital Check (a heal that also
revives: either side, fallen or not, presumably). The next word (+0x14) is 1 on 21
skills, among them the Floating Alchemy bombs (an area, presumably). No code reads it through
the skill table (`0x140d51ed0`, 96-byte records): its ten references read +0x8, +0x10, +0x18,
+0x1c, +0x20, +0x24, +0x58 and the effect list from +0x28 (`0x14015f620`, `0x14015faea`), never
+0x14, so it may be unused in this game; how many targets an action hits has to come from
elsewhere (the action or its effects). The skill action (vtable `0x140808ba8`, its execute in
slot 5, `0x1401bb650`) takes its targets as a list: `0x140189fc0` copies the battle's current
selection (an object at +0xab8 of the battle, a vector of fighter pointers from its +0x1f8 to
+0x200) into a local vector, and every step after that (the turn mark at the fighter's +0x29c,
the hit loop from `0x1401bb956`) walks the whole vector. So one action can hit several
fighters, and how many is decided where that selection is filled. One filler: `0x140189d50`,
when the selection's state (+0x1e8) is 5, has `0x1401a4430` gather fighters into it (+0x1f8)
by a pick code from +0x1ec: 0 and 3 give 0x409, 1 gives 0x40a, anything else 0x408. Both
come from the action (virtual slot 1, for a skill `0x1401bb4d0`, through `0x140189650`): the
mode (+0x1e8) from the skill's target word by a table (0 → 2, 1 → 1, 2 → 0, 3 → 2, 4 → 4, 5
→ 5), the sub-code (+0x1ec) from its effects (0x34-byte entries): 0, or 1 when its only
effect is act 81 (revive), 2 when it revives and does more. So the whole-list pick is mode 5,
target word 5, which no skill has (they use 0 to 4): one-target skills, and the area case
belongs to another action kind (items, presumably: the bombs); the revive sub-codes pick
among the fallen (0x40a) or everyone (0x408).

The action factory (`0x1401b9f70`, kinds by the jump table at `0x1401ba2a8`) builds
`BtlCharaActionSkill` for kinds 0, 3 and 4, `BtlCharaActionItem` for 1, `Move` 5, `Wait` 6,
`Change` 7 and `Escape` 8 (vtables `0x140808b18` to `0x140808c48`). The item action's slot 1
(`0x1401ba830`) works as the skill's: the held item (+0x31c/+0x31e of the fighter) gives a
target word (`0x1402a25a0`: 288-byte item records, the word at `0x140d19c7c` + 288 × row)
mapped by the same table, the sub-code from its effects, forced to 2 when `0x14029e320(item,
0x18)` holds. Over the 274 rows: 226 own side (1), 33 the other side (2: the attack items, one
target in the selection), 6 none (0), 4 either (4), 3 self (3) and 2 the whole-side pick (5,
rows 34 and 38). So bombs pick one foe here; if they spread, it is
decided later, and it is: by distance.

**Areas.** The gatherer (`0x1401a4430(candidates, user, centre, kind, out)`) takes an area
kind from the action (virtual +0x10: a skill's record +0x20, `0x14015f840`; an item's word after
its target word, `0x1402a25e0`). Kind 6 builds a shape from the user towards the centre
(`0x1401bc5c0`, constants at `0x140805500`: ±150 across, 2,000 long) and keeps the candidates
inside it (`0x1401bc700`, their size +0x358 allowed for): a line. Any other kind keeps the
candidates whose distance from the centre on the ground (x and z of +0x230) is less than
their size (+0x358) plus `RADIUS[kind]` (`0x1408041d8`: 50, 350, 450, 650, 900, 1000, 150).
Over the 532 skills: kind 0 (50, the target alone in practice) 353, 1 (350) 6, 2 (450) 117, 3
(650) 3, 5 (1000) 49, 6 (a line) 4. So many skills hit everyone near the target; the battle
loop here keeps one target until fighters have positions (`fight::Fighter::position`, all 0).

Where fighters stand: each keeps a home spot (its model's +0x3d8) that its position (+0x230)
returns to. The battle's setup hands over a list of `{fighter, position}` records (read at
`0x14019ddd0`: each home spot set from the record, the fighter's size +0x35c stored beside it),
so the formation comes from whoever builds the battle (the field encounter, not read yet). The
setup travels as the battle scene's request (the scene factory around `0x1402946b0` makes the
0x1468-byte battle, `0x14019eec0`, from the request's +0x40 and +0x48; the battle keeps the
setup at +0xf50). The setup holds the party as 8-byte entries from +0 to +8, the monsters as
0x18-byte entries from +0x18 to +0x20 (a monster number, then a 16-byte position at +8 whose
second float the fighter's size replaces), a word at +0x30 and a flag at +0x38. Before placing
anyone `0x14019dee0` marks, per monster, whether its number lies in 0x45..0x81. Who fills the
request (the field encounter) is not found yet: no table names battle positions, and the
field area tables (`Saves/field/AREA_*.xml`) hold only camera, fog, light and render settings.
Fighters joining later are scattered by `0x1401a3e7d`: parked at (-100000, 0, 0), then each
after the first put at a random angle (0 to 359°) and a random distance 200 to 400 from the
centre ((sin, 0, cos) × distance, `0x140804bb4` = 200), the first at the centre. Another path (`0x14018b5b6`) hands the same list on to `0x1401c9ad0`
and `0x14018e520` after placing the fighters. `Skill::target` and
`spread` hold them, and the duel puts stat states on the target's side.

### Encounter groups

Who a field enemy's battle brings is a static table in the executable: 226 groups of 0x38
bytes at `0x140754bc0` (English executable; `0x140050340` returns group k, `0x1400501b0`
lists the groups with a given id; `battle_data::encounter_groups`, `barrel encounters EXE`).
As 14 words:

| word | meaning |
|---|---|
| 0 | the group's id (its index) |
| 1 | always 1 |
| 2 | 0, 3 or 4 (3 and 4 only on single bosses), kept by the battle setup at +0x3c (not read yet) |
| 3, 4 | how many monsters, at least and at most (1..2 for most, up to 4..5) |
| 5, 6 / 7, 8 / 9, 10 | three (monster, most of it) slots, -1 and 0 when empty |
| 11 to 13 | other monsters, -1 for none, listed apart in the setup at +0x48 (beside bosses: e.g. the Slayer Titan with Panzer, Howling Beast and Heavy Skin, reinforcements by the look of it; how they join is not read yet) |

**The draw** (`0x14019da40`): a count is rolled from the least to the most (`0x1404ca660`,
`Rng::range`), then the slots are filled in order, each giving its monster up to its own most,
until the count is reached; later slots get only what the earlier ones leave
(`battle::encounter_monsters`). So group 12 (3 or 4; Ore Rats up to 2, then Stray Dogs up to 2)
is always two Ore Rats and one or two Stray Dogs; group 6 is the Rampage Beast alone. Each
monster goes into the battle setup as a 0x18-byte entry `{monster, flag 0, position (0, -1,
0, 0)}`, the position meaning "place it": the battle arranges them itself.

**The battle setup** (0x60 bytes; `0x1400502a0(group)`: `0x14019d6c0` initialises it,
`0x14019d860` adds the party, `0x14019da40` the group): the party as up to six 8-byte entries
`{character, slot}` from the squad (`0x140330660`), the monsters as above from +0x18, the group's
word 2 at +0x3c, its extra monsters from +0x48. The encounter (`0x140084730`) then sets +0x38
from the field enemy (+0x20) and +0x30 from a per-area value, and hands the setup to the scene
request (`0x140291b00`, `0x140291fd0`) that starts the battle. The field's enemy data (`clsFMDataEnemy`, a 0x7030-byte singleton at
`0x140dfc1f0`) holds entries of 0x68 bytes (up to 277, from +0x10) whose word at +0x4c is the
group id (`0x140113470` walks them). **Field strikes.** When the hero swings at a field enemy (`0x140084410`), the game averages
the group's monster levels (each monster's from `0x140111a30`) and the squad's (the eleven
slots at `0x141093530`, each member's level at +4, `0x140330a10`). If the squad is 10 or more
levels above the group, the field is not in a state that forbids it (`0x1402177c0`), the
enemy's kind (+0x24) is not 1 to 5, and an option is on (a global's +0x1e0 == 1), then an
enemy whose +0x20 is 1 is marked (+0x51) to fall to the strike without a battle. The hit plays
the script macro `wandHitEffect` (Escha) or `swordHitEffect` (Logy) by the chosen hero
(`0x14035ebb0`), with the target and that mark (`0x140122170`).

**The formation** (`0x1401a3d00`, after the setup's fighters are made; a monster whose setup y
is below 0 keeps no home spot of its own, `0x14019ddd0`). Everyone is first parked at
(-100000, 0, 0). The monsters (fighter list 7) are sorted largest first (`+0x358`, comparator
`0x1401a4a20`); the first stands at the centre, each other at an angle rolled from 0 to 360
(degrees) and a distance of 200 plus a roll from 0 to 100, times 2: (d sin θ, 0, d cos θ), the
angle rolled before the distance. Each spot then goes through `0x1401b53b0` (a nudge, not read
yet) and becomes the monster's home (+0x3d8). Each monster also gets a sector, its angle / 120
(+0x394; the centre one's angle counts as 180, so sector 1; an angle of 360 gives 3, as the game
has it). The party (list 6) is not scattered: each member's sector is its index (+0x74) mod 3
(+0x398) and its spot comes from the sector (`0x1401b0b40`, `0x1401b0df0`, `0x1401b0e30`):
the game's "three positions around the enemy". `battle::scatter` does the monsters' part.

A member's place (`0x1401b0b90(slot, index)`, `battle::party_spot`): a base offset by its
index, member 0 (0, 0, -800), members 1 and 2 (±200, 0, -700), any other (0, 0, -1200), turned
about the vertical by its slot's angle (`0x1400c2930`, from sin and cos), the slot being its
sector + 3 wrapped to 0..5 and the six slots' angles 60, 180, 300, 240, 0 and 120 degrees (a
jump table at `0x1401b0d20`). So the three in front stand 700 to 800 from the centre and about
120 degrees apart (member 1 straight behind at (200, 0, -700)). `0x1401b0e30` turns each to face
by its slot (six directions, `0x140804f40` and on, into +0x240). The rotation's sense (which
way is "turned") is read from the matrix's layout and wants a look at a real battle.

**The nudge** (`0x1401b53b0`, run on each monster's spot right after its two rolls, so its own
rolls fall between those of the next monster; `battle::nudge`, inside `battle::scatter`): up
to two passes, the second only if the first moved the spot. Each pulls the spot in by however
far it lies outside the arena (`0x14019eab0` on the battle stage, 0 when inside: the spot is
scaled by (length - outside) / length), then, for each other fighter in play (alive and placed:
the unplaced wait at (-100000, 0, 0), out of reach), pushes it straight away from that one by
their overlap (both sizes, `+0x358`, minus their horizontal distance) when positive. Two
fighters on the same spot part at an angle rolled from 0 to 360, rolled even when they do not
overlap. Which of the two library calls there is sine and which cosine (`0x1405cc626`,
`0x1405cc62c`) is taken from their order; it only matters for fighters on the same spot.

The arena (`0x14019eab0`, `battle::arena_outside`) is a circle of radius 500 (`0x140804bb8`)
about the centre: a spot's distance from it past 500, horizontally, else 0. So the scattered
monsters (200 to 400 out, then nudged) stay within 500, while the party stands at 700 to 800,
outside it (the party is not nudged). The skirmish (`skirmish::skirmish`) takes the party
and any number of monsters: each member at its place, the monsters scattered (their sizes are
not read yet, so all alike: they keep their order, the first at the centre), so the monster
rules' nearest and farthest targets (`Battle::pick_target`) go by real distances. The party's
stand-in works down the line (the first monster standing). `barrel battle EXE gN --from SAVE`
fights encounter group N's draw with the save's party: group 12 drew two Ore Rats and two Stray
Dogs against Escha, Katla and Wilbell, who won. An action now reaches everyone its area holds (`Battle::area`, `act_with`): its effects are
rolled once and then applied, in slot order, to each fighter on the centre's side within it
(the centre first), each reported; the action's wait and count once. Kind 0 is the target alone
(no two fighters stand within 50 in the game). Each fighter's size (`Fighter::size`, +0x358)
adds to the reach (and the line's width and length), as in the game; monsters' real sizes are
not read yet, so the skirmish gives each a stand-in 100, the same it spaces them by. Where they
are: a fighter's size is copied, as it is made (`0x1401add2c`, `0x1401b89e1`), from the battle
data singleton (`0x14017cde0`, at `0x141077ed0`, 0x216c88 bytes, its constructor `0x14017abc0`
only setting up empty arrays) at its id × 0x80 + 0x64. What fills those 128-byte records is not
found (no static table; likely per-character battle data loaded with the models), so a short
live run reading them is the quicker way. Items
reach their record's area word (the word after the target, `0x1402a25e0`;
`battle::item_action`'s `area`): Bomb, Ice Bomb, Living Bomb, Dire Bomb and Omega Craft 1 (350),
Craft and the Dimension Bombs 2 (450), the Lightning Bomb 0 (one foe). Over the 265 battle items
(target, area): (1, 0) 214, (2, 2) 15, (2, 1) 9, (1, 2) 7, (2, 0) 5, (2, 5) 3, and a few others.
Against group 3's five Ore Rats a Bomb now hits two at once, for the same rolled damage. The
skills' area word is record +0x20 (`battle_data::Skill::spread`; it had been read from +0x14,
another word).

Still to read: monsters' sizes (above).

### Field enemies

The enemies on the field maps are not in any data file: `clsFMDataEnemy`'s second virtual
(`0x14024a500`) builds them in code, field by field with immediates into a 0x114 × 0x68-byte
table on its stack (some 10,800 instructions of six forms: 32-bit immediates, float constants
or zero, and string pointers), then copies each record into the enemy data (`0x140dfc1f0` +
0x10). `battle_data::field_enemies` rebuilds the table by replaying those stores
(`barrel field-enemies EXE`): 276 enemies. A record:

| at | meaning |
|---|---|
| +0 | the `Chara.xml` row it takes on the map: generic slots "Enemy 1" (418) to "Enemy 3", the bosses' "Enemy 30" (448) |
| +4 | not read yet (66, 63, 44... most often) |
| +8 | a scale, 1 for most |
| +0xc, +0x10 | event records (-1 none): it appears once the first has been seen and until the second has (`0x14035f2d0`, on the event records at `0x1410a42d0`, 36 bytes each, byte 0 "seen", the save's `event` records; `battle::field_enemy_appears`) |
| +0x14 | the area it appears in |
| +0x18 | always 100 |
| +0x1c | its kind: 0 for most, 3 and 5 on some (the bosses 5) |
| +0x20 to +0x34 | floats (speeds and ranges, by the look of them; +0x28 always 6) |
| +0x38, +0x40 | its field AI scripts, `Script/<name>.nut` (`ai_tracking` and `ai_move`, `ai_dog_in2` and `ai_dog_out`, the bosses' `ai_wait`) |
| +0x48 | a float: 180, 220 or 270 most often (an angle of view, perhaps) |
| +0x4c | its encounter group |
| +0x50 | where it stands (x, y, z) |

The field (`0x140072980`) spawns, for its area, each record whose area matches (or is -1), whose
conditions hold and which is not already out or beaten (`0x140075e10`), through `0x14026fc20`
(its +0, +4 and +0x1c). The area is the field map's number (`FIELDMAP_*`: towns 0 to 11, then `AREA_01_01` = 12 and
on), so area 12 is `AREA_01_01`, with Ore Rats (group 10) and Stray Dogs (11) near (40, 0, 40);
area 107's single record is the Ancient Beast King (group 220). `barrel encounter EXE AREA
--from SAVE [N]` lists an area's enemies and fights the Nth's group with the save's party, listing only those out at the save's point in the
story: `AREA_01_01`'s Stray Dog enemy drew three Stray Dogs; in the user's save
`AREA_01_01` has all 3 of its enemies out, `AREA_01_07` none of its 11, `AREA_07_06` 3 of 6.

### A first duel

`barrel_game::fight::Battle` holds a battle in progress (fighters with base and current stats,
side, timed states, action count and slot uses; the timeline) and its steps: `next` (whose
turn), `start_turn` (the netted HP change and the states' ageing) and `act` (a skill on a
target: its poison and stat states, its damage, the next wait, the action counted), reporting
`Event`s. The duel drives it for two fighters. `use_item` acts an item's effects (in a skill's shape) the same way with the item's power (`battle::item_power`) in place of the user's stat 5, as the damage handler does for any action kind but 0 (`0x1401bdf23`); the trait bonuses (flags 3 and 4) and using the item up are not in yet. `barrel_game::skirmish` (`barrel battle EXE MONSTER p<member>:<level>...`) puts party
members (base stats at their level, MP, the skills learnt by then at their listed cost) against
one monster: the monster acts by its rules and target picks, and each member, standing in for
the player, uses the costliest learnt skill it can pay for that hits the foe, or its normal
attack. Escha, Logy and Reyfer at level 20 beat a Howling Beast with Ether Shoot, Flash Cut and
Dragon Breath until their MP runs out; the Beast spreads its hits by the weighted draw and
fells Escha. A member with no affordable heal (or revive) skill uses a healing (or reviving) item it carries instead (`skirmish::Held`: the item as `battle::item_action`, its `item_power` and a count), through `Battle::use_item`. On the command line a member takes items as `p<member>:<level>+<effect id>[,<id>...]@<quality>x<count>` (effect ids are `item_effect` rows, `barrel item-effects`): `barrel battle EXE 2 p0:6 p1:6+167@50x2` has Logy use both HP Recovery S when the Golden Rat brings him low (without an item row the wait is the attack's 600). The battle item table (`battle_data::item_battles`: 265 rows of 288
bytes from `0x140d19c70`) starts each row with the item's id (its `itemData.xml` row;
`0x1400e31a0` finds a row by id), then holds at +0xc, +0x10 and +0x14 the target word, the
area kind (the Bomb: 1, radius 350; the Craft: 2) and its action's wait (`0x1402a2620`: 300 to
900 on the battle items, 0 on the rest; Craft 500, Bomb 550, Healing Salve 500, Clear Jelly
300), which the item action's slot 6 (`0x1401bb310`) runs through the
same speed formula as a skill's wait (`0x1401b9b70`), less stat 19 in percent; `+#<row>:...`
gives an item by id (`p1:6+#234:167@50x2`: the Healing Salve, own side, wait 500, its effect the HP Recovery S row). In place of effect ids, `~<fire>,<water>,<wind>,<earth>` takes the effects the item's recipe lines give at those element values (`recipe::effect_at`), as synthesis would: `p1:6+#234:~0,60,0,0@50x2` is a Healing Salve made at water 60 (its stronger heal; at 10 or 30 the weaker). Traits follow the count as `t<row>,<row>` (`item_potential.xml` rows; `barrel trait-kinds EXE` lists the `ITEM_SUB_*` kinds in enum order, 46 of them): each use acts through `use_item_boosted`, rolling the bonus for each effect (`fight::Boost`); `p1:15+#234:~0,10,0,0@50x3t38` (Extreme Healing, +50) heals about half again as much (21 → 30, 17 → 25). Slot 8 answers
10,000 for any item, where a skill asks the user.

`barrel_formats::battle_data` reads the monster and skill tables from the English
executable, and `barrel_game::duel` puts the pieces above together: two monsters trading the
monsters' normal attack (`SKILL_ATTACK_ENEMY`, `DAMAGE_HP` 48 to 60, wait 600), each turn
advancing the timeline, rolling the amount (and the second value), working the damage
through `skill_damage`, `reduce` and `hit_amount`, drawing the hit sound's number, taking
the HP, and setting the attacker's next wait from its speed. A monster picks each action by
its block's rules ("Monster actions": its own action count stands in for the turn count, the
the latest skill is 0 before any action), the slot naming the skill; only the skill's `DAMAGE_HP` effect is worked, with its element: an
elemental hit skips defence and meets the target's resistance, a physical one the reverse
(Ice Bomb on the Grand Dragon does 41, Bomb 10 against its fire resistance). `DAMAGE_TURN` (act 8, `0x1401bebdc`) is worked too: the
rolled amount grows by the effect's second value in percent for each action the user has
taken (its +0x2c8): `trunc((second × turns + 100) × amount / 100) × scale`, dealt straight through
`0x1401c1970` with no attack, defence or resistance, so Flame Gale (36 to 45, 5 a turn) hits
5% harder each turn (66 at turn 10). A character's +0x2c8 counts its own actions: it is 0 at the start (`0x1401b7515` for
monsters) and goes up once the action is done, after its next wait is set
(`0x1401aecda`), so the rules and `DAMAGE_TURN` see the actions taken before this one. Skills that do
something else deal 0 there. `barrel duel EXE 3 30` has the Alchemy Rat throwing Ice Bomb and
Rolling Rat and the Grand Dragon mixing Jaws of Death and Pressure, then Flame Gale on its
sixth action. `barrel duel EXE A B` runs one
(monster numbers, or `p<member>:<level>` for a party member from "Party growth", with the
member's own normal attack, command 0 of its skill list; fixed seed). A party member fights there with base
stats only, no equipment: Escha at level 5 does 1 damage to a Poison Squirrel.
`--equip "NAME[@QUALITY]"` gives the first fighter an item from `itemData` (its HP, attack,
defense and speed through `equipment::item_stat`, made with no effects or traits): with the Traditional Staff (her
weakest staff, level 1, attack 5; the equip flags such as `PC00` are -1 for "can equip"),
she hits for 4 to 5. It is a check that the pieces fit, not the game's battle: the
start of battle, traits, states, passives, the roll's globals and the action's float are left
out or taken as neutral, so its numbers are not yet the game's.

### Random numbers

The game draws every random number from one Xorshift128 generator (Marsaglia's: `t = x ^ (x
<< 11)`, then `x, y, z = y, z, w` and `w = w ^ (w >> 19) ^ t ^ (t >> 8)`), state `x, y, z, w`
at `0x1410c2fd8`. A number from `a` to `b` (`0x1404ca660`, both included, in either order) is
`low + (w & 0x7fffffff) % (high - low + 1)`, and draws nothing when `a == b`; a number below
`n` (`0x1404ca740`) is `(w & 0x7fffffff) % n` and always draws; `0x1404ca7a0` returns
`w & 0x7fffffff`; a chance of p percent (`0x1404ca5f0`) is `(w & 0x7fffffff) % 100 < p`. The state is not
in the file: `0x1404ca4a0`, run once at startup (from `0x1406e5059`), seeds it from the clock,
`_time64(NULL)`'s low 32 bits as `s[-1]` and each word `1812433253 × (s[i-1] ^ (s[i-1] >> 30))
+ i` (Mersenne Twister's initialisation; `Rng::seeded`). Saves do not hold it either, so a
loaded game draws on from wherever the generator stands since boot. (`0x1404ca510 (n)` draws
from `-n` to `n`.) `barrel_game::rng` is the same generator.

### Applying effects (executable)

The function at `0x1401bde10` (English Escha executable) applies a resolved action's effects
one by one. Each runtime effect entry holds its `ACT_*` number at +0, an integer amount at
+0x20 and a float scale at +0x28 (the power roll from the minimum..maximum happens before this
function, not yet found). A switch on the number (`cmp r14d,0xe9`, byte table `0x1401c0930`,
jump table `0x1401c0840`) sends it to one of 64 handlers:

- one handler for 17 HP-damage kinds (`DAMAGE_HP`, `DAMAGE_REDUCE`, `ASSIST_ATTACK`,
  `DAMAGE_IF_BADSTATUS`, `DAMAGE_TIME_LEFT`, `DAMAGE_RANGE_PROPOTION`, ...), at `0x1401bdf6d`;
- one each for `DAMAGE_FIX`, `DAMAGE_PERCENT`, `DAMAGE_MP`, `HEAL_HP`, `HEAL_MP`,
  `HEAL_HP_BY_ATK`, `RUNAWAY`, `ASSIST_GUARD`, `RETURN_WT`, `PERCENT_QUICK`, `KNOCKBACK`, ...;
- shared ones for the bad states (`BAD_SLEEP`, `BAD_POISON`, ...), their cures, the stat
  drops (`MINUS_ATK/DEF/SPD`), the timed effects (`DAMAGE_HP_TURN`, `PLUS_ATK`, ...), the
  drains and the field effects;
- 105 numbers (`CONSUME_MP`, `MOVE_DASH`, ...) that do nothing here (handled elsewhere).

The instant-death acts (`battle::death_chance`; the user is r13, the target r12 in
`0x1401bde10`): `PERCENT_DEATH` (80, `0x1401bfd59`) hands the death event a chance of the
amount + 5 × (user's level − target's level); `DEATH_PERCENT` (176, `0x1401bfd9f`) a certain
one when the target's HP × 100 / max(max HP, 1) is at most the amount, else none;
`ABSOLUTE_DEATH` (195, `0x1401bfdfe`) builds the event already landed. The event
(`0x1401c1340`, vtable `0x140809170`) misses under state 0x50 or when the target's virtual
+0x40 says so (not read yet), else rolls its chance (`0x1404ca5f0`, `Rng::chance`). A landed
one leaves the target at 0 HP (`fight::Event::Felled`). In `barrel battle` a member with an attack item (damage or death acts, not aimed at its own side) throws it at the monster when no heal is due, before its skills (a stand-in for the player): Logy at level 5 with three Crafts (`p1:5+#195:~10,0,0,0@50x3`) loses to the Flesh Eater (5); with Fatal Blow (`…x3t50`) the first Craft hits for 18 and then fells it (the trait's effect follows the item's own). `fight::Battle` acts as the game does: every effect's amounts rolled first, in slot order (`0x1401bd520`), then each applied in slot order (`0x1401bde10`): poison, stat states, instant death, heals and revives, damage (the first hit drawing for its sound). An item's bonus is rolled in the roll pass for each effect, right after its amounts (`fight::Boost::apply`): on a flag-3 act `item_bonus` asks the traits' sums kind by kind in the game's order (each ask rolls the slots, `battle::kind_sum`), then on a flag-4 act kind 39's.

### Battle stats (executable)

A battle character keeps 35 i32 stats at +0x88 (`0x1401af3c0 (c, k)` reads stat k) and 35 base
values at +0x114. For a monster, `0x1401b70e0` fills the base values from its stat block
(`docs/tables.md`, "Monsters") through the monster getters at `0x1401122d0` on:

| Stat | What | From the monster's stat block |
|---|---|---|
| 0 | level | word 0 |
| 1, 3 | HP, maximum HP | word 1 (both) |
| 2, 4 | MP, maximum MP (presumably) | 0 |
| 5 | attack | word 2 |
| 6 | defence | word 3 |
| 7 | speed | word 4 |
| 8 to 11 | resistance to elements 0 to 3 | words 6 to 9 |
| 12 | not known | word 10 |
| 13 to 34 | 0 for monsters | |

Others are known from where battle reads them: 14 (a percentage the user gains on a target
with bad status 7), 15 (added to effect percent 2), 20 (drain percent). The low-HP test
(`0x1401ae380` k = 4) is HP × 100 / maximum HP at most 25. `barrel_game::battle::stat` names
the indices.

### HP damage (executable)

The shared HP-damage handler (`0x1401bdf6d`) first adjusts the amount for some kinds, among
them `DAMAGE_TIME_LEFT`, `DAMAGE_IF_BADSTATUS`, `DAMAGE_COST_OVER`, `DAMAGE_HP_LEFT`,
`DAMAGE_SPLASH` (amount / number of targets), `DAMAGE_TARGET_LEFT` and
`DAMEGE_TIMECARD_NUM` (not written out yet), then, for a skill (action kind 0):

    attack  = the user's EQUIP_PARAM_LEVEL value + the user's stat 0
    base    = trunc(trunc(amount × user stat 5 / 100) × ((attack^1.5)^1.1 / 50 + 100) / 100)
    defense = target stat 6 / 4      (0 when the effect's +4 is not -1, or for DAMAGE_4ELEMENT)
    damage  = trunc(max(base - defense, base / 5) × scale)

The powers are `powf` in floats; the divisions are integer, truncating. For an item (kind not
0), attack and defense are 0, and the entry's +0x18 takes the place of stat 5. Stats 0, 5 and 6 are
the level, attack and defence ("Battle stats"). Then the target's reductions:

- an elemental effect (its +4 is the element, 0 to 3) is multiplied by `(100 - min(target
  stat 8 + element, 100)) / 100`, so stats 8 to 11 are the resistances;
- a physical effect (+4 is -1) by `(100 - r) / 100`, where r is the target's passive
  `EQUIP_REDUCE_SUFFERDMG` (at most 50) when it has one, else `AUTO_DAMAGE_REDUCE` (at most
  80). The second replaces the first; they do not add;
- a target with passive `PASSIVE_ABSORB_DAMAGE` (asked with the element) turns it into
  `-(damage × value / 100)`, healing;
- when global `0x141077e44` is positive and the target is of type 0, that percentage of the
  damage is taken off and added to global `0x141077e48` (a shared pool, not yet named);
- an action with target mode 2 adds the damage to the user's +0x304 and +0x308 instead of
  dealing it; a later hit by a user with +0x304 positive deals `trunc(+0x304 × the action's
  float +0x20) + 1`, at most what is left in +0x308, and takes that from +0x308 (stored
  damage released over hits, apparently).

A damage of 0 or more is then dealt by `0x1401c1970 (battle, user, target, damage, element)`,
which builds a damage event (vtable `0x140809230`): its amount is the damage less the target's
+0x2b8 percent, at least 1 (and is added to the target's +0x2c4 while +0x2c0 is positive). The
event's apply method (`0x1401c1ac0`) takes it off with `0x1401af770 (target, &amount, 1)`:

1. with passive `PASSIVE_REVERSE_DAMAGE`, that percentage of the amount comes off MP instead
   (`0x1401af970`; MP not under 0, and under `EQUIP_HPTOMP_MPTOHP` that MP loss heals HP by its
   percentage through `0x1401afa20`, which `FIELD_INVALID_HEAL` blocks), the rest off HP;
2. HP goes down by the amount;
3. the first hit of an action (global `0x141077e74`, cleared by `0x1401bd000`) draws a random
   number (`0x1404ca7a0`: the next number's low 31 bits) and plays one of four sounds, 0x1e +
   number % 4. Only a sound, but it draws from the shared generator, so a reimplementation must
   draw too to stay in step;
4. HP is kept at least 0;
5. if HP damage was done and the target lives: a party character (type 0) shakes by amount /
   maximum HP unless option 0x2a35 is set, and with `EQUIP_HPTOMP_MPTOHP` the target gains
   amount × value / 100 MP, up to maximum MP.

Then the event sets the target's +0x2ec and, unless it has bad status 10, calls `0x1401b0520`
(the death check, presumably).
Afterwards, when the user's stat 1 is positive (alive, presumably), `DAMAGE_REDUCE` heals the
user by its second value as a percentage of the damage dealt; any other skill heals the user
by the user's stat 20 percent of it (drain), items do not.
`barrel_game::battle::skill_damage`, `reduce`, `hit_amount` and `take_hp_damage` are the
formulas above.

`HEAL_HP` heals `trunc(amount × scale)` (the amount as a float, times the scale, truncated to
an integer). `HEAL_HP_BY_ATK` (21, `0x1401bedf3`) heals `trunc(trunc(user's attack ×
amount / 100) × scale)`: Lucille's Vital Check (72 to 80) heals about three quarters of her
attack. Both go through the heal event (`0x1401c1d00`), whose apply (`0x1401afa20`) raises HP
by the amount, no higher than the maximum HP, and clears the character's +0x28d; under
`FIELD_INVALID_HEAL` (state 0x4e) a character still standing gets nothing (`battle::heal`,
`heal_by_attack`; `fight::Battle::act` heals before any damage effect). For `ACT_ASSIST_COUNTER` (93) the
amount is first replaced with `user[+0x2c0] × (user[+0x2c4] + 100) / 100`.

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

## Field areas

The `FIELDMAP_AREA_*` enum has 86 tags (`FIELDMAP_AREA_01_01` to `_13_05`); a field's files
are named by its tag without the prefix (`AREA_01_01`). The executable keeps 85 records of
0x58 bytes for them at `0x140d67fb0` (English Escha executable; `0x140172120 (area)` returns
one, -1 none), one per tag in order; `tools/areas.py EXE` lists them.

| Bytes | What |
|---|---|
| 0 | pointer to the developers' Japanese name: an id and two places, "R0(コルセイト-黄昏を迎える森)" |
| 8 | pointer to the short id: "R0" to "R14" for the first 13 and the last 2, "A0r0" to "E2r7" for the rest |
| 16 | i32 1 in all |
| 20 | i32: 46 to 991 (shared by an R record and the first of its letter group: R0 and A0r0 both 46) |
| 24, 28 | i32 -1, or small numbers in a few |
| 32 | i32 -1, or 181 in one |
| 36 | f32: 1.5 in most; 1.0 to 3.5 among the R records |
| 40 to 60 | six i32: on the R records three pairs (2 to 7, 688 to 692) and a 0 or 1; 0 and -1 on the others |
| 64 to 87 | three pointers, 0 in the file: filled at run time (+0x48 and +0x50 point to the two places' objects) |

So the R records join two places on the world map (Colseit and "the forest that welcomes
twilight" for R0), and the float is most likely the days the trip takes; the lettered
records (A0 the forest's, A1 the old road's, ...) join two parts of one field ("A0R1: the
forest entrance to the path to the settlement"). `0x14037f650` (which takes an area number) confirms the joining: it takes the two place
objects from +0x48 and +0x50, and looks the short id up by name (`0x1404f14d0`) in the map
object, the route's path. The float's meaning is still a reading from the values; no
reader of +0x24 has been found yet. Record 85, past the
table, is something else (pointers to "Knowledge: nothing", "Knowledge: new", ...).

### Places

The places the area records join are a table of 96 records of 0x60 bytes at `0x140d65bb0`
(English Escha executable; `0x1401717e0` finds one by its id). `tools/areas.py --places EXE`
lists them.

| Bytes | What |
|---|---|
| 0 | pointer to the name in the executable's language ("Colseit", "Twilight Forest") |
| 16 | i32: 0 for the 16 world-map places, 1 for the 80 places inside fields |
| 24 | pointer to the id: "T0" (the town), "A0" to "G2" (fields), "B1p4" (a place in field B1) |
| 32 | six i32: the area records that start or end here, -1 for none |
| 56 | pointer to the description ("A withering forest.") |
| 72 | two i32 (-1 and 0 in most) |

The links agree with the area names: Colseit's are areas 0 to 3 (R0 to R3, the four routes
whose names start at Colseit), the Upland Forest's are R3, R7, R13 and R6. One link is out of
the area range: the Withering Plains' 208, still to be explained.

### Gathering (not found yet)

What each field's gathering points give is in none of the data files: not in
`Saves/field/AREA_*.xml` (camera, fog, light, sky, HDR and shader settings only), not in the
area containers (`Data/x64/Field/AREA_*/AREA_*.elixir.gz` and `_Obj`: terrain, textures,
placements, collision), whose placement lists hold scenery models only, and not in
`Script/` (36 Squirrel scripts for monster behaviour, `ai_*_in.nut` and `_out.nut`). Like
the recipes and monsters, it is most likely compiled into the executable.

In the executable, field objects that do things are "gimmicks", of the types in the
`FIELDMAP_GIMMICK_TYPE_*` enum: `ITEM_D`, `ITEM_D_MULTIPLE`, `ITEM_D_MULTIPLE_PROGRAM` and
`ITEM_D_MULTIPLE_RARE_PROGRAM` (gathering points, by the names), `ITEM_BOX`, `ITEM_TRADE`,
`ROCK`, `TARU` (barrels), `BREAKABLE_OBJECT`, `WATER`, `WELL_WATER`, `APPLE`, `SHOP`, `MIX`,
`POSTER`, `CALENDAR`, `BUDGET`, `SAVE_AND_LOAD`, `WARP`, `TRAIN`, the map changes
(`FIELDMAP_CHANGE`, `WORLDMAP_CHANGE`, `IN_CHANGE`, ...), messages and events. Twelve
gimmicks have their own tags, named by area and event (`FIELDMAP_GIMMICK_ENUM_AREA_03_07_MS01_070`).
Where the gimmicks of each area are listed is still to be found.

## Party growth

The party's base stats by level are compiled into the executable too: at `0x140d8f370`
(English Escha executable), right after the `PARTY_*` enum's names, 11 blocks of 100 rows of
five i32, one block per party member, each 99 levels (1 to 99) and a row of zeros. A row is
HP, MP, attack, defence, speed, the battle stats 1 (and 3), 2 (and 4), 5, 6 and 7.
`barrel_formats::battle_data::party_growth` reads them; `barrel party EXE` prints them.

The blocks are in the `PARTY_*` order (Escha, Logy, Awin, Threia, Reyfer, Lucille, Linca,
Wilbell, Micie, Katla, Nio): the getter `0x14032ce70 (member, level, k)` reads
`table[(member × 100 + level - 1) × 5 + k - 2]` for a party member number up to 10, a level
up to 99 and k from 2 to 6 (HP to speed), and `0x14032d840` fills a party member's record
(member number at +0, level at +4) with the five at +0x14 to +0x24 before `0x14032de50`
works out the rest (equipment, presumably). The level test lets 0 through, which would read
the row before the block; the game presumably never asks for it. Awin has the most HP (461
at level 99), Threia the most MP (290), Lucille is the fastest (208); Escha starts at 18 HP,
27 MP, 4 attack, 3 defence, 6 speed.

### A party member's stats (executable)

A party member's record keeps three arrays of 32 stats: the base (+0x14, from the growth table
above), a second block (+0x94) and the stats in use (+0x114). `0x14032de50` works them out as
below; last, every stat is kept within the limits of a table at `0x140935180` (999 for most):
at least 1 for the first five, at least minus the limit for the others (`equipment::in_use`
does not clamp yet).

**The stats in use** (`0x14032de50`, run whenever a character's equipment or bonuses
change): for each of the 32 stats from max HP on, the value in use (+0x114 of the character's
record) is the base (+0x14) plus a second block (+0x94), plus what each of the four equipped
items gives (`0x14029dda0` per item and stat, items at +0x194, 0x24 bytes each), and, for the
protagonist alone (the record's character equal to `global` +0x38, `0x14035ebb0`), bonus
value 4 + k for stat k (the `Add_Status_*` bonuses and the 27 after them, "Bonuses"). One save
bears it out: Escha (the protagonist there) has max HP 30 at base and 35 in use, with bonus
value 4 at 5 and no HP from equipment; Logy, with no bonus, has the same max HP in both. The
saved record keeps the in-use block, so battles started from a save (`barrel battle ... --from`)
already include the bonuses.

What an equipped item gives (`0x14029dda0 (item, stat, base)`): for the first nine stats and
an item whose row's +0x28 is 2 (equipment), the item row's own value for that stat (+0x78 +
4 × stat); plus, for each of the item's four effects, the amount of each of the effect's
slots whose act is one the stat takes (`0x1402a20f0`: a roll between the slot's min and max,
`0x1404ca660`, which is the value itself when they are equal); that sum times
`(quality × 0.5 + 75) / 100` (the same scale as an item's battle power), truncated. Then, unless
the item's +0xe is set, each of its three traits adds the amounts of its slots whose kind the
stat takes by a second list (`0x1402b4770`, not read yet; kind 0x2c is a percentage of `base`,
the stat's base value). The acts each stat takes (`0x1402b4560`):

| stat | acts |
|---|---|
| 0 to 4 (max HP, max MP, attack, defense, speed) | `ACT_EQUIP_ADD_PARAM_HP`, `_MP`, `_ATK`, `_DEF`, `_SPD` |
| 5 to 9 | `ACT_EQUIP_ADD_RESIST` (special-cased: by its element, presumably) |
| 10 | `ACT_PASSIVE_CRITICAL`, `ACT_EQUIP_ADD_CRITICAL` |
| 11, 12 | `ACT_EQUIP_ADD_CRITBONUS`; `ACT_EQUIP_ADD_POWER` |
| 13 | `ACT_PASSIVE_CHANGE_SUPPORTGAUGE`, `ACT_ADD_SUPGAUGE` |
| 15, 16, 17 | `ACT_AUTO_DODGE`; `ACT_PASSIVE_REDUCE_WT_SKILL`; `ACT_EQUIP_ADD_DMGCONVERT` |
| 18 to 20 | `ACT_EQUIP_ADD_FIRE_DMG`, `_ICE_DMG`, `_THUN_DMG` |
| 22, 23 | `ACT_EQUIP_GUTS_POWER`; `ACT_EQUIP_ADD_PERCENT` |
| 24 to 28 | `ACT_BAD_POISON`, `ACT_BAD_SLEEP`, `ACT_BAD_BLIND`, `ACT_BAD_CURSE`, `ACT_BAD_SLOW` (resistances) |
| 29, 30, 31 | `ACT_EQUIP_BONUS_EXP`; `ACT_EQUIP_BONUS_MONEY`; `ACT_PASSIVE_LUCKY_STAR`, `ACT_EQUIP_ADD_DROPRATE` |
| 14, 21 | none |

The last three agree with the bonuses: `Add_Exp_Rate`, `Add_Money_Rate` and `Add_Drop_Rate`
are bonus values 33 to 35, stat 29 to 31 + 4.

The traits each stat takes (`0x1402b4770`, `ITEM_SUB_*`): stats 0 to 4 their own
`EQI_ADD_MAXHP`, `_MAXMP`, `_ATTACK`, `_DEFENSE`, `_SPEED`, plus `EQI_ADD_ALLPARAM` and
`EVOLVE_STATUS` (a percentage of the base); 5 to 9 `RESIST_FIRE`, `_WATER`, `_WIND`, `_EARTH`,
`_BADSTATUS`; 12 `EQI_ADD_SKILLPOWER`; 14 `EQI_REDUCE_SKILLCOST`; every stat
`BTL_ADD_USEEFFECT` (an effect, its row in the slot's lower bound, counted by the effect
rule). An `ACT_EQUIP_ADD_RESIST` slot names its element in its first range (stat − 5) and
the amount in its second. Stats 5 to 8 are the element resistances (the item rows'
`RES_FIRE` .. `RES_EARTH`), 9 the ailment resistance.

`equipment::in_use` ports the refresh up to the bonuses, and `BARREL_EXE=... barrel save`
rebuilds every character's block and compares: in one save all 11 characters' max HP, max
MP, attack, defense, speed and resistances come out exactly (the equipped items sit at +0x19c
of the saved `Party` record, after base +0x18, a second block +0x98 and the block in use
+0x118). After the bonuses come the passive skills (`0x14032e080`): each skill on the character's
list (the party skill lists, `0x140d96f90`) learnt by its level byte at or below the
character's level, whose skill record has target word 0 (the passives), adds its two effect
slots (record +0x28, 0x18 apart) to every stat whose act list has the slot's act (a roll
between the slot's bounds); an `ACT_EQUIP_ADD_RESIST` slot adds its second range to
resistance 5 + its first, and since stats 5 to 9 all take that act it is added five times
(kept as the game does it). `equipment::passives`. With them every one of the 32 stats of all
11 characters in the save read comes out as saved. Two more passes follow, which nothing
in that save uses: `0x14032e580` sums `ACT_EQUIP_PARAM_LEVEL_PROPOTION` (0xa8) over the third
and fourth equipped items (`0x14029e7e0`) and adds level × it × the base attack, defense and
speed / 1000 to those three; `0x14032e670` asks the same items for act 0x99
(`ACT_PASSIVE_CHANGE_HP_MP`) and only answers whether any has it.

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
| 5 | not known yet (1 or 2; not a battle stat) |
| 6..9 | the four element resistances |
| 10 | battle stat 12, not known yet |
| 12..19 | four pointers (below) |

The block's **second pointer** (words 14, 15) is its skill list: 16-byte records
`{i32 skill, i32 0, pointer to the event that plays it}`, ended by skill -1. The Poison
Squirrel has Running Headbutt (`skill/Skill_MONSTER_FRUITRAT_01_0.ebd`) and Poison Fruit
(`_4.ebd`); the Alchemy Rat, Running Headbutt, Bomb, Ice Bomb, Lightning Bomb and Rolling Rat;
the Grand Dragon, five (`skill/Skill_MONSTER_BOSS_DRAGON_01_0..4.ebd`). `barrel monsters EXE`
lists every monster's blocks with their skills (185 blocks).

### Monster actions

The **first pointer** (words 12, 13) leads to the block's action set: `{pointer to the
rules, i32 slot[9]}`. Each slot names an index into the block's skill list (the getter
`0x140112c70` takes slots 0 to 8). The rules are 32-byte records, ended by one whose first
word is -1:

| Word | What |
|---|---|
| 0 | a kind (2 or 3; the chooser does not read it) |
| 1 | the target, handed on with the slot |
| 2, 3 | the condition and its value |
| 4 | the chance in percent |
| 5 | the slot it picks |
| 6 | how many times the slot may be used in the battle (-1: no limit) |
| 7 | 0 |

The chooser (`0x1401a5290`, from the monster's turn at `0x1401b72xx`) tries the rules in
order and takes the first whose condition holds (`0x1401a5360`), whose slot has been used
fewer than `limit` times (the use counts are the monster's +0x3e8 per slot, raised as it
acts), and whose `chance(percent)` roll succeeds (one roll per rule that gets that far).
The conditions: 0 always; 1 every `value`-th action of its own (its count of actions taken,
the character's +0x2c8, modulo `value` is `value` - 1); 2 HP at or below `value`% of the maximum
(`max HP × value / 100 >= HP`, battle stats 3 and 1); 3 the monster's latest action used skill `value` of its list: +0x2d0, set as each action starts (`0x1401ba3c2`, `0x1401d6320`, `0x1401d6dfa`) from virtual +0x18 of the current action (`0x1401aeec0`, the first in the list at +0x1f0; +0x2cc gets the action's kind from virtual +0x20: 3 a skill, 1, 5 to 8 other kinds), which for a skill action (`0x1401b9f70` kinds 0, 3, 4) is the value the monster's turn passed: `0x140112c70(monster, block, slot)`, the slot's entry in the block's slot table. Bosses chain skills with it (Frigate: Charge once, then Bunker Quake after it; 191 such rules, values 0 to 8). Before the first action it is taken as 0 (the constructor's value is not read). If no rule gives an action the game logs an error and takes slot 0 at target 0.
The rule's target is a pick mode (`0x14018a2f0`, among the fighters in the battle's target
list at `battle+0xab8`, +0x38 to +0x40, filled for the skill's side by `0x140189b00`): 1 the
nearest to the monster (positions at the model's +0x230), 2 the farthest, 3 the lowest share
of its maximum HP (stats 1 / 3 in floats), each keeping the first on ties; 4 the character the
monster remembers (+0x314, through `0x1401b26a0`) and 5 the one in global 0x141077e3c, each
when it is in the list; otherwise, and for 0, a weighted draw (`0x14018a090`): each candidate
weighs `max(virtual +0x128 + 40, 1)` and one number (`0x1404ca7a0`, always drawn) modulo the
total falls on the first whose running total passes it (`fight::Battle::pick_target`).
`barrel_game::monster_ai::choose` does the same, and `barrel monsters EXE` prints each
block's rules: the Poison Squirrel uses Running Headbutt 40% of the time and Poison Fruit
40% (at most twice); the Grand Dragon opens with slot 5 once, uses Flame Gale every sixth turn
and has once-only actions at 75% and 50% HP. Its slots 5 to 8 read 0 (the skill list's first
skill) because its slot list names only five; whether slots past the list mean something
else is not checked.

The fourth pointer is set only for some bosses (the Grand
Dragon's: `2, 9, 48, 40, 1, -1, ...`), not read yet.

The **drop list** starts with up to four `{item, chance in percent, count}` triples, then
entries of another shape (skills, probably). Some drops have numbers above the item enum's
568; what those are is still open.

| Monster | Level | HP | ATK | DEF | SPD | Drops |
|---|---|---|---|---|---|---|
| Poison Squirrel | 6 | 35 | 15 | 8 | 13 | Apple 50%, Acorn 35%, Poison Apple 25%, Blue Nuts 10% |
| Golden Rat | 30 | 351 | 52 | 27 | 37 | Golden Apple 50%, ... |
| Altegewalt | 60 | 35,000 | 329 | 187 | 139 | resistances 40, and 75 in its second block |

## Curves

`Saves/curve/curve_data.xml` holds 72 keyed curves, `curve_obj_000` to `curve_obj_071`,
used by number (`0x1404e1770` in the English executable indexes the loaded list; `s26`'s
last operand is one). Each `CurveData` has one `CURVE_ENVELOPE_LINKER_DATA` (`initData`,
`num`, `bLoop`, `offset`) and 16 `CURVE_ENVELOPE_DATA` slots `{point, frame, liner}`, the
first `num` used (the rest have frame -1). Frames run 0 to 60; a user stretches them to its
own length (frame × seconds / 60, `0x1402034c0`).

The value at a time (`0x1404e1230`) comes from the segment it falls in, by the segment's
first key's `liner`: 1 straight, 2 hold, 0 smooth, a cubic spline (`0x1404e0440` solves it,
`0x1404e0850` evaluates it with the usual b, c = 3σ, d form) through the segment's two keys
plus the key before when that one is smooth and the key after next when the next is smooth.
With only two keys in all, or only two points to use, it is straight. The spline is natural
(σ, the second derivative over 6, is 0 at both ends). With four points the solver puts 0 in
the third row's right-hand side instead of the slope difference (`subss xmm1, xmm1` at
`0x1404e0581`: the slope array has just been reused for the diagonal), so those curves bend
differently from a textbook spline; Barrel keeps the game's arithmetic. Past the last key the value is the last key's.
`curve_obj_001` (0, 0.757 at 30, 1) eases out, `_003` (0, 0.203, 1) eases in, `_000` and
`_002` are straight, `_004` rises to 1 at frame 30 and falls back. `barrel_game::curve`
reads and evaluates them.

## Motion phases (`CharaDiv.xml`, `SubCharaDiv.xml`)

`Saves/chara/CharaDiv.xml` (2,281 rows) cuts a character's motions into phases: each row is
`{BaseTag, MotionTag, Div0, Div1, ...}`, the tag a `CharaBase` motion column (`JUMP`,
`ATTACK00`, `GUARD`, `B_RUN`, `MIX_POT`, ...) and each `DivN` a `start, end, loop` triple in
seconds (end -1: to the motion's end; loop 1: the phase repeats until something ends it).
Escha's `JUMP` is `0,0.1,0` `0.1,0.6,0` `0.61,1.1,0` `1.1,1.5,1` `1.5,-1,0`: take-off, rise, fall,
a held landing loop, recovery. `SubCharaDiv.xml` does the same with twelve slots for event
motions (`EV_ACT_1` ...). What reads them (the field, battle and synthesis motion players)
is not traced yet. These are not `s116`'s segments, which are fixed frame ranges in the code
(`docs/events.md`).

Field NPCs are not in the field tables: `Saves/field/<FIELD>.xml` holds the field's camera,
fog, light, wind and shader settings, and the field's `.opl2` only its objects. Where a
town's own NPCs (character numbers 10048 and up, which events put in event mode with `q7`)
come from is still open.

## Deliveries (`Saves/delivery/DeliveryData.xml`)

71 rows (25 `DELIVERY_TEST_*` the developers' own), one per hand-in an assignment card or a
character's request asks for (`DELIVERY_CARD_<card>_<n>`, `DELIVERY_QC<..>_<who>_<n>`,
`DELIVERY_MM<..>`). Each row: `DeliveryTag`, `SubjectName` (the card's subject, Japanese),
`PassScore`, up to three `ItemTag<n>`/`ItemScore<n>` (the items asked for and what each is
worth), and optional bonuses: `CategoryTag` (an item category, no score of its own),
`AtVal`/`AtScore` (an effect level at least `AtVal`, 20 rows), `Effect`/`EffScore` (5),
`Quality`/`QualityScore` (3) and `Fire`/`Water`/`Wind`/`Earth` (4, element values). The pass
scores run from 100 to 3,000 (`DELIVERY_CARD_08_00`: a key item worth 650 plus food of effect
level 1 or more at 100 each). How a hand-in's score adds up (per item, per bonus, capped or
not) is not read from the executable yet. `barrel_game::delivery` reads the table and
`barrel deliveries INSTALL` lists the rows.

The progress is saved as `delivery/delivery_data`: 71 pairs `{state, score}` at
`0x141098160`, one per row. State 2 is a delivery passed (the script function at
`0x140354dc0` counts those); the score is what has been handed in so far, and the progress
shown is `min(score, pass) × 10000 / max(pass, 1)` (`0x140354d10`, the pass score read from
the loaded rows, 0x78 bytes each, +0x20). One save has row 25 passed with 100, and rows 0, 26,
27 and 66 to 70 at state 1 with no score (taken, presumably). `save::Summary::deliveries`.

## Attached props (`Saves/chara/CharaAddObj.xml`): unused

71 rows `{BaseTag, GroupNo, Index, Node, Directory, File}`: a prop model hung on a named bone
(`H_4_WepAttach` 26, `H_4_WepAAttach` 12, `H_4_LetterAttach`, `H_4_BookAttach`,
`H_4_PenAttach`, `H_4_TobaccoAttach` ...), switched by `s66` (`ECT_CHARA_CHANGE_ADD_OBJ`, one
use in all the events). It covers only `PC00` to `PC06` and four of `PC12` to `PC16`, not
Escha (`PC20`) or Logy (`PC21`), and none of its files (`PC00WEAPON01`, `PC00BOOK` ...) are in
the install: a table kept from the series' previous game. In Escha & Logy the weapons and
props are part models in the character's own container (`<stem>_P_M_Wep_*`, `_Opt_*`),
switched by `CharaPartsChange.xml`; accessories (`_P_M_Acc_*`) are always worn
(`docs/renderer.md`, "Accessories").

## Bonuses

`bonus/Bonus.xml` (245 rows; `Saves_EN` has the English names) is what completing
assignments grants: an assignment's `Bonus` names a row. A row has a `Name`, a `Text` and up to
five actions, `BonusEntryN` (a `BonusEntry_*` name) with `EntryNPrm1` to `Prm3`. Once loaded
each row is 0xb0 bytes (behind `0x140df9998`) with the five actions at +0x20, 16 bytes each:
the kind, then the three parameters resolved to numbers. `0x14002f310(row)` runs them, a
switch over 55 kinds in the order of the executable's `BonusEntry_*` names
(`bonus::KINDS`):

| kinds | what they do |
|---|---|
| 0 `GetMoney` | money (`0x14036a8e0`) |
| 1 to 3 `Event_Tag_On`, `Event_Flag_On`, `Event_Flag_Add` | event state (`0x14035f480`, `f660`, `f6b0`) |
| 4 `Get_Trophy` | a Steam achievement (`0x140168390`); no row uses it |
| 5 `Add_Basket`, 6 `Add_Work_Point` | basket size (`0x14029f3b0`), work points (`0x14035fc90`) |
| 7 to 9 | bonus value 1 (adventure experience %), 2 (adventure level limit: read as 50 + it, capped at 99), 3 (synthesis experience %) |
| 10 `Get_Recipe`, 11 `Get_Mix_Skill`, 12 `Get_Mix_Bonus` | recipe book level (`0x140369560`), synthesis skill (`0x14036c470`), synthesis bonus (`0x14036c400`, three parameters) |
| 13 `Add_Status_ALL` | values 4 to 8 at once (from fields past the actions; no row uses it) |
| 14 to 45 | bonus value kind − 10: HP, MP, attack, defense, speed (4 to 8), the four element and the condition resistances, corrections, critical rates, the element damages, death avoidance, the five ailments, experience, money and drop rates |
| 46 `Add_Item_Slot` | `0x1403654f0` |
| 47 `Reduce_Collect_Time` | adds Prm2 to gathering slot Prm1 of the 17 (`bonus/collectTimeBonus`), or to all 17 when Prm1 is -1 |
| 48 `Reduce_Move_Time` | nothing (falls through) |
| 49 to 51 | values 39 (move fatigue), 40 (sell price %), 41 (buy cost) |
| 52 `GetGift`, 53 `Get_Explore_Equip`, 54 `Get_Costume` | gifts (`0x14002f950`), explore equipment (`0x14002fad0`), costumes (`0x140017000`) |

After each action the hero's record is refreshed, and HP and MP gains raise the current HP
and MP as well. All 332 actions in the table resolve. Taken together the rows add 100% to
both experience rates, 49 to the level limit (so 99), 50 HP, 40 MP, 15 to each other stat,
and 4 to every gathering slot. `barrel bonuses Bonus.xml` lists them; `bonus::apply` runs the
value actions and hands the rest back to their systems.
