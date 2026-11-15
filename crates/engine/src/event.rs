//! The event player: runs an event's bytecode and says what happens, step by step.
//!
//! Action 0 is the event's main script; its `w1 N` starts action N alongside it (the game runs
//! them at once; here a started action's steps come right where it starts). Each command
//! becomes a [`Step`]. It knows `u18` (0x75000012), which shows the event's messages
//! from its first operand to its second, inclusive, and the waits `p1` and `u1`.
//! Everything else is reported by name, with its operands, for the systems that will
//! handle it.
//!
//! The command words here (the constants below, [`Step`]'s sources) are Escha & Logy DX's, the
//! first game Barrel read: they are the engine's vocabulary. Another game's events are carried
//! over to them by command name ([`Player::translated`]); what a game alone has stays flagged.

use std::collections::HashMap;

use barrel_formats::ebd::{self, Ebd, OperandTable};
use barrel_formats::ebm::Message;
use barrel_formats::Error;

/// `u18`: show messages first..=last.
pub const SHOW_MESSAGES: u32 = 0x7500_0012;
/// `p1` (`ECT_MACRO_TIME_WAIT`): wait, the operand in hundredths of a second.
pub const TIME_WAIT: u32 = 0x7000_0001;
/// `u1` (0x75000001): wait 0.3 seconds (the event player's constant).
pub const SHORT_WAIT: u32 = 0x7500_0001;
/// `u15` (0x7500000f): wait 2 seconds (`0x140051412`, the constant at `0x14071c964`).
pub const LONG_WAIT: u32 = 0x7500_000f;
/// `u36` (0x75000024, `{event, seen}`): marks an event seen or not (`0x14035f480`: its record's
/// flag; when seen, its count goes up and the day is stamped; `save::EventRecord`). The
/// openings mark the event that has just played (`EVENT_*` number, 241 for example).
pub const MARK_EVENT: u32 = 0x7500_0024;
/// `u34` (0x75000022, `{flag, value}`): sets one of the 151 event flags (`0x14035f660`: the
/// words at `0x1410932d0`, saved as `event/m_eventFlag`).
pub const SET_EVENT_FLAG: u32 = 0x7500_0022;
/// `u35` (0x75000023, `{flag, amount}`): adds to an event flag (`0x14035f6b0`, case
/// `0x140051516`). Like `u34` it then runs the event checks with trigger 7 and the flag.
pub const ADD_EVENT_FLAG: u32 = 0x7500_0023;
/// `u29` (0x7500001d, `{mode, 0, 0}`): the game goes to another mode when the event lets go
/// (case `0x14005171e`: a `GAMEMODE_CREATE_DATA`, `0x140291b70`, of that mode handed to the
/// event's interface, `0x140292eb0`): 14 the world map (most), 0 the field map, 4 the title
/// (after an ending). Names in [`GAME_MODES`].
pub const CHANGE_GAME_MODE: u32 = 0x7500_001d;
/// The game modes (`GAME_MODE_TYPE_INFO_*`, in enum order): `u29` and `u46` (13, the movie)
/// name them by number.
pub const GAME_MODES: [&str; 75] = [
    "FIELDMAP", "BATTLE", "BUC", "BALLOON_BUC", "TITLE", "BALLOON_SELECT", "HELP", "MONEY_GP", "OPTION", "LIBRARY",
    "EVENT_TELOP", "SKIP_ICON", "CLEAR_SAVE", "MOVIE", "WORLDMAP", "MESSAGE_ITEM_GP", "MESSAGE_LOG", "EVENT_SKIP", "INSTALL",
    "GAMEOVER", "SAVELOAD_STEAM", "MENU", "INPUT_COUNT", "CALENDAR_MENU", "SHOP", "HOM", "WORK", "RANKUP", "NPC_DELIVERY",
    "MIX", "LOG_VIEWER", "DISCOVERY", "QUEST_STOCK", "SAVELOAD", "EVENT_ITEM_SELECT", "EXTRA", "MIX_WEAPON", "EXTRA_EVENT",
    "MIX_MELT", "CALENDAR_GP", "THROW_CONTAINER", "ITEM_GHOST", "RECLAIM_MENU", "DLCBGM", "PLANBOARD", "VIEWMODE", "SEED_LETTER",
    "ITEM_FAIR", "MEMORIES_NOTE", "MASTER_SELECT", "MARION_MENU", "MONTHLY_REPORT", "QUEST", "STDSAVELOAD", "ENDING_REPORT",
    "COSTUME", "EVENT_INFOMATION", "DEBUG_EVENT_FLAG", "DEBUG_EVENT_CHECK", "DEBUG_USER_DATA", "DEBUG_PARTY", "DEBUG_ITEM",
    "DEBUG_KNOWLEDGE", "DEBUG_WORLDMAP_ROUTE", "DEBUG_WORLDMAP_POINTER", "DEBUG_LIBRARY", "DEBUG_EXTRA", "DEBUG_RECLAIM_SEED",
    "DEBUG_RECLAIM_TARGET", "DEBUG_FACILITY", "DEBUG_NPC_DELIVERY", "DEBUG_DELIVERY", "DEBUG_BONUS_ENTRY", "DEBUG_MOTION_LOG",
    "DEBUG_EXPLORE_EQUIP",
];
/// The planning board's game mode (`PLANBOARD`), opened by `u107`.
pub const PLANBOARD_MODE: u32 = 44;
/// `u107` (0x7500006b, `{n}`): opens the planning board (game mode [`PLANBOARD_MODE`], with
/// `n` at the request's +0xd0; case `0x1400515cc`, `0x140292240`), at the start of each main
/// quest period (`EVENT_MM03_MQ1_020` with 12, `EVENT_MQ02_010` to `MQ09_010` with 62, 112,
/// ... 412: 50 apart).
pub const OPEN_PLANBOARD: u32 = 0x7500_006b;
/// `u134` (0x75000086, `{group}`): which of the event's process data groups its end runs
/// (the event's +0x60, case `0x140051dbb`; read at `ECT_END`, `0x140052c76`, which runs that
/// group of `EventProcessDataAll.epd` through `0x14005a070`). 1 unless set
/// ([`DEFAULT_END_GROUP`]); `EVENT_MM12_ST3_031` to `_036` choose among five, `EVENT_MS06_080`
/// between two.
pub const SET_END_GROUP: u32 = 0x7500_0086;
/// The process data group an event's end runs when its script does not choose (the
/// constructors' 1 at +0x60).
pub const DEFAULT_END_GROUP: usize = 1;
/// `u38` (0x75000026, `{trophy}`): unlocks a Steam achievement once (`0x140168390`: the
/// `TROPHY_*` table at `0x140d5fcf8`, 0x198 bytes a row, through the user-stats interface's
/// `SetAchievement` and `StoreStats`). Barrel leaves it out on purpose while it is being built
/// and tested (it never touches the player's Steam account); a finished engine may offer it as
/// an option.
pub const TROPHY: u32 = 0x7500_0026;
/// `u46` (0x7500002e, `{movie, 1, 0}`): plays movie `movie` (the game's movie mode, 13, through
/// a `GAMEMODE_CREATE_DATA`, case `0x140051a96`; the table at `0x140d8ece0`, `docs/events.md`,
/// "Movies"), then `u47` and `u48` follow it.
pub const PLAY_MOVIE: u32 = 0x7500_002e;
/// `t2/0x30` (`ECT_VARIINDEX_INDEX_SETTING`, `{slot, row}`): binds a variable character slot
/// (10028 ...) to a `Chara.xml` row (most often the position markers 590 to 603); commands then
/// name the character by either (6,736 uses, 34 slots, 54 rows over the events).
pub const SLOT_SET: u32 = 0x7430_0002;
/// `t1/0x30` (`ECT_VARIINDEX_INDEX_START`): starts the bindings (no operands).
pub const SLOT_START: u32 = 0x7430_0001;
/// `p3` (`ECT_MACRO_CHARA_WAIT`): wait for a character (its number).
pub const CHARA_WAIT: u32 = 0x7000_0003;
/// `s3` (`ECT_CHARA_TIME_WAIT`): a character waits, `{character, hundredths}`.
pub const CHARA_TIME_WAIT: u32 = 0x7300_0003;
/// `s4` (`ECT_CHARA_POS_SET`): `{character, 0, 0, 0, x, y, z}`.
pub const CHARA_POS_SET: u32 = 0x7300_0004;
/// `s10` (`ECT_CHARA_MOTION_SET_LOOP`): `{character, motion, blend (hundredths), flag}`.
pub const CHARA_MOTION_LOOP: u32 = 0x7300_000a;
/// `s11` (`ECT_CHARA_MOTION_SET_AUTO_LOOP`): `{character, motion, division, flag, blend
/// (hundredths)}`; the runner's +0x5c is set when division equals flag (not read further).
pub const CHARA_MOTION_AUTO_LOOP: u32 = 0x7300_000b;
/// `s13` (`ECT_CHARA_MOTION_SET_OFFSET_ONLY`): `{character, division, flag}`: the motion
/// playing moves to another of its divisions, from that division's start, without a blend
/// (`0x1401f6830`).
pub const CHARA_MOTION_DIVISION: u32 = 0x7300_000d;
/// `s6` (`ECT_CHARA_MOTION_WAIT`): `{character}`: the action waits for the character's motion
/// (or division) to play out, when it does not loop.
pub const CHARA_MOTION_WAIT: u32 = 0x7300_0006;

