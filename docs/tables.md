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

`barrel_game::shop` reads the wares (`barrel shops INSTALL [LANG]` lists them). How `Refill`,
`RefillCnt` and `DayAdd` restock a shop over the days is not read yet: the loader keeps the
rows (0x1e8 bytes each, the integers at the schema's offsets) in a vector, but the code that
uses them has not been found.

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
is rolled as its kind is asked for, in that order. With a bonus b or an extra
(`0x1401bdbcc`): b is cut to the act's bonus limit, and the amount becomes `trunc(amount ×
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
attack (what fills it, and the quality bonus of flag 3, are not read yet)
(`battle_data::item_effects`, `barrel item-effects EXE`).

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
1165 records of 0x24 bytes from `0x1410a42e0` each get a roll from 0 to 100 (each a draw from
the random generator), then every character but one (`0x140330120 (0x141098a48)`, the
leader presumably) adds to its record's float +0x2a4 (records of 0x2b0 bytes at
`0x1410bcb60`) a gain by its place in the 11-slot list at `0x141093530`: 0.11 in slots 0 to 2
(or not in the list), 0.08 in 3 to 5, 0.03 below, kept at 100 at most (friendship points,
presumably). On a month's first day (date divisible by 30) `0x140363860` and `0x140267f50`
(the manager at `0x140dfc220` +0x120) run. Then, for each 0x34-byte record of a list from
`0x140168990`, one whose `0x140168710` (a float) is 0 or less and that is not yet flagged
(+0x31) is flagged and handed to `0x140268290 (manager +0x120, its i16 +2)` (expiring
requests, presumably). Last the shop rotation (`0x14033aaa0` on `0x140dfc880`): on the 1st,
11th and 21st of each month (day of month mod 10 is 1) global `0x1410989e8` is cleared and
`0x14033ab20` redraws ten 0x34-byte slots at `0x1410985e0` (three from one list, four from
another with stock `min(n / 2, 10)`, at least 1, three from a third, through `0x14033b5e0`,
`0x14033b990` and `0x14033bd10`) with a generator of its own, seeded from the date plus global
`0x1410989f0` by the Mersenne Twister seeding step: `s0 = (x ^ (x >> 30)) × 0x6c078965 + 1`
with `x` that sum, `s1` the same of `s0` plus 2 (the shift arithmetic). The rotation's generator is the object's own two words (`0x140dfc880`), stepped like the
battle's Xorshift on two words (`t = x ^ (x << 11)`, `x = y`, `y = y ^ (y >> 19) ^ (t >> 8) ^ t`,
the 19 arithmetic, the 8 logical; a number is the new `y`'s low 31 bits, `0x14033c210` takes
it modulo n). Each of the first three slots (`0x14033b5e0`, one per group 0 to 2) gathers
candidates from the list at `0x140dff518` (built by `0x14017c1e0`): entries whose +0x3c is the
group (or -1), whose counter in the array at `0x141079fe0` (by +0x4) is at least 6, that pass
`0x1400e31a0`/`0x14033b450`; then, in order, each draws a number modulo 100 and is taken when it
is below `trunc(chance + 0.5)` (the chance is the float at +0x38); if none is, the list is
shuffled (each place k from 1 swapped with `below(k + 1)`) and the first taken
(`shop::ShopRng`, `rotation_pick`; what the entries are is not read yet). The day ends by clearing
`0x1410782e2` (`calendar::daily_rolls`, `daily_gain`, `add_gain`, `month_start`, `shop_day`,
`shop_seed`).

### Skill targets (executable)

A skill record's first word (+0x10, `0x14015f900`) says whom it is aimed at. The target
picker (`0x1401abf70`) turns it into a side, comparing with each candidate's side (virtual
+0x10, 0 or 1): 1 and 3 the user's own side, 2 the other side, anything else no side (-1).
Over the 534 skills: 0 on passives (64), 1 on heals and Wind Shell (28), 2 on attacks and
debuffs (most), 3 on Howling-like self-buffs (49), 4 on Lucille's Vital Check (a heal that also
revives: either side, fallen or not, presumably). The next word (+0x14) is 1 on 21
skills, among them the Floating Alchemy bombs (an area, presumably; not read). `Skill::target` and
`spread` hold them, and the duel puts stat states on the target's side.