/// How long a character's motion (a division of it, when given) plays and whether it loops:
/// what [`Player::schedule`] needs for `s6`. The player knows no clips; whoever does supplies it.
pub type MotionLength<'a> = Box<dyn Fn(u32, u32, Option<u32>) -> Option<(f32, bool)> + 'a>;
/// `q4` (`ECT_FIELDMAP_CHARA_ADDITION_EX`): `{character, 0, 0, 0, x, y, z, 0, angle, ...}` (14
/// operands): adds the character there, facing `angle` degrees.
pub const FIELD_CHARA_ADD: u32 = 0x7100_0004;
/// `q2` (`ECT_FIELDMAP_CHANGE`) and `q12` (`ECT_FIELDMAP_CHANGE_NOT_BGM`): `{map}`.
pub const FIELD_CHANGE: u32 = 0x7100_0002;
pub const FIELD_CHANGE_KEEP_BGM: u32 = 0x7100_000c;
/// `q5` (`ECT_FIELDMAP_CHARA_DELETE`): `{character}`.
pub const FIELD_CHARA_DELETE: u32 = 0x7100_0005;
/// `q6` (`ECT_FIELDMAP_CHARA_DELETE_ALL`): every field character is removed (an action usually
/// starts with it before placing the scene's cast).
pub const FIELD_CHARA_DELETE_ALL: u32 = 0x7100_0006;
/// `q8` (`ECT_FIELDMAP_CHARA_COLLISION_FLAG`): `{character, solid, flag, ...}`: whether the
/// field character's body is in the physics world (`0x14026c570`, +0x128) and a second flag
/// (`0x14026c5d0`, +0x129, not read further); case `0x140071348`.
pub const FIELD_CHARA_COLLISION: u32 = 0x7100_0008;
/// `q15` (`ECT_FIELDMAP_MAPVISIBLE_SET`): `{version, shown, ...}`: shows or hides the field
/// objects tagged with that version (`xAM_balloon_ver<N>`, the airship dock in TOWN_07: Logy's
/// opening shows version 2 and hides the dock's version 3 parts). Read from the oracle frames,
/// not the handler (not found yet).
pub const FIELD_MAP_VISIBLE: u32 = 0x7100_000f;
/// `q10` (`ECT_FIELDMAP_EVENT_OBJECT_VISIBLE_SET`) and `q36`, one case (`0x14028a7a6`):
/// `{shown}`: the field objects whose names hold `xEV_` (event-only props: TOWN_03's
/// `obj-xEV_door_r`) shown or hidden (`0x140291860` with the string `xEV_`).
pub const FIELD_EVENT_OBJECTS: u32 = 0x7100_000a;
pub const FIELD_EVENT_OBJECTS_2: u32 = 0x7100_0024;
/// `s5` (`ECT_CHARA_MOTION_SET`): `{character, motion, variant, blend ×100}`.
pub const CHARA_MOTION_SET: u32 = 0x7300_0005;
/// `u31` (`ECT_EVENT_VOICE_PLAY`): `{clip}`, a voice clip number (`docs/formats.md`, "Voices").
pub const EVENT_VOICE_PLAY: u32 = 0x7500_001f;
/// `s102` (`ECT_CHARA_VOICE_PLAY`): `{character, clip}`.
pub const CHARA_VOICE_PLAY: u32 = 0x7300_0066;
/// `u13` (`ECT_EVENT_SET_POST_EFFECT`): `{?, param, hundredths of a second, kind}`; the kinds
/// are in `docs/events.md`, "Screen effects" (3: a cross-fade from the screen as it was).
pub const EVENT_SET_POST_EFFECT: u32 = 0x7500_000d;
/// `u14` (`ECT_EVENT_WAIT_POST_EFFECT`): the action waits for the screen effect to end.
pub const EVENT_WAIT_POST_EFFECT: u32 = 0x7500_000e;
/// `t14` (`ECT_CAMERA_QUAKE_SET`): `{quake, seconds × 100}`, camera quake `cq_obj_<quake>`
/// (`barrel_formats::quake`; `docs/events.md`, "Camera quakes").
pub const CAMERA_QUAKE_SET: u32 = 0x7400_000e;
/// `t16` (`ECT_CAMERA_QUAKE_STOP`): ends the quake.
pub const CAMERA_QUAKE_STOP: u32 = 0x7400_0010;
/// `t10` (`ECT_CAMERA_ROT_MOVE_SET`, the manual camera's): `{from, to, seconds ×100, curve}`:
/// the camera rolls (about its line of sight) from `from` to `to` degrees along a curve.
pub const CAMERA_ROT_MOVE_SET: u32 = 0x7400_000a;
/// `t12` (`ECT_CAMERA_LENGTH_MOVE_SET`): `{length, seconds ×100, curve}`: the eye slides along
/// its line to the target, from where it is to `length` field units away, along a curve.
pub const CAMERA_LENGTH_MOVE_SET: u32 = 0x7400_000c;
/// `u3 {n}`: label `n` in its action (`0x14020fcd6` on: an action's labels are found by
/// scanning its words when it starts; also `u73` when the global at `0x141077ef0` is set).
pub const LABEL: u32 = 0x7500_0003;
/// `u8/0x40`, `u9/0x40`, `u10/0x40` (134, 3,628 and 3,628 uses, no operands): the event
/// handler (`0x1400513b7` and on) does nothing for them and its ready check (`0x140050d30`)
/// passes them at once, so they only mark the script (`u9`/`u10` bracket each setup action).
pub const MARKERS: [u32; 3] = [0x7540_0008, 0x7540_0009, 0x7540_000a];
/// `u5 {n}`: the action goes on from label `n` (`0x140210be0` → `0x14020fd80`, which sets the
/// running action's next command, +0x8, to the label's).
pub const GOTO: u32 = 0x7500_0005;
/// `u6 {a, b}`: the action goes on from label `a + b`.
pub const GOTO_SUM: u32 = 0x7500_0006;
/// `u7 {n}`: goes on from label [`LABEL_BASE`] + `n` (case `0x14005145a`).
pub const GOTO_FROM_BASE: u32 = 0x7500_0007;
/// The base `u7` counts from: the event's +0x5c, which both constructors set to 1 (from the
/// 16 bytes `(-1, 1, 1, 0)` at `0x1407581f0` for +0x58 to +0x64) and nothing changes after.
pub const LABEL_BASE: u32 = 1;
/// `u8` (`{first, v}`): goes on from label `first` + variable `v` (case `0x14005147a`).
pub const GOTO_VAR: u32 = 0x7500_0008;
/// `u9` (`{first, v, c}`): from label `first + 1` when variable `v` is above `c`, else `first`.
pub const GOTO_ABOVE: u32 = 0x7500_0009;
/// `u10` (`{first, v, c}`): from label `first` when variable `v` is `c`, else `first + 1`.
pub const GOTO_EQUAL: u32 = 0x7500_000a;
/// `u22` (`ECT_EVENT_CONTRAST_SET_MIX`): `{contrast id, preset, hundredths of a second}`: the
/// screen's colour overlay moves from the preset's first colour to its second over the time
/// (`docs/events.md`, "Fades").
pub const EVENT_CONTRAST_SET_MIX: u32 = 0x7500_0016;
/// `q42` to `q57`: the field's lights for a scene, each `_SET` with an `_ORG` that puts the
/// field's own back (`docs/events.md`, "Lighting").
pub const FIELDMAP_LIGHT_PARALLEL_SET: u32 = 0x7100_002a;
pub const FIELDMAP_LIGHT_PARALLEL_ORG: u32 = 0x7100_002b;
pub const FIELDMAP_LIGHT_AMBIENT_SET: u32 = 0x7100_002c;
pub const FIELDMAP_LIGHT_AMBIENT_ORG: u32 = 0x7100_002d;
pub const FIELDMAP_CHARA_LIGHT_COLOR_SET: u32 = 0x7100_0032;
pub const FIELDMAP_CHARA_LIGHT_COLOR_ORG: u32 = 0x7100_0033;
pub const FIELDMAP_CHARA_RIM_LIGHT_COLOR_SET: u32 = 0x7100_0034;
pub const FIELDMAP_CHARA_RIM_LIGHT_COLOR_ORG: u32 = 0x7100_0035;
pub const FIELDMAP_POINT_LIGHT_SET: u32 = 0x7100_0036;
pub const FIELDMAP_POINT_LIGHT_ORG: u32 = 0x7100_0037;
pub const FIELDMAP_CHARA_POINT_LIGHT_STRENGTH_SET: u32 = 0x7100_0038;
pub const FIELDMAP_CHARA_POINT_LIGHT_STRENGTH_ORG: u32 = 0x7100_0039;

/// `q54`'s point light as a field look's `POINT` (`0x14028b1c9`): the place in field units,
/// the colour and the ambient in 128ths, the constant and linear attenuation in thousandths,
/// and the range in field units. The quadratic term comes from `fall` (thousandths): chosen so
/// that, the linear term left out, the light is down to `fall` of itself at the range
/// (1 / (att0 + att2 · range²) = fall).
pub fn point_light(pos: [u32; 3], color: [u32; 3], ambient: [u32; 3], att: [u32; 3], range: u32) -> barrel_formats::field_look::Light {
    let i = |v: u32| v as i32 as f32;
    let c = |v: [u32; 3]| [i(v[0]) / 128.0, i(v[1]) / 128.0, i(v[2]) / 128.0, 1.0];
    let range = i(range) / 100.0;
    barrel_formats::field_look::Light::Point {
        pos: [i(pos[0]) / 100.0, i(pos[1]) / 100.0, i(pos[2]) / 100.0],
        color: c(color),
        ambient: c(ambient),
        att: barrel_formats::field_look::point_attenuation(i(att[0]) / 1000.0, i(att[1]) / 1000.0, i(att[2]) / 1000.0, range),
        range,
    }
}

/// A change to the field's lights (`q42` to `q57`); `None` puts the field's own value back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightChange {
    /// Sun light `light` (0 or 1): its colour and its direction, (0, 0, 1) turned about x by
    /// `pitch` and then about y by `yaw` (degrees), the field look's own convention.
    Parallel { light: u32, set: Option<([f32; 3], f32, f32)> },
    Ambient(Option<[f32; 3]>),
    /// Point light `light` (0 to 3), as a field look's `POINT` (metres).
    Point { light: u32, set: Option<barrel_formats::field_look::Light> },
    /// The characters' `atCharaLightColor`.
    CharaColor(Option<[f32; 4]>),
    /// The characters' `atCharaRimLightColor`.
    RimColor(Option<[f32; 4]>),
    /// `atCharaPointLightStrength` (the original's _ORG sets 0.3, its usual value).
    PointStrength(Option<f32>),
}
/// `u55` (`EventTelop`): `{card}`: shows narration card `card` (the voiced lines of the
/// openings, the ending), layout id `0x140d85ce0[card]` (960 + ...; `docs/events.md`, "Telops").
pub const EVENT_TELOP: u32 = 0x7500_0037;
/// `u1/0x30` (`IpuGallery`): `{picture}`: a still picture over the scene
/// (`barrel_escha::data::layout::still`; 0 and 1 show none).
pub const STILL_SHOW: u32 = 0x7530_0001;
/// `u2/0x30`: `{picture, hundredths}`: the still changes to `picture` fading over the time
/// (`0x14029b2f0`, where `u1/0x30` calls `0x14029b1f0`).
pub const STILL_FADE: u32 = 0x7530_0002;
/// `u56`: `{from, to, hundredths}`: the card's opacity (hundredths) moves from `from` to `to`.
pub const EVENT_TELOP_ALPHA: u32 = 0x7500_0038;
/// `u97` (`EventTelop`, case `0x14021972f`): `{part, on, hundredths}`: shows (`on` non-zero:
/// `0x140370f10`, from clear to the part's own opacity over the time) or hides
/// (`0x140370fc0`) piece `part` of the telop's layout (its +0x280 node's children). All 65
/// uses show pieces 0 to 9 over 0.5 s.
pub const EVENT_TELOP_PART: u32 = 0x7500_0061;
/// `ECT_SOUND_SE_PLAY` (`u1/0x10`, 5 uses): `{effect, a, volume %, pan ×100, d}`, taken to
/// play as `u8/0x10` does (its own handler is not read).
pub const SOUND_SE_PLAY: u32 = 0x7510_0001;
/// `u8/0x10` (234 uses, the events' sound effects): `{effect, a, volume %, pan ×100, d}`
/// (`0x14005210c`): the volume and pan (0 left, 64 centre, 128 right) are divided by 100, `a`
/// and `d` (also /100) go to settings not named yet (`0x140160270`, `0x140160210`); the effect
/// is a number for `0x140160070` (`barrel_audio::Library::effect`), its voice kept in a ring of
/// ten (the event's +0xb4) for stopping later.
pub const SOUND_EFFECT: u32 = 0x7510_0008;
/// `s62` (`ECT_CHARA_SE_PLAY`): `{character, effect, a, volume %, pan ×100, d}` from a character
/// (18 uses, all in battle skill events, where the effect looks like another numbering: 151
/// would be the rain loop; its handler is not read).
pub const CHARA_SE_PLAY: u32 = 0x7300_003e;
/// `s116`: `{character, segment, rate, weight}` (the last two in hundredths): a head gesture,
/// one segment of the character's `M_HEAD` clip (`docs/events.md`, "`s116` and `s117`").
pub const CHARA_HEAD_GESTURE: u32 = 0x7300_0074;
/// `s117`: `{character}`: the head gesture ends.
pub const CHARA_HEAD_STOP: u32 = 0x7300_0075;

/// The head gesture segments' frame ranges at 30 frames a second (`0x140037a10`, tables at
/// `0x140726730` and `0x140726750`).
pub const HEAD_SEGMENTS: [(f32, f32); 8] =
    [(0.0, 39.0), (54.0, 88.0), (103.0, 137.0), (147.0, 181.0), (192.0, 223.0), (237.0, 267.0), (282.0, 312.0), (327.0, 360.0)];
/// `ECT_SOUND_BG_PLAY`: `{track, volume (percent), 0, 0}`; the track is a number in the
/// executable's music list (`barrel_escha::data::bgm`).
pub const SOUND_BG_PLAY: u32 = 0x7510_0003;
/// `ECT_SOUND_BG_STOP`: `{0}`.
pub const SOUND_BG_STOP: u32 = 0x7510_0004;
/// `ECT_SOUND_BG_FADEOUT`: `{hundredths of a second, 0}`.
pub const SOUND_BG_FADEOUT: u32 = 0x7510_0005;
/// `s23` (`ECT_CHARA_ROT_SET`): `{character, x, y, z}`, degrees.
pub const CHARA_ROT_SET: u32 = 0x7300_0017;
/// `s24` (`ECT_CHARA_ROT_SET_TARGET`): `{character, position (6)}`.
pub const CHARA_ROT_SET_TARGET: u32 = 0x7300_0018;
/// `s84` (no name; the facing-only sibling of `s85`, `ECT_CHARA_ROT_SET_COPY_TARGET_ALL`):
/// `{character, target (6)}`, the character takes the target's facing (`0x1401f0a93`: the
/// target's rotation from `0x1402f1640`, its y copied into +0x244).
pub const CHARA_ROT_COPY_TARGET: u32 = 0x7300_0054;
/// `s85` (`ECT_CHARA_ROT_SET_COPY_TARGET_ALL`, `0x1401f0ad1`): the same, all three angles. The
/// stage keeps only the facing, so it plays as `s84` (the other two are ignored, as for `s23`).
pub const CHARA_ROT_COPY_TARGET_ALL: u32 = 0x7300_0055;
/// `s26` (`ECT_CHARA_ROT_MOVE_SET`): `{character, x, y, z, seconds ×100, curve}`.
pub const CHARA_ROT_MOVE: u32 = 0x7300_001a;
/// `s29` (`ECT_CHARA_ROT_MOVE_WAIT`): `{character}`: waits for its timed turn (`s26`) to end.
pub const CHARA_ROT_MOVE_WAIT: u32 = 0x7300_001d;
/// `s21` (`ECT_CHARA_MOVE_PRE_WAIT`): `{character, hundredths}`: waits until the character's
/// move has that long or less left (the mover's +0x30 against its time left, +0x28;
/// `0x1402f20d0`, checked in `0x1402f2894`).
pub const CHARA_MOVE_PRE_WAIT: u32 = 0x7300_0015;
/// `s18` (`ECT_CHARA_MOVE_SET`): `{character, position (6), extra, type, ...}`.
pub const CHARA_MOVE: u32 = 0x7300_0012;
/// `w1` (0x77000001): start action N (its operand) alongside the one running. Action 0 is the
/// event's main script; it starts the others, each once, in order.
pub const START_ACTION: u32 = 0x7700_0001;
/// The old name of [`START_ACTION`], from when it was taken for a step marker.
pub const STEP: u32 = START_ACTION;
/// `w4` (0x77000004, `{n}`): starts action N as `w1` does, but only when the event is watched
/// again from the Extra menu (`ExtraEventExec` sets its runner's +0x69, `0x140050cb0`) or a
/// debug switch (`0x141077ef0`, never set) is on (`0x1400523cc`): the set-up a replay needs
/// that play would have given. 206 uses.
pub const REPLAY_START_ACTION: u32 = 0x7700_0004;

#[derive(Clone, Debug, PartialEq)]
pub enum Step<'a> {
    /// Action `action` starts here (`w1`), running alongside the action that started it; its
    /// steps follow, tagged with its number.
    StartAction { action: usize },
    /// A line of dialogue (or system text) to show.
    /// `expression` is the speaker's face from this line on and `gesture` a body gesture to
    /// play, when the line sets them (the message's fourth and third fields; -1 leaves them).
    /// `name` is the name the window shows (the message's second field: an entry of the
    /// executable's speaker-name table, `barrel_escha::data::layout::speaker_names`; "???" before a
    /// character is introduced).
    Message { index: usize, speaker: i32, name: Option<i32>, text: String, expression: Option<i32>, gesture: Option<i32> },
    /// Wait this many seconds before going on (`0x14005b310` sets the event's timer, which
    /// counts down by each frame's elapsed time).
    Wait { seconds: f32 },
    /// `u36`: an event marked seen (or unseen), by its `EVENT_*` number.
    MarkEvent { event: u32, seen: bool },
    /// `u34`: an event flag set.
    EventFlag { flag: u32, value: i32 },
    /// `u35`: an amount added to an event flag.
    EventFlagAdd { flag: u32, amount: i32 },
    /// `u134`: the process data group the event's end runs.
    EndGroup { group: u32 },
    /// `u29`: the game goes to this mode (`GAME_MODES`) after the event.
    GameMode { mode: u32 },
    /// `u107`: the planning board opens with `first` (its first entry, presumably).
    Planboard { first: u32 },
    /// `u38`: the game would unlock this Steam achievement here (Barrel does not).
    Trophy { trophy: u32 },
    /// A movie plays (`u46`; not drawn yet: `docs/engine.md`, "Movies").
    Movie { movie: u32 },
    /// Wait until a character (by `Chara.xml` row) is ready (`p3`); what "ready" means is
    /// still open (`docs/events.md`, "Waits").
    WaitForCharacter { character: u32 },
    /// A character pauses its own commands (`s3`) while the event goes on.
    CharacterWait { action: usize, character: u32, seconds: f32 },
    /// A character is put at a point in field units (`s4`).
    Place { action: usize, character: u32, position: [f32; 3] },
    /// A character is put at an offset from another (`s4` with kind 8, `{character, 8, of, 1,
    /// x, y, z}`; Ryza's events use it, Escha's never do): the offset turned by `of`'s facing
    /// (z along it), added to `of`'s position.
    PlaceNear { action: usize, character: u32, of: u32, offset: [f32; 3] },
    /// A character is put at a position anchored to a thing (`s4` with anchor kind 1, 5 or 6,
    /// as the battle skill events place their actors: `{character, kind, id, point, x, y, z}`,
    /// the offset turned by the anchor's facing). Only with [`Player::anchored_places`].
    PlaceAt { action: usize, character: u32, at: crate::camera::Position },
    /// A character plays a looping motion (`s10`; a `MOTION_*` number), blending over `blend`
    /// seconds.
    /// `s10` loops the whole clip (`0x1402036d0` with -1, -1 clears the play range); `s5`
    /// (`ECT_CHARA_MOTION_SET`, `{character, motion, division, blend}`) and `s11`
    /// (`ECT_CHARA_MOTION_SET_AUTO_LOOP`, `{character, motion, division, flag, blend}`) play
    /// one of its divisions (`barrel_formats::chara_div`), which says whether it loops.
    Motion { action: usize, character: u32, motion: u32, blend: f32, looping: bool, division: Option<u32> },
    /// The motion a character plays moves to another division (`s13`).
    MotionDivision { action: usize, character: u32, division: u32 },
    /// The event moves to a field map (`q12`, `ECT_FIELDMAP_CHANGE_NOT_BGM`, `{map}`; `q2`,
    /// `ECT_FIELDMAP_CHANGE`, likewise): `map` is a `FIELDMAP_*` number ([`field_maps`]).
    FieldChange { action: usize, map: u32 },
    /// A field character is removed (`q5`, `ECT_FIELDMAP_CHARA_DELETE`, `{character}`).
    Remove { action: usize, character: u32 },
    /// Every field character is removed (`q6`).
    RemoveAll { action: usize },
    /// A field character's collision (`q8`): solid or not, and the second flag.
    Collision { action: usize, character: u32, solid: bool, flag: bool },
    /// The field objects of a version shown or hidden ([`FIELD_MAP_VISIBLE`]).
    MapVisible { action: usize, version: u32, shown: bool },
    /// The `xEV_` field objects shown or hidden ([`FIELD_EVENT_OBJECTS`]).
    EventObjects { action: usize, shown: bool },
    /// A camera quake starts ([`CAMERA_QUAKE_SET`]) for `seconds`.
    Quake { action: usize, quake: u32, seconds: f32 },
    /// The camera rolls ([`CAMERA_ROT_MOVE_SET`]; degrees).
    CameraRoll { action: usize, from: f32, to: f32, seconds: f32, curve: u32 },
    /// The eye slides to `length` from the target ([`CAMERA_LENGTH_MOVE_SET`]).
    CameraDistance { action: usize, length: f32, seconds: f32, curve: u32 },
    /// The camera quake ends ([`CAMERA_QUAKE_STOP`]).
    QuakeStop { action: usize },
    /// A voice clip plays (`u31` `{clip}`, or `s102` `{character, clip}` from a character).
    Voice { action: usize, character: Option<u32>, clip: u32 },
    /// A screen effect starts (`u13`): `kind` (`docs/events.md`, "Screen effects"), its
    /// parameter, over `seconds`.
    ScreenEffect { action: usize, kind: u32, param: i32, seconds: f32 },
    /// The screen's colour overlay starts a fade (`u22`): preset `preset` over `seconds`.
    Fade { action: usize, preset: u32, seconds: f32 },
    /// The field's lights change (`q42` to `q57`).
    Light { action: usize, change: LightChange },
    /// A head gesture starts (`s116`): `segment` of [`HEAD_SEGMENTS`] played at `rate` (1 is
    /// real time; the events' 999 jumps to its end and holds it), laid over the body at
    /// `weight`. A segment outside the table ends the gesture.
    HeadGesture { action: usize, character: u32, segment: i32, rate: f32, weight: f32 },
    /// The head gesture ends (`s117`).
    HeadStop { action: usize, character: u32 },
    /// A narration card is chosen (`u55`).
    Telop { action: usize, card: u32 },
    /// A still picture is shown over the scene (`u1/0x30`), or fades in over `seconds`
    /// (`u2/0x30`).
    Still { action: usize, picture: u32, seconds: f32 },
    /// The narration card's opacity moves from `from` to `to` (0 to 1) over `seconds` (`u56`).
    TelopAlpha { action: usize, from: f32, to: f32, seconds: f32 },
    /// A piece of the narration card's layout shown (fading in over `seconds`) or hidden
    /// (`u97`).
    TelopPart { action: usize, part: u32, shown: bool, seconds: f32 },
    /// A sound effect plays (`ECT_SOUND_SE_PLAY`, or `s62` from a character) at `volume`
    /// (0 to 1) and `pan` (-1 left to 1 right).
    Effect { action: usize, character: Option<u32>, effect: u32, volume: f32, pan: f32 },
    /// The music changes: a track starts at `volume` percent, or (`track` None) stops, fading
    /// over `fade` seconds (0 at once).
    Music { action: usize, track: Option<u32>, volume: f32, fade: f32 },
    /// The event camera's eye or target is set or starts moving (`t3`, `t4`, `t6`, `t7`,
    /// `t9`; [`crate::camera`]).
    Camera { action: usize, part: crate::camera::Part, change: crate::camera::Change },
    /// The field-map camera changes (`t3/0x10` to `t20/0x10`, [`crate::field_camera`]).
    FieldCamera { action: usize, change: crate::field_camera::Change },
    /// A character's rotation is set (`s23`, `ECT_CHARA_ROT_SET`, `{character, x, y, z}` in
    /// degrees, here in radians): `rotation[1]` is the facing about the vertical axis, 0
    /// towards +z (the runner's +0x240, the same angle a move's facing writes).
    Turn { action: usize, character: u32, rotation: [f32; 3] },
    /// A character turns to a rotation over `seconds` (`s26`, `ECT_CHARA_ROT_MOVE_SET`,
    /// `{character, x, y, z (degrees), seconds ×100, curve}`; `0x1401f42e0`): each angle goes
    /// the short way round, along curve number `curve` (`0x1404e1770`; its shape is not read).
    TurnOver { action: usize, character: u32, rotation: [f32; 3], seconds: f32, curve: u32 },
    /// A character turns to face a position, on the ground (`s24`,
    /// `ECT_CHARA_ROT_SET_TARGET`; `0x1401f4f50`: yaw from the direction, 0 when the position
    /// is where the character stands).
    Face { action: usize, character: u32, toward: crate::camera::Position },
    /// A character takes the facing of what `from` is anchored to (`s84`).
    CopyFacing { action: usize, character: u32, from: crate::camera::Position },
    /// A character starts moving (`s18`, `ECT_CHARA_MOVE_SET`; [`crate::camera::Move`]).
    CharacterMove { action: usize, character: u32, extra: u32, movement: crate::camera::Move },
    /// A command the player does not handle itself.
    Command { action: usize, word: u32, name: Option<&'static str>, operands: &'a [u32] },
}