### A first duel

`barrel_game::fight::Battle` holds a battle in progress (fighters with base and current stats,
side, timed states, action count and slot uses; the timeline) and its steps: `next` (whose
turn), `start_turn` (the netted HP change and the states' ageing) and `act` (a skill on a
target: its poison and stat states, its damage, the next wait, the action counted), reporting
`Event`s. The duel drives it for two fighters. `barrel_game::skirmish` (`barrel battle EXE MONSTER p<member>:<level>...`) puts party
members (base stats at their level, MP, the skills learnt by then at their listed cost) against
one monster: the monster acts by its rules and target picks, and each member, standing in for
the player, uses the costliest learnt skill it can pay for that hits the foe, or its normal
attack. Escha, Logy and Reyfer at level 20 beat a Howling Beast with Ether Shoot, Flash Cut and
Dragon Breath until their MP runs out; the Beast spreads its hits by the weighted draw and
fells Escha.

`barrel_formats::battle_data` reads the monster and skill tables from the English
executable, and `barrel_game::duel` puts the pieces above together: two monsters trading the
monsters' normal attack (`SKILL_ATTACK_ENEMY`, `DAMAGE_HP` 48 to 60, wait 600), each turn
advancing the timeline, rolling the amount (and the second value), working the damage
through `skill_damage`, `reduce` and `hit_amount`, drawing the hit sound's number, taking
the HP, and setting the attacker's next wait from its speed. A monster picks each action by
its block's rules ("Monster actions": its own action count stands in for the turn count, the
phase is 0), the slot naming the skill; only the skill's `DAMAGE_HP` effect is worked, with its element: an
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
`--equip "NAME[@QUALITY]"` gives the first fighter an item from `itemData` (its HP, ATK, DEF and
SPD columns through `equipment_stat`, traits not counted; which item column feeds which battle
stat is taken by name, not yet from the executable's array): with the Traditional Staff (her
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
in the file, and the code that seeds it is still to be found. `barrel_game::rng` is the same
generator.

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
above), a bonus (+0x94) and the total the battle uses (+0x114). `0x14032de50` works the total
out:

1. total = base + bonus, stat by stat;
2. for each of the 4 equipment slots (stride 0x24 from +0x194; the item number at +0x196,
   0xffff for none), every stat gets `0x14029dda0 (slot, stat, base)` added. For the first
   nine stats that starts with the item's own value (the item table's 0x1c0-byte rows,
   `row + 0x78 + 4 × stat` when the row's kind at +0x28 is 2) plus what each of the slot's 4
   traits (+0x10) adds (`0x1402a20f0`), scaled by quality (the float at slot +4):
   `trunc((item + traits) × (quality × 0.5 + 75) / 100)`, so quality 50 gives the item's value
   and 100 gives 125%. More shares follow from the slot's entries at +8, indexes into the
   400 potentials (`0x58`-byte rows), not read yet;
3. for one party member (the one `0x14035ebb0` names), `0x14002f2f0 (stat + 4)` is added to
   each stat (a party-wide bonus, by the look of it);
4. three more passes (`0x14032e080`, `0x14032e580`, `0x14032e670`), not read yet;
5. every stat is kept within the limits of a table at `0x140935180` (999 for most): at least 1
   for the first five, at least minus the limit for the others.

`barrel_game::battle::equipment_stat` is step 2's formula.

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
(`max HP × value / 100 >= HP`, battle stats 3 and 1); 3 the battle's phase (+0x2d0) is
`value`. If no rule gives an action the game logs an error and takes slot 0 at target 0.
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