pub struct Player<'a> {
    names: HashMap<u32, &'static str>,
    /// Each action's commands.
    actions: Vec<Vec<ebd::Command<'a>>>,
    /// The actions being run, innermost last: (action, next command).
    running: Vec<(usize, usize)>,
    started: Vec<bool>,
    messages: &'a [Message],
    pending: std::collections::VecDeque<Step<'a>>,
    /// How long [`Player::schedule`] holds each line: none by default, since the game waits
    /// for the player; [`Player::reading_time`] sets one for unattended playback.
    reading: Option<fn(&str) -> f32>,
    /// The motions' lengths for `s6`; without them `s6` takes no time.
    motion_length: Option<MotionLength<'a>>,
    /// The variable slots `t2/0x30` binds (slot number, 10000 and up, to a `Chara.xml` row):
    /// commands may name a character by its slot.
    slots: HashMap<u32, u32>,
    /// The protagonist the player chose (`Chara.xml` row 0 Escha, 1 Logy), which
    /// [`MASTER_CHARACTER`] stands for.
    hero: u32,
    /// The event's 20 variables (the event data's +0x94): the jumps `u8` to `u10` test them,
    /// `u80`, `u82` and `u130` set them from the game's answers (the item picked ...), which
    /// playback does not have: they stay 0, as nothing has been picked.
    vars: [i32; 20],
    /// Watched again from the Extra menu: `w4` starts its actions.
    replay: bool,
    /// `s4` with anchor kinds 1, 5 and 6 read as anchored positions ([`Step::PlaceAt`]).
    anchored_places: bool,
    /// A `p3` also waits for an action whose last command was the character's and whose wait
    /// has not ended (Escha's reading; off for the events carried over from other games, whose
    /// schedules stay as they were).
    finishing_waits: bool,
}

/// `Chara.xml` row 749, "Master Character": a stand-in for the protagonist the player chose.
/// The executable swaps it for the choice as it reads each command's operands (`0x1401ddcbe`
/// and the like: `cmp ecx, 749` → `0x14035ebb0`, which returns the global `0x141098a48`); the
/// new game's choice screen sets that to 1 for Logy and 0 for Escha (`0x1400ce4f3`).
pub const MASTER_CHARACTER: u32 = 749;

/// Marks a translated command that has no Escha counterpart ([`Player::translated`]).
pub const UNMAPPED: u32 = 0x0080_0000;

/// A reading time for unattended playback (not the game's: it waits for a button): one
/// second plus 0.08 s a character, newlines and spaces not counted.
/// An effect's pan operand (hundredths of 0 left to 128 right) as -1 to 1 (`0x140160230`:
/// `(clamp(x, 0, 128) / 128 - 0.5) × 2`).
fn se_pan(hundredths: u32) -> f32 {
    ((hundredths as i32 as f32 / 100.0).clamp(0.0, 128.0) / 128.0 - 0.5) * 2.0
}

pub fn reading_time(text: &str) -> f32 {
    1.0 + 0.08 * text.chars().filter(|c| !c.is_whitespace()).count() as f32
}

impl<'a> Player<'a> {
    /// A player for an event in the engine's own command words, decoded with `table`. The
    /// engine speaks Escha & Logy DX's numbering (the first game read, `ebd_names::ESCHA`);
    /// another game's events come in through [`Player::translated`].
    pub fn new(event: &'a Ebd, messages: &'a [Message], table: &OperandTable) -> Result<Self, Error> {
        let actions = event.actions.iter().map(|a| ebd::decode(&a.words, table)).collect::<Result<Vec<_>, _>>()?;
        let mut started = vec![false; actions.len()];
        let running = if actions.is_empty() { vec![] } else { vec![(0, 0)] };
        if let Some(s) = started.first_mut() {
            *s = true;
        }
        // `t2/0x30` (`ECT_VARIINDEX_INDEX_SETTING`, `{slot, row}`): the slots' rows (the event's
        // setup binds them once, before anything names them).
        let slots = actions.iter().flatten().filter(|c| c.word == SLOT_SET).filter_map(|c| Some((*c.operands.first()?, *c.operands.get(1)?))).collect();
        Ok(Player { names: barrel_formats::ebd_names::ESCHA.iter().copied().collect(), actions, running, started, messages, pending: Default::default(), reading: None, motion_length: None, slots, hero: 0, vars: [0; 20], replay: false, anchored_places: false, finishing_waits: true })
    }

    /// A player for another game's event (decoded with that game's `table`), its commands
    /// carried over to Escha's words by name (`game_names`, the game's word → `ECT_*` name) where
    /// both games give the name the same operand count; the others stay as they are, which
    /// the player does not know and passes over. `same` names commands by what they do where
    /// neither game's executable names them (the game's word, Escha's): taken before the names,
    /// also where the game's command adds operands after Escha's (they are not read).
    pub fn translated(event: &'a Ebd, messages: &'a [Message], table: &OperandTable, game_names: &[(u32, &str)], same: &[(u32, u32)]) -> Result<Self, Error> {
        let escha = OperandTable::escha();
        let by_name: HashMap<&str, u32> = barrel_formats::ebd_names::ESCHA.iter().map(|&(w, n)| (n, w)).collect();
        let theirs: HashMap<u32, &str> = game_names.iter().copied().collect();
        let mut p = Self::new(event, messages, table)?;
        p.finishing_waits = false;
        let escha_named: std::collections::HashSet<u32> = barrel_formats::ebd_names::ESCHA.iter().map(|&(w, _)| w).collect();
        for c in p.actions.iter_mut().flatten() {
            // A `same` entry carries over when Escha's command takes no more operands than the
            // game's: extra trailing ones (Ryza's camera moves add a curve) are left to the end.
            let fits = |w: u32| match (escha.get(w), table.get(c.word)) {
                (Some(Some(e)), Some(Some(g))) => e <= g,
                (e, g) => e == g,
            };
            if let Some(&(_, w)) = same.iter().find(|&&(g, w)| g == c.word && fits(w)) {
                c.word = w;
                continue;
            }
            match theirs.get(&c.word).and_then(|n| by_name.get(n)) {
                Some(&w) if escha.get(w) == table.get(c.word) => c.word = w,
                // Unnamed in Escha (`w1`, the step marks, and the like; the other game may name it,
                // as Ryza's `ECT_MACRO_INDEX_CHANGE`, by a name Escha's executable never
                // registered) with the same operand count in both: taken as the same command.
                None if !escha_named.contains(&c.word) && escha.get(c.word).is_some() && escha.get(c.word) == table.get(c.word) => {}
                // No name, a name Escha lacks, or other operands: kept off Escha's words (the
                // same word can mean something else there), with a flag bit no Escha word has.
                _ => c.word |= UNMAPPED,
            }
        }
        p.slots = p.actions.iter().flatten().filter(|c| c.word == SLOT_SET).filter_map(|c| Some((*c.operands.first()?, *c.operands.get(1)?))).collect();
        Ok(p)
    }

    /// Plays as `hero` (`Chara.xml` row: 0 Escha, the default, or 1 Logy), whom
    /// [`MASTER_CHARACTER`] then stands for.
    /// Reads `s4`'s anchor kinds 1, 5 and 6 as positions anchored to a thing, as the battle
    /// skill events use them (`Event/skill`: 1,241 kind 5, 415 kind 1, 2 kind 6). The story
    /// events use only kinds 0 and 3, which stay world positions; Ryza's are not checked, so
    /// this is off unless asked for.
    pub fn anchored_places(mut self) -> Self {
        self.anchored_places = true;
        self
    }

    pub fn hero(mut self, hero: u32) -> Self {
        self.hero = hero;
        self
    }

    /// Plays the event as the Extra menu replays it: `w4` ([`REPLAY_START_ACTION`]) starts its
    /// actions (in play it does nothing).
    pub fn replay(mut self, replay: bool) -> Self {
        self.replay = replay;
        self
    }

    /// Holds each line for `time(text)` seconds in [`Player::schedule`] (the action showing
    /// it waits that long), as an unattended playback would.
    pub fn reading_time(mut self, time: fn(&str) -> f32) -> Self {
        self.reading = Some(time);
        self
    }

    /// Makes `s6` wait in [`Player::schedule`] for the motion's length (`length(character,
    /// motion, division)`: seconds and whether it loops; a looping one is not waited for).
    pub fn motion_length(mut self, length: MotionLength<'a>) -> Self {
        self.motion_length = Some(length);
        self
    }

    /// Where a jump command (`u5` to `u10`) in `action` sends it: the index of the
    /// label's command.
    fn jump(&self, action: usize, c: ebd::Command<'a>) -> Option<usize> {
        // A variable beyond the 20 reads as 0 (the game's getter does not check; none does so).
        let var = |v: u32| self.vars.get(v as usize).copied().unwrap_or(0);
        let target = match (c.word, c.operands) {
            (GOTO, [n, ..]) => *n,
            (GOTO_SUM, [a, b, ..]) => a.wrapping_add(*b),
            (GOTO_FROM_BASE, [n, ..]) => LABEL_BASE.wrapping_add(*n),
            (GOTO_VAR, [first, v, ..]) => first.wrapping_add(var(*v) as u32),
            (GOTO_ABOVE, [first, v, c, ..]) => first.wrapping_add(u32::from(var(*v) > *c as i32)),
            (GOTO_EQUAL, [first, v, c, ..]) => first.wrapping_add(u32::from(var(*v) != *c as i32)),
            _ => return None,
        };
        self.actions[action].iter().position(|l| l.word == LABEL && l.operands.first() == Some(&target))
    }

    /// The name the executable gives a command word, if any.
    /// A position whose anchor names a thing by a bound slot, read with the slot's row.
    fn slot_position(&self, mut p: crate::camera::Position) -> crate::camera::Position {
        if let crate::camera::Anchor::Thing { id, .. } = &mut p.anchor {
            *id = self.slot(*id);
        }
        p
    }

    /// A character number with a bound slot (`SLOT_SET`) read as its row, and the master
    /// character as the hero.
    fn slot(&self, character: u32) -> u32 {
        let c = self.slots.get(&character).copied().unwrap_or(character);
        if c == MASTER_CHARACTER {
            self.hero
        } else {
            c
        }
    }

    pub fn name(&self, word: u32) -> Option<&'static str> {
        self.names.get(&word).copied()
    }
}

impl<'a> Player<'a> {
    /// What command `c` of action `action` does: steps to report (in order), and an action
    /// to start, if any.
    fn translate(&self, action: usize, c: ebd::Command<'a>, out: &mut Vec<Step<'a>>) -> Option<usize> {
        let f = |v: u32| v as i32 as f32;
        match (c.word, c.operands) {
            (START_ACTION, [n, ..]) => return Some(*n as usize),
            (START_ACTION, []) => {}
            (REPLAY_START_ACTION, [n, ..]) if self.replay => return Some(*n as usize),
            (REPLAY_START_ACTION, _) => {}
            (TIME_WAIT, [hundredths, ..]) => out.push(Step::Wait { seconds: f(*hundredths) / 100.0 }),
            (TIME_WAIT, []) => out.push(Step::Wait { seconds: 0.0 }),
            (SHORT_WAIT, _) => out.push(Step::Wait { seconds: 0.3 }),
            (LONG_WAIT, _) => out.push(Step::Wait { seconds: 2.0 }),
            (SET_EVENT_FLAG, [flag, value, ..]) => out.push(Step::EventFlag { flag: *flag, value: *value as i32 }),
            (CHANGE_GAME_MODE, [mode, ..]) => out.push(Step::GameMode { mode: *mode }),
            (OPEN_PLANBOARD, [n, ..]) => out.push(Step::Planboard { first: *n }),
            (SET_END_GROUP, rest) => out.push(Step::EndGroup { group: rest.first().copied().unwrap_or(0) }),
            // A missing amount reads as 0 (the case reads it only when given).
            (ADD_EVENT_FLAG, [flag, rest @ ..]) => out.push(Step::EventFlagAdd { flag: *flag, amount: rest.first().map_or(0, |&a| a as i32) }),
            (TROPHY, [trophy, ..]) => out.push(Step::Trophy { trophy: *trophy }),
            (PLAY_MOVIE, [movie, ..]) => out.push(Step::Movie { movie: *movie }),
            (MARK_EVENT, [event, rest @ ..]) => out.push(Step::MarkEvent { event: *event, seen: rest.first().is_some_and(|&v| v != 0) }),
            (CHARA_WAIT, [character, ..]) => out.push(Step::WaitForCharacter { character: self.slot(*character) }),
            (CHARA_TIME_WAIT, [character, hundredths, ..]) => {
                out.push(Step::CharacterWait { action, character: self.slot(*character), seconds: f(*hundredths) / 100.0 })
            }
            (CHARA_POS_SET, [character, kind @ (1 | 5 | 6), rest @ ..]) if self.anchored_places && rest.len() >= 5 => {
                if let Some((at, _)) = crate::camera::Position::read(&[&[*kind], rest].concat()) {
                    out.push(Step::PlaceAt { action, character: self.slot(*character), at: self.slot_position(at) });
                }
            }
            (CHARA_POS_SET, [character, 8, of, _, x, y, z, ..]) => {
                out.push(Step::PlaceNear { action, character: self.slot(*character), of: self.slot(*of), offset: [f(*x), f(*y), f(*z)] })
            }
            (CHARA_POS_SET, [character, _, _, _, x, y, z, ..]) => {
                out.push(Step::Place { action, character: self.slot(*character), position: [f(*x), f(*y), f(*z)] })
            }
            (CHARA_MOTION_LOOP, [character, motion, rest @ ..]) => {
                let blend = rest.first().map_or(0.0, |&b| f(b) / 100.0);
                out.push(Step::Motion { action, character: self.slot(*character), motion: *motion, blend, looping: true, division: None });
            }
            (CHARA_MOTION_SET, [character, motion, division, hundredths, ..]) => {
                let blend = f(*hundredths) / 100.0;
                out.push(Step::Motion { action, character: self.slot(*character), motion: *motion, blend, looping: false, division: Some(*division) });
            }
            (CHARA_MOTION_DIVISION, [character, division, ..]) => out.push(Step::MotionDivision { action, character: self.slot(*character), division: *division }),
            (CHARA_MOTION_AUTO_LOOP, [character, motion, division, rest @ ..]) => {
                let blend = rest.get(1).map_or(0.0, |&b| f(b) / 100.0);
                out.push(Step::Motion { action, character: self.slot(*character), motion: *motion, blend, looping: true, division: Some(*division) });
            }
            (FIELD_CHANGE | FIELD_CHANGE_KEEP_BGM, [map, ..]) => out.push(Step::FieldChange { action, map: *map }),
            (FIELD_CHARA_DELETE, [character, ..]) => out.push(Step::Remove { action, character: self.slot(*character) }),
            (FIELD_CHARA_DELETE_ALL, _) => out.push(Step::RemoveAll { action }),
            (FIELD_CHARA_COLLISION, [character, rest @ ..]) => out.push(Step::Collision {
                action,
                character: self.slot(*character),
                solid: rest.first().is_none_or(|&v| v != 0),
                flag: rest.get(1).is_none_or(|&v| v != 0),
            }),
            (CAMERA_QUAKE_SET, [quake, rest @ ..]) => out.push(Step::Quake { action, quake: *quake, seconds: rest.first().map_or(0.0, |&s| f(s) / 100.0) }),
            (CAMERA_QUAKE_STOP, _) => out.push(Step::QuakeStop { action }),
            (CAMERA_ROT_MOVE_SET, [from, to, hundredths, curve, ..]) => {
                out.push(Step::CameraRoll { action, from: f(*from), to: f(*to), seconds: f(*hundredths) / 100.0, curve: *curve })
            }
            (CAMERA_LENGTH_MOVE_SET, [length, hundredths, curve, ..]) => {
                out.push(Step::CameraDistance { action, length: f(*length), seconds: f(*hundredths) / 100.0, curve: *curve })
            }
            (FIELD_EVENT_OBJECTS | FIELD_EVENT_OBJECTS_2, rest) => out.push(Step::EventObjects { action, shown: rest.first().is_some_and(|&v| v != 0) }),
            (FIELD_MAP_VISIBLE, [version, shown, ..]) => out.push(Step::MapVisible { action, version: *version, shown: *shown != 0 }),
            (EVENT_VOICE_PLAY, [clip, ..]) => out.push(Step::Voice { action, character: None, clip: *clip }),
            (SOUND_BG_PLAY, [track, volume, ..]) => out.push(Step::Music { action, track: Some(*track), volume: *volume as f32 / 100.0, fade: 0.0 }),
            (SOUND_BG_STOP, _) => out.push(Step::Music { action, track: None, volume: 0.0, fade: 0.0 }),
            (SOUND_BG_FADEOUT, [hundredths, ..]) => out.push(Step::Music { action, track: None, volume: 0.0, fade: *hundredths as f32 / 100.0 }),
            (CHARA_HEAD_GESTURE, [character, segment, rest @ ..]) => out.push(Step::HeadGesture {
                action,
                character: self.slot(*character),
                segment: *segment as i32,
                rate: rest.first().map_or(1.0, |&v| v as i32 as f32 / 100.0),
                weight: rest.get(1).map_or(1.0, |&v| v as i32 as f32 / 100.0),
            }),
            (CHARA_HEAD_STOP, [character, ..]) => out.push(Step::HeadStop { action, character: self.slot(*character) }),
            (STILL_SHOW, [picture, ..]) => out.push(Step::Still { action, picture: *picture, seconds: 0.0 }),
            (STILL_FADE, [picture, rest @ ..]) => {
                out.push(Step::Still { action, picture: *picture, seconds: rest.first().map_or(0.0, |&h| h as i32 as f32 / 100.0) })
            }
            (EVENT_TELOP, [card, ..]) => out.push(Step::Telop { action, card: *card }),
            (EVENT_TELOP_PART, [part, rest @ ..]) => out.push(Step::TelopPart {
                action,
                part: *part,
                shown: rest.first().is_some_and(|&v| v != 0),
                seconds: rest.get(1).map_or(0.0, |&h| h as i32 as f32 / 100.0),
            }),
            (EVENT_TELOP_ALPHA, [from, to, hundredths, ..]) => out.push(Step::TelopAlpha {
                action,
                from: *from as i32 as f32 / 100.0,
                to: *to as i32 as f32 / 100.0,
                seconds: *hundredths as i32 as f32 / 100.0,
            }),
            (FIELDMAP_LIGHT_PARALLEL_SET, [light, r, g, b, pitch, yaw, ..]) => {
                let k = |v: &u32| *v as i32 as f32 / 128.0;
                let set = Some(([k(r), k(g), k(b)], *pitch as i32 as f32 / 100.0, *yaw as i32 as f32 / 100.0));
                out.push(Step::Light { action, change: LightChange::Parallel { light: *light, set } })
            }
            (FIELDMAP_LIGHT_PARALLEL_ORG, [light, ..]) => out.push(Step::Light { action, change: LightChange::Parallel { light: *light, set: None } }),
            (FIELDMAP_LIGHT_AMBIENT_SET, [r, g, b, ..]) => {
                let k = |v: &u32| *v as i32 as f32 / 128.0;
                out.push(Step::Light { action, change: LightChange::Ambient(Some([k(r), k(g), k(b)])) })
            }
            (FIELDMAP_POINT_LIGHT_SET, [light, x, y, z, r, g, b, ar, ag, ab, att0, att1, fall, range, ..]) => {
                let set = Some(point_light([*x, *y, *z], [*r, *g, *b], [*ar, *ag, *ab], [*att0, *att1, *fall], *range));
                out.push(Step::Light { action, change: LightChange::Point { light: *light, set } })
            }
            (FIELDMAP_POINT_LIGHT_ORG, [light, ..]) => out.push(Step::Light { action, change: LightChange::Point { light: *light, set: None } }),
            (FIELDMAP_LIGHT_AMBIENT_ORG, _) => out.push(Step::Light { action, change: LightChange::Ambient(None) }),
            (FIELDMAP_CHARA_LIGHT_COLOR_SET | FIELDMAP_CHARA_RIM_LIGHT_COLOR_SET, [r, g, b, w, ..]) => {
                let k = |v: &u32| *v as i32 as f32 / 100.0;
                let v = Some([k(r), k(g), k(b), k(w)]);
                let change = if c.word == FIELDMAP_CHARA_LIGHT_COLOR_SET { LightChange::CharaColor(v) } else { LightChange::RimColor(v) };
                out.push(Step::Light { action, change })
            }
            (FIELDMAP_CHARA_LIGHT_COLOR_ORG, _) => out.push(Step::Light { action, change: LightChange::CharaColor(None) }),
            (FIELDMAP_CHARA_RIM_LIGHT_COLOR_ORG, _) => out.push(Step::Light { action, change: LightChange::RimColor(None) }),
            (FIELDMAP_CHARA_POINT_LIGHT_STRENGTH_SET, [v, ..]) => {
                out.push(Step::Light { action, change: LightChange::PointStrength(Some(*v as i32 as f32 / 100.0)) })
            }
            (FIELDMAP_CHARA_POINT_LIGHT_STRENGTH_ORG, _) => out.push(Step::Light { action, change: LightChange::PointStrength(None) }),
            (EVENT_CONTRAST_SET_MIX, [_, preset, hundredths, ..]) => out.push(Step::Fade { action, preset: *preset, seconds: *hundredths as i32 as f32 / 100.0 }),
            (EVENT_SET_POST_EFFECT, [_, param, hundredths, kind, ..]) => {
                out.push(Step::ScreenEffect { action, kind: *kind, param: *param as i32, seconds: *hundredths as i32 as f32 / 100.0 })
            }
            (SOUND_SE_PLAY | SOUND_EFFECT, [effect, _, volume, pan, ..]) => out.push(Step::Effect { action, character: None, effect: *effect, volume: *volume as f32 / 100.0, pan: se_pan(*pan) }),
            (CHARA_SE_PLAY, [character, effect, _, volume, pan, ..]) => {
                out.push(Step::Effect { action, character: Some(*character), effect: *effect, volume: *volume as f32 / 100.0, pan: se_pan(*pan) })
            }
            (CHARA_VOICE_PLAY, [character, clip, ..]) => out.push(Step::Voice { action, character: Some(*character), clip: *clip }),
            (SHOW_MESSAGES, [first, last]) => {
                for i in *first as usize..=(*last as usize) {
                    if let Some(m) = self.messages.get(i) {
                        let field = |k: usize| m.fields.get(k).copied().filter(|&v| v >= 0);
                        out.push(Step::Message { index: i, speaker: m.speaker, name: field(1), text: m.repaired_text(), expression: field(3), gesture: field(2) });
                    }
                }
            }
            (FIELD_CHARA_ADD, [character, _, _, _, x, y, z, _, angle, ..]) => {
                out.push(Step::Place { action, character: self.slot(*character), position: [f(*x), f(*y), f(*z)] });
                out.push(Step::Turn { action, character: self.slot(*character), rotation: [0.0, f(*angle).to_radians(), 0.0] });
            }
            (CHARA_ROT_SET, [character, x, y, z, ..]) => {
                let rotation = [*x, *y, *z].map(|v| (v as i32 as f32).to_radians());
                out.push(Step::Turn { action, character: self.slot(*character), rotation })
            }
            (CHARA_ROT_MOVE, [character, x, y, z, hundredths, curve, ..]) => {
                let rotation = [*x, *y, *z].map(|v| (v as i32 as f32).to_radians());
                let seconds = *hundredths as i32 as f32 / 100.0;
                out.push(Step::TurnOver { action, character: self.slot(*character), rotation, seconds, curve: *curve })
            }
            (CHARA_ROT_SET_TARGET, [character, rest @ ..]) if crate::camera::Position::read(rest).is_some() => {
                let (toward, _) = crate::camera::Position::read(rest).unwrap();
                out.push(Step::Face { action, character: self.slot(*character), toward: self.slot_position(toward) })
            }
            (CHARA_ROT_COPY_TARGET | CHARA_ROT_COPY_TARGET_ALL, [character, rest @ ..]) if crate::camera::Position::read(rest).is_some() => {
                let (from, _) = crate::camera::Position::read(rest).unwrap();
                out.push(Step::CopyFacing { action, character: self.slot(*character), from: self.slot_position(from) })
            }
            (CHARA_MOVE, [character, rest @ ..]) if crate::camera::Move::read_with_extra(rest).is_some() => {
                let (extra, movement) = crate::camera::Move::read_with_extra(rest).unwrap();
                let mut movement = movement;
                *movement.to_mut() = self.slot_position(*movement.to_mut());
                out.push(Step::CharacterMove { action, character: self.slot(*character), extra, movement })
            }
            (word, operands) if crate::field_camera::Change::read(word, operands).is_some() => {
                let mut change = crate::field_camera::Change::read(word, operands).unwrap();
                if let crate::field_camera::Change::Follow { character: Some(c) } = &mut change {
                    *c = self.slot(*c);
                }
                out.push(Step::FieldCamera { action, change })
            }
            (word, operands) if !crate::camera::changes(word, operands).is_empty() => {
                out.extend(crate::camera::changes(word, operands).into_iter().map(|(part, change)| {
                    let change = match change {
                        crate::camera::Change::Set(p) => crate::camera::Change::Set(self.slot_position(p)),
                        crate::camera::Change::Move(mut m) => {
                            *m.to_mut() = self.slot_position(*m.to_mut());
                            crate::camera::Change::Move(m)
                        }
                    };
                    Step::Camera { action, part, change }
                }))
            }
            (word, _) if MARKERS.contains(&word) || word == SLOT_SET || word == SLOT_START => {}
            (word, operands) => out.push(Step::Command { action, word, name: self.name(word), operands }),
        }
        None
    }

    /// The event on a clock: every started action runs at once, each held only by its own
    /// waits (`p1`, `u1`). The game keeps one wait timer per event, not per action
    /// (`docs/events.md`), so overlapping waits may run differently there; not settled. Messages take no time here
    /// (the game waits for the player to read them), nor do waits for characters or the
    /// camera, whose lengths depend on what they wait for.
    pub fn schedule(mut self) -> Vec<Timed<'a>> {
        // (ready at, action, next command), in start order.
        let mut threads: Vec<(f32, usize, usize)> = vec![];
        if !self.actions.is_empty() {
            threads.push((0.0, 0, 0));
        }
        let mut out = Vec::new();
        let mut steps = Vec::new();
        // Each character's motion: (motion, division, started at, whether the command loops it).
        let mut motions: HashMap<u32, (u32, Option<u32>, f32, bool)> = HashMap::new();
        // When the latest screen effect (u13) ends: u14 waits for it.
        let mut effect_end = 0.0f32;
        // A stage the steps are applied to, for where moves start and so how long they take;
        // each character's move end and timed turn end (s21, s29 wait for them).
        let mut scratch = crate::stage::Stage::default();
        let mut move_end: HashMap<u32, f32> = HashMap::new();
        let mut turn_end: HashMap<u32, f32> = HashMap::new();
        while let Some(t) = (0..threads.len()).filter(|&i| threads[i].2 < self.actions[threads[i].1].len()).min_by(|&i, &j| {
            threads[i].0.partial_cmp(&threads[j].0).unwrap().then(i.cmp(&j))
        }) {
            let (now, action, pc) = threads[t];
            threads[t].2 = pc + 1;
            let c = self.actions[action][pc];
            // p3: wait while another running action still has commands for that character (its
            // command runner busy, `docs/events.md`, "Waiting for a character"): retry once the
            // earliest of them has moved on. An action whose last command for it was a wait (a
            // motion's end, `s6`) still runs until that wait ends, so it counts until then.
            if let (CHARA_WAIT, [character, ..]) = (c.word, c.operands) {
                let for_it = |r: &ebd::Command| r.word >> 24 == 0x73 && r.operands.first() == Some(character);
                let busy = (0..threads.len())
                    .filter(|&k| k != t)
                    .filter(|&k| {
                        let (at, a, from) = threads[k];
                        let queued = self.actions[a].get(from..).is_some_and(|rest| rest.iter().any(for_it));
                        let finishing = self.finishing_waits && from >= self.actions[a].len() && at > now && self.actions[a].last().is_some_and(for_it);
                        queued || finishing
                    })
                    .map(|k| threads[k].0)
                    .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.min(v))));
                if let Some(ready) = busy {
                    threads[t] = (ready.max(now) + 1e-4, action, pc);
                    continue;
                }
            }
            if let Some(to) = self.jump(action, c) {
                threads[t].2 = to;
                continue;
            }
            steps.clear();
            if let Some(n) = self.translate(action, c, &mut steps)
                && n < self.actions.len() && !self.started[n] {
                    self.started[n] = true;
                    threads.push((now, n, 0));
                    out.push(Timed { time: now, action, step: Step::StartAction { action: n } });
                }
            for s in steps.drain(..) {
                if let Step::Wait { seconds } = s {
                    threads[t].0 += seconds.max(0.0);
                }
                let held = match (&s, self.reading) {
                    (Step::Message { text, .. }, Some(f)) => f(&text.replace("<CR>", "")).max(0.0),
                    _ => 0.0,
                };
                match &s {
                    Step::Motion { character, motion, division, looping, .. } => {
                        motions.insert(*character, (*motion, *division, threads[t].0, *looping));
                    }
                    Step::MotionDivision { character, division, .. } => {
                        if let Some(m) = motions.get_mut(character) {
                            (m.1, m.2) = (Some(*division), threads[t].0);
                        }
                    }
                    Step::ScreenEffect { seconds, .. } => effect_end = threads[t].0 + seconds.max(0.0),
                    Step::CharacterMove { character, .. } => {
                        scratch.apply(&s);
                        match scratch.move_left(*character) {
                            Some(left) => move_end.insert(*character, threads[t].0 + left),
                            None => move_end.remove(character),
                        };
                    }
                    Step::TurnOver { character, seconds, .. } => {
                        turn_end.insert(*character, threads[t].0 + seconds.max(0.0));
                    }
                    Step::Command { word: CHARA_MOVE_PRE_WAIT, operands: [character, rest @ ..], .. } => {
                        let before = rest.first().map_or(0.0, |&h| h as i32 as f32 / 100.0);
                        if let Some(&end) = move_end.get(character) {
                            threads[t].0 = threads[t].0.max(end - before);
                        }
                    }
                    Step::Command { word: CHARA_ROT_MOVE_WAIT, operands: [character, ..], .. } => {
                        if let Some(&end) = turn_end.get(character) {
                            threads[t].0 = threads[t].0.max(end);
                        }
                    }
                    Step::Command { word: EVENT_WAIT_POST_EFFECT, .. } => threads[t].0 = threads[t].0.max(effect_end),
                    Step::Command { word: CHARA_MOTION_WAIT, operands: [character, ..], .. } => {
                        let end = motions.get(character).and_then(|&(motion, division, since, looping)| {
                            let (length, loops) = self.motion_length.as_ref()?(*character, motion, division)?;
                            // A division says whether it loops; a whole clip loops as its command said.
                            let loops = if division.is_some() { loops } else { looping };
                            (!loops).then_some(since + length)
                        });
                        if let Some(end) = end {
                            threads[t].0 = threads[t].0.max(end);
                        }
                    }
                    _ => {}
                }
                if !matches!(s, Step::CharacterMove { .. }) {
                    scratch.apply(&s);
                }
                let shown_at = threads[t].0;
                threads[t].0 += held;
                out.push(Timed { time: if held > 0.0 { shown_at } else { now }, action, step: s });
            }
        }
        // In time order (stable: steps at the same moment keep their order). A thread that
        // holds lines runs ahead of the others, so its later lines came out before their steps
        // (event 212: action 20's camera cut, at line 2, came after lines 2 to 4 and was
        // never reached while they showed).
        out.sort_by(|a, b| a.time.total_cmp(&b.time));
        out
    }
}

/// A step and when it happens, in seconds from the event's start, and the action it is in.
#[derive(Clone, Debug, PartialEq)]
pub struct Timed<'a> {
    pub time: f32,
    pub action: usize,
    pub step: Step<'a>,
}

impl<'a> Iterator for Player<'a> {
    type Item = Step<'a>;

    fn next(&mut self) -> Option<Step<'a>> {
        loop {
            if let Some(s) = self.pending.pop_front() {
                return Some(s);
            }
            let &mut (action, ref mut next) = self.running.last_mut()?;
            let Some(&c) = self.actions[action].get(*next) else {
                self.running.pop();
                continue;
            };
            *next += 1;
            if let Some(to) = self.jump(action, c) {
                if let Some(top) = self.running.last_mut() {
                    top.1 = to;
                }
                continue;
            }
            let mut steps = Vec::new();
            if let Some(n) = self.translate(action, c, &mut steps)
                && n < self.actions.len() && !self.started[n] {
                    self.started[n] = true;
                    self.running.push((n, 0));
                    return Some(Step::StartAction { action: n });
                }
            self.pending.extend(steps);
        }
    }
}

/// The field an event leaves the player in: the map of its last field change, if it has one.
pub fn last_field(steps: &[Timed]) -> Option<u32> {
    steps.iter().rev().find_map(|t| match t.step {
        Step::FieldChange { map, .. } => Some(map),
        _ => None,
    })
}

/// The field an event plays in: the map of its first field change, if it has one.
pub fn first_field(steps: &[Timed]) -> Option<u32> {
    steps.iter().find_map(|t| match t.step {
        Step::FieldChange { map, .. } => Some(map),
        _ => None,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn event_point_light() {
        // EVENT_MQ09's commonest q54: {1, 2657, 180, 2865, 200, 128, 80, 150, 100, 30, 600, 12, 233, 1500}.
        let barrel_formats::field_look::Light::Point { pos, color, att, range, .. } =
            super::point_light([2657, 180, 2865], [200, 128, 80], [150, 100, 30], [600, 12, 233], 1500)
        else {
            panic!()
        };
        assert_eq!(pos, [26.57, 1.8, 28.65]);
        assert_eq!(color, [200.0 / 128.0, 1.0, 80.0 / 128.0, 1.0]);
        assert_eq!(range, 15.0);
        assert!((1.0 / (att[0] + att[2] * 1500.0 * 1500.0) - 0.233).abs() < 1e-4);
    }

    #[test]
    fn another_games_words_carry_over_by_name() {
        use barrel_formats::ebd::{Action, Ebd, END};
        // A game where ECT_MACRO_TIME_WAIT is word 0x7000_0009 and an unnamed 0x7f00_0000 means
        // something Escha has no word for.
        let names = [(0x7000_0009u32, "ECT_MACRO_TIME_WAIT")];
        let table = OperandTable::new(&[(0x7000_0009, Some(1)), (0x7400_0007, Some(0))]);
        let event = Ebd { actions: vec![Action { flags: 0, name: vec![], description: vec![], words: vec![0x7000_0009, 150, 0x7400_0007, END] }], resources: vec![] };
        let p = Player::translated(&event, &[], &table, &names, &[]).unwrap();
        assert_eq!(p.actions[0][0].word, TIME_WAIT);
        // 0x7400_0007 is a camera command in Escha: the stranger's word is kept off it.
        assert_eq!(p.actions[0][1].word, 0x7400_0007 | UNMAPPED);
    }

    #[test]
    fn effect_pan() {
        assert_eq!(se_pan(6400), 0.0);
        assert_eq!(se_pan(0), -1.0);
        assert_eq!(se_pan(12800), 1.0);
        assert_eq!(se_pan(99999), 1.0);
        assert_eq!(se_pan(u32::MAX), -1.0);
    }

    use super::*;

    #[test]
    fn mark_event_and_long_wait_read() {
        assert_eq!(Step::MarkEvent { event: 241, seen: true }, Step::MarkEvent { event: 241, seen: true });
        assert_eq!((LONG_WAIT, MARK_EVENT), (0x7500_000f, 0x7500_0024));
    }
    use barrel_formats::ebd::{Action, END};

    #[test]
    fn jumps_test_the_variables() {
        // Nothing picked (every variable 0): u8 and u9 go to `first`, u10 to `first` when it
        // tests for 0 and to `first + 1` otherwise.
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        let table = OperandTable::new(&[(GOTO_FROM_BASE, Some(1)), (GOTO_VAR, Some(2)), (GOTO_ABOVE, Some(3)), (GOTO_EQUAL, Some(3)), (LABEL, Some(1)), (TROPHY, Some(1))]);
        let run = |jump: Vec<u32>| -> Vec<String> {
            let mut words = jump;
            words.extend([TROPHY, 1, LABEL, 7, TROPHY, 7, LABEL, 8, TROPHY, 8, END]);
            let event = Ebd { actions: vec![act(words)], resources: vec![] };
            Player::new(&event, &[], &table).unwrap().filter(|s| matches!(s, Step::Trophy { .. })).map(|s| format!("{s:?}")).collect()
        };
        let t = |n: u32| format!("{:?}", Step::Trophy { trophy: n });
        assert_eq!(run(vec![GOTO_VAR, 7, 3]), [t(7), t(8)]);
        assert_eq!(run(vec![GOTO_ABOVE, 7, 3, 0]), [t(7), t(8)]);
        assert_eq!(run(vec![GOTO_EQUAL, 7, 3, 0]), [t(7), t(8)]);
        assert_eq!(run(vec![GOTO_EQUAL, 7, 3, 5]), [t(8)]);
        // u7 counts from label 1.
        assert_eq!(run(vec![GOTO_FROM_BASE, 7]), [t(8)]);
    }

    #[test]
    fn replays_start_their_own_actions() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        let event = Ebd { actions: vec![act(vec![REPLAY_START_ACTION, 1, END]), act(vec![TROPHY, 9, END])], resources: vec![] };
        let table = OperandTable::new(&[(REPLAY_START_ACTION, Some(1)), (TROPHY, Some(1))]);
        let trophies = |replay| -> usize {
            Player::new(&event, &[], &table).unwrap().replay(replay).filter(|s| matches!(s, Step::Trophy { .. })).count()
        };
        assert_eq!((trophies(false), trophies(true)), (0, 1));
    }

    #[test]
    fn an_event_hands_over_to_the_world_map() {
        let act = Action { flags: 0, name: vec![], description: vec![], words: vec![CHANGE_GAME_MODE, 14, 0, 0, END] };
        let event = Ebd { actions: vec![act], resources: vec![] };
        let table = OperandTable::new(&[(CHANGE_GAME_MODE, Some(3))]);
        let steps: Vec<Step> = Player::new(&event, &[], &table).unwrap().collect();
        assert!(steps.contains(&Step::GameMode { mode: 14 }) && GAME_MODES[14] == "WORLDMAP" && GAME_MODES[13] == "MOVIE");
        assert_eq!((GAME_MODES[PLANBOARD_MODE as usize], GAME_MODES[74]), ("PLANBOARD", "DEBUG_EXPLORE_EQUIP"));
    }

    #[test]
    fn a_card_piece_fades_in() {
        let act = Action { flags: 0, name: vec![], description: vec![], words: vec![EVENT_TELOP_PART, 3, 1, 50, END] };
        let event = Ebd { actions: vec![act], resources: vec![] };
        let table = OperandTable::new(&[(EVENT_TELOP_PART, Some(3))]);
        let steps: Vec<Step> = Player::new(&event, &[], &table).unwrap().collect();
        assert!(steps.contains(&Step::TelopPart { action: 0, part: 3, shown: true, seconds: 0.5 }), "{steps:?}");
    }

    #[test]
    fn a_flag_is_added_to() {
        let act = Action { flags: 0, name: vec![], description: vec![], words: vec![ADD_EVENT_FLAG, 12, 3, END] };
        let event = Ebd { actions: vec![act], resources: vec![] };
        let table = OperandTable::new(&[(ADD_EVENT_FLAG, Some(2))]);
        let steps: Vec<Step> = Player::new(&event, &[], &table).unwrap().collect();
        assert!(steps.contains(&Step::EventFlagAdd { flag: 12, amount: 3 }), "{steps:?}");
    }

    #[test]
    fn a_movie_plays() {
        let act = Action { flags: 0, name: vec![], description: vec![], words: vec![PLAY_MOVIE, 5, 1, 0, END] };
        let event = Ebd { actions: vec![act], resources: vec![] };
        let table = OperandTable::new(&[(PLAY_MOVIE, Some(3))]);
        let steps: Vec<Step> = Player::new(&event, &[], &table).unwrap().collect();
        assert_eq!(steps, [Step::Movie { movie: 5 }]);
    }

    #[test]
    fn the_master_character_is_the_hero() {
        use crate::field_camera::{Change, FOLLOW};
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // An event's end hands the camera back: follow 749, then 749 waits for a character.
        let event = Ebd { actions: vec![act(vec![FOLLOW, MASTER_CHARACTER, 1, CHARA_WAIT, MASTER_CHARACTER, END])], resources: vec![] };
        let table = OperandTable::new(&[(FOLLOW, Some(2)), (CHARA_WAIT, Some(1))]);
        let follows = |hero: Option<u32>| -> Vec<String> {
            let p = Player::new(&event, &[], &table).unwrap();
            let p = match hero {
                Some(h) => p.hero(h),
                None => p,
            };
            p.map(|s| match s {
                Step::FieldCamera { change: Change::Follow { character }, .. } => format!("follow {character:?}"),
                Step::WaitForCharacter { character } => format!("wait {character}"),
                other => format!("{other:?}"),
            })
            .collect()
        };
        // Escha by default, Logy when chosen.
        assert_eq!(follows(None), ["follow Some(0)", "wait 0"]);
        assert_eq!(follows(Some(1)), ["follow Some(1)", "wait 1"]);
    }

    #[test]
    fn actions_start_where_the_main_script_says() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // Action 0 waits, starts 2, waits, starts 1 (and 2 again, which does nothing).
        let event = Ebd {
            actions: vec![
                act(vec![TIME_WAIT, 10, START_ACTION, 2, TIME_WAIT, 20, START_ACTION, 1, START_ACTION, 2, END]),
                act(vec![TIME_WAIT, 100, END]),
                act(vec![TIME_WAIT, 200, END]),
                act(vec![TIME_WAIT, 300, END]),
            ],
            resources: vec![],
        };
        let table = OperandTable::new(&[(START_ACTION, Some(1)), (TIME_WAIT, Some(1))]);
        let steps: Vec<String> = Player::new(&event, &[], &table)
            .unwrap()
            .map(|s| match s {
                Step::StartAction { action } => format!("start {action}"),
                Step::Wait { seconds } => format!("wait {seconds}"),
                other => format!("{other:?}"),
            })
            .collect();
        // Action 3 is never started, so it never runs.
        assert_eq!(steps, ["wait 0.1", "start 2", "wait 2", "wait 0.2", "start 1", "wait 1"]);
    }

    #[test]
    fn effect_waits_for_the_effect() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // u13 -1 0 50 3 (a half-second cross-fade), a 0.2 s wait, u14, then a wait to mark the time.
        let event = Ebd {
            actions: vec![act(vec![EVENT_SET_POST_EFFECT, u32::MAX, 0, 50, 3, TIME_WAIT, 20, EVENT_WAIT_POST_EFFECT, TIME_WAIT, 10, END])],
            resources: vec![],
        };
        let table = OperandTable::new(&[(EVENT_SET_POST_EFFECT, Some(4)), (EVENT_WAIT_POST_EFFECT, Some(0)), (TIME_WAIT, Some(1))]);
        let steps = Player::new(&event, &[], &table).unwrap().schedule();
        let waits: Vec<f32> = steps.iter().filter(|t| matches!(t.step, Step::Wait { .. })).map(|t| t.time).collect();
        assert_eq!(waits, [0.0, 0.5]);
    }

    #[test]
    fn motion_waits_last_the_division() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // s11 0 76 0 1 40, s6 0, s13 0 1 1, s6 0, then a wait to mark the time.
        let event = Ebd {
            actions: vec![act(vec![
                CHARA_MOTION_AUTO_LOOP, 0, 76, 0, 1, 40,
                CHARA_MOTION_WAIT, 0,
                CHARA_MOTION_DIVISION, 0, 1, 1,
                CHARA_MOTION_WAIT, 0,
                TIME_WAIT, 10,
                END,
            ])],
            resources: vec![],
        };
        let table = OperandTable::new(&[
            (CHARA_MOTION_AUTO_LOOP, Some(5)),
            (CHARA_MOTION_WAIT, Some(1)),
            (CHARA_MOTION_DIVISION, Some(3)),
            (TIME_WAIT, Some(1)),
        ]);
        // Division 0 plays 0.5 s once; division 1 loops (not waited for).
        let steps = Player::new(&event, &[], &table)
            .unwrap()
            .motion_length(Box::new(|_, _, d| match d {
                Some(0) => Some((0.5, false)),
                _ => Some((2.0, true)),
            }))
            .schedule();
        let wait = steps.iter().find(|t| matches!(t.step, Step::Wait { .. })).unwrap();
        assert_eq!(wait.time, 0.5);
    }

    #[test]
    fn actions_run_at_once_on_a_clock() {
        let act = |words: Vec<u32>| Action { flags: 0, name: vec![], description: vec![], words };
        // Main: start 1, wait 1 s, mark; action 1: wait 0.5 s, mark, wait 1 s, mark.
        let mark = 0x7300_0006;
        let event = Ebd {
            actions: vec![
                act(vec![START_ACTION, 1, TIME_WAIT, 100, mark, 0, END]),
                act(vec![TIME_WAIT, 50, mark, 1, TIME_WAIT, 100, mark, 2, END]),
            ],
            resources: vec![],
        };
        let table = OperandTable::new(&[(START_ACTION, Some(1)), (TIME_WAIT, Some(1)), (mark, Some(1))]);
        let marks: Vec<(f32, u32)> = Player::new(&event, &[], &table)
            .unwrap()
            .schedule()
            .into_iter()
            .filter_map(|t| match t.step {
                Step::Command { operands: [m], .. } => Some((t.time, *m)),
                _ => None,
            })
            .collect();
        assert_eq!(marks, [(0.5, 1), (1.0, 0), (1.5, 2)]);
    }

    #[test]
    fn messages_come_out_in_ranges() {
        let words = vec![
            STEP, 1, SHOW_MESSAGES, 0, 1, 0x7300_0006, 4, TIME_WAIT, 50, SHOW_MESSAGES, 2, 2,
            CHARA_POS_SET, 0, 0, 0, 0, 100, (-5i32) as u32, 200, CHARA_MOTION_LOOP, 0, 32, 40, 1, CHARA_TIME_WAIT, 1, 150, END,
        ];
        let event = Ebd { actions: vec![Action { flags: 0, name: vec![], description: vec![], words }], resources: vec![] };
        let msg = |t: &str| Message { kind: 1, speaker: 0, fields: vec![0; 6], after: Vec::new(), text: t.into() };
        let messages = vec![msg("a"), msg("b"), msg("c")];
        let table = OperandTable::new(&[
            (STEP, Some(1)),
            (SHOW_MESSAGES, Some(2)),
            (0x7300_0006, Some(1)),
            (TIME_WAIT, Some(1)),
            (CHARA_POS_SET, Some(7)),
            (CHARA_MOTION_LOOP, Some(4)),
            (CHARA_TIME_WAIT, Some(2)),
        ]);
        // Read unattended, each line holds its reading time: "a" at 0, "b" after it, the wait
        // and "c" after that.
        let timed = Player::new(&event, &messages, &table).unwrap().reading_time(|_| 2.0).schedule();
        let at: Vec<(String, f32)> =
            timed.iter().filter_map(|t| match &t.step { Step::Message { text, .. } => Some((text.clone(), t.time)), _ => None }).collect();
        assert_eq!(at, [("a".into(), 0.0), ("b".into(), 2.0), ("c".into(), 4.5)]);
        let steps: Vec<Step> = Player::new(&event, &messages, &table).unwrap().collect();
        let texts: Vec<String> = steps
            .iter()
            .map(|s| match s {
                Step::Message { text, .. } => text.clone(),
                Step::StartAction { action } => format!("start {action}"),
                Step::Command { name, .. } => name.unwrap_or("?").to_string(),
                Step::Wait { seconds } => format!("wait {seconds}"),
                Step::PlaceNear { character, of, offset, .. } => format!("place {character} at {offset:?} from {of}"),
                Step::PlaceAt { character, at, .. } => format!("{character} at {at:?}"),
                Step::MarkEvent { event, seen } => format!("mark {event} {seen}"),
                Step::EventFlag { flag, value } => format!("flag {flag} = {value}"),
                Step::EventFlagAdd { flag, amount } => format!("flag {flag} += {amount}"),
                Step::EndGroup { group } => format!("end runs process group {group}"),
                Step::Planboard { first } => format!("planning board from {first}"),
                Step::GameMode { mode } => format!("then game mode {}", GAME_MODES.get(*mode as usize).unwrap_or(&"?")),
                Step::Trophy { trophy } => format!("trophy {trophy}"),
                Step::Movie { movie } => format!("movie {movie}"),
                Step::WaitForCharacter { character } => format!("wait for {character}"),
                Step::CharacterWait { character, seconds, .. } => format!("{character} waits {seconds}"),
                Step::Place { character, position, .. } => format!("{character} at {position:?}"),
                Step::Motion { character, motion, blend, .. } => format!("{character} motion {motion} blend {blend}"),
                Step::MotionDivision { character, division, .. } => format!("{character} motion division {division}"),
                Step::Camera { part, .. } => format!("camera {part:?}"),
                Step::FieldCamera { change, .. } => format!("field camera {change:?}"),
                Step::Remove { character, .. } => format!("{character} removed"),
                Step::RemoveAll { .. } => "everyone removed".to_string(),
                Step::Collision { character, solid, .. } => format!("{character} solid {solid}"),
                Step::MapVisible { version, shown, .. } => format!("version {version} shown {shown}"),
                Step::EventObjects { shown, .. } => format!("event objects shown {shown}"),
                Step::Quake { quake, seconds, .. } => format!("quake {quake} for {seconds} s"),
                Step::QuakeStop { .. } => "quake stops".to_string(),
                Step::CameraRoll { from, to, seconds, .. } => format!("camera rolls {from}° -> {to}° over {seconds} s"),
                Step::CameraDistance { length, seconds, .. } => format!("eye slides to {length} out over {seconds} s"),
                Step::Voice { clip, .. } => format!("voice {clip}"),
                Step::Effect { effect, .. } => format!("effect {effect}"),
                Step::ScreenEffect { kind, seconds, .. } => format!("screen effect {kind} over {seconds} s"),
                Step::Fade { preset, seconds, .. } => format!("fade {preset} over {seconds} s"),
                Step::Light { change, .. } => format!("light {change:?}"),
                Step::Telop { card, .. } => format!("narration card {card}"),
                Step::Still { picture, seconds, .. } => format!("still {picture} over {seconds} s"),
                Step::HeadGesture { character, segment, rate, weight, .. } => format!("{character} head gesture {segment} at {rate}× (weight {weight})"),
                Step::HeadStop { character, .. } => format!("{character} head gesture ends"),
                Step::TelopAlpha { from, to, seconds, .. } => format!("narration card {from} -> {to} over {seconds} s"),
                Step::TelopPart { part, shown, seconds, .. } => format!("narration card piece {part} {} over {seconds} s", if *shown { "shown" } else { "hidden" }),
                Step::Music { track, fade, .. } => match track {
                    Some(t) => format!("music {t}"),
                    None => format!("music stops over {fade} s"),
                },
                Step::FieldChange { map, .. } => format!("field {map}"),
                Step::CharacterMove { character, .. } => format!("{character} moves"),
                Step::Turn { character, .. } | Step::Face { character, .. } | Step::CopyFacing { character, .. } | Step::TurnOver { character, .. } => {
                    format!("{character} turns")
                }
            })
            .collect();
        assert_eq!(
            texts,
            ["a", "b", "ECT_CHARA_MOTION_WAIT", "wait 0.5", "c", "0 at [100.0, -5.0, 200.0]", "0 motion 32 blend 0.4", "1 waits 1.5"]
        );
    }
}
