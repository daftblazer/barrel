//! `barrel`: inspect and unpack Atelier game data.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use barrel_formats::ebd::{self, Ebd, OperandTable, Word};
use barrel_formats::elixir::Elixir;
use barrel_formats::game::Game;
use barrel_formats::magic;

const USAGE: &str = "\
usage:
  barrel game DIR              which game an install directory holds
  barrel ls FILE.elixir.gz     list a container's entries
  barrel unpack FILE.elixir.gz OUTDIR
  barrel scan DIR              read every container under DIR and count what they hold
  barrel pak-extract PAK PART OUTDIR  write the entries whose names contain PART under OUTDIR
  barrel ebd FILE.ebd          print an event's actions as words
  barrel ebd-scan DIR          read every event under DIR and count its commands
  barrel pssg FILE             print a PSSG file's node tree
  barrel pssg-obj FILE OUT.obj  write a PSSG file's meshes as OBJ
  barrel pssg-textures FILE OUTDIR  write a PSSG file's textures as PNG
  barrel walk FIELD_DIR X Y Z DX DZ STEPS  walk a character against a field's walls
  barrel g1co-scan DIR         read and check every field's collision
  barrel pssg-scan DIR         read every PSSG file under DIR (Ayesha DX) and count its node kinds
  barrel ebd-decode DIR        decode every event under DIR into commands (the installed game's table)
  barrel xml FILE.xml          print a table's rows
  barrel xml-scan DIR          read every XML table under DIR and list what it holds
  barrel g1t FILE OUTDIR       write a texture pack's textures as PNG (FILE.elixir.gz#N for an entry)
  barrel g1t-scan DIR          read every texture, loose and in containers
  barrel g1m-obj FILE#N OUT.obj  write a model's submeshes as Wavefront OBJ
  barrel g1m-bones FILE#N      a model's chunks, joints and submesh bone palettes
  barrel g1m-materials FILE#N  a model's materials: each texture's index, kind and subtype, and which submeshes use it
  barrel g1h FILE#N            list a morph file's sets and targets
  barrel g1h-scan DIR          read every morph file in the containers under DIR
  barrel g1m-scan DIR          read every model in the containers under DIR
  barrel oid-scan DIR          read every *Oid.bin bone-name list in the containers
  barrel ebm FILE.ebm          print an event's messages
  barrel ebm-scan DIR          read every message file under DIR
  barrel opl2-scan DIR|PAK...  read every field placement list in the containers (a folder's, or
                               in PAK archives: Sophie 2, the Ryza games)
  barrel ryza-field INSTALL [FIELD]  a Ryza DX field loaded from the archives into a scene (no
                               FIELD: every field, one line each)
  barrel opl2 FILE#N              a field's placements: each object, its instances and their tags, then its effects
  barrel ktf-scan DIR          read every field terrain in the containers
  barrel swg FILE#N OID#N      a character's swing settings, bones named from its Oid list
  barrel g1a FILE#N [BONE]     list an animation's tracks (bone, kind, values over time); with BONE, its pose every 0.1 s
  barrel g1a-moves FILE#N [OID#N]  which bones an animation turns, and how far (names from a bone list)
  barrel g1a-scan DIR          read every animation in the containers
  barrel pak-ls FILE.PAK       list a PAK archive's files (Sophie 2, Ryza 1-3)
  barrel pak-get FILE.PAK NAME OUT  extract one file from a PAK archive
  barrel pak-scan FILE.PAK...  read every texture, model, animation, morph, event and message in archives
  barrel xml-e FILE.xml.e OUT.xml  decrypt an encrypted table (Sophie 2, Ryza)
  barrel pak-xml-e FILE.PAK    decrypt every encrypted table in an archive, compared with its plain twin
  barrel faces EXE             list each character's facial animation names (by CharaBase row)
  barrel nut-parse ROOT        parse every Script/*.nut with Barrel's Squirrel front end
  barrel nut-run ROOT FILE.nut FRAMES  run a creature script on a creature 600 units from the player, printing its changes
  barrel new-game ROOT [escha|logy]  a new game run headless: each scene in turn (events by name with the field they leave, then free play)
  barrel event-check ROOT EVENT  an event's start conditions: its 14 words and each group's conditions and triggers
  barrel event-chain ROOT START [K.M>K.M|sleep:N|mix:N|travel:N|trip:FROM>TO ...]  follow the story from an event in a new game: each event's end starts the next; then the steps
  barrel event-starts ROOT GAMEDATA|new GROUP [VALUES...]  the events a save (or a new game) would start when a trigger group fires
  barrel subjects ROOT [GAMEDATA]  the assignments: card, square, what they count; with a save, its state of each
  barrel costumes ROOT [SYSDATA]  the outfits: each body's model; with a settings save, who wears what
  barrel field-sim ROOT AREA FRAMES  an area's field enemies under their scripts while a player walks past
  barrel gather INSTALL [LIST] what gathering gives: each list's items, weights, qualities and traits
  barrel gather-sim INSTALL POINT [SAVE]  one gather at a point (BARREL_TABLE_LEVEL, BARREL_PICKS, BARREL_SEED)
  barrel town-talk EXE ID       one town person's lines (their placement's talk id)
  barrel town-people EXE [MAP [GAMEDATA]]  who the field load places on each map, and the events it waits for (with a save: which pass)
  barrel world-points EXE       the world map's points and the field maps they open
  barrel world-routes EXE       the world map's routes and the marks at their ends
  barrel world-places EXE       the world map's places: tags, route links, names, descriptions
  barrel travel EXE FROM TO     a trip on the world map between two marks (T0, A0, A0p0 ...): routes, days, fields
  barrel gimmicks EXE [MAP]    each map's gimmicks (towns 0-11, field areas from 12): kind, place, size, event window
  barrel import-save INSTALL GAMEDATA.pcsave   the game's save as a Barrel save (stdout), for its Load Game
  barrel routes EXE [SAVE]     the world map's routes and the days each trip takes (now, from a save)
  barrel enum EXE NAME         number the names of the enum that NAME belongs to (e.g. EVENT_MM01_OP1_111)
  barrel board INSTALL DAY [--events] [--offset=N]  the request board drawn on a game day (--events: every
      request's start event seen; --offset: the save's board seed offset, global 0x1410989f0, default 0)
  barrel days INSTALL N [--events]  a new game's first N days (shops emptied at the start): board redraws, month closes, restocks
  barrel save GAMEDATA.pcsave   a save's plain header (the body is encrypted, not read yet)
  barrel vorbis-scan SeriesNNNN.fpl|DIR  read every clip's (or every .kvs in DIR) Vorbis headers (identification and setup) in a voice bank
  barrel voice-wav SeriesNNNN.fpl CLIP OUT.wav [OUT.ogg]  decode one clip natively to 16-bit WAV (and its Ogg)
  barrel stream-check FILE.kvs   checks packet-at-a-time decoding (and seeking to the loop) against a whole decode
  barrel font-text EXE FONT.g1t OUT.png TEXT   lays out TEXT in the game's font (glyph table from EXE)
  barrel layout-png ROOT OUT.png ID...   draws interface layouts over each other
  barrel sprite-png ROOT OUT.png PART    draws every cut of an interface sprite, one under another
  barrel layouts EXE            every interface layout: its nodes and their texture pieces
  barrel telops EXE             the narration cards (u55): each one's layout and texture pieces
  barrel g1s FILE.g1s [N]       a shader pack's programs: stage, inputs, outputs, constants, textures (N: program N's code)
  barrel se-scan BANK.wh2       a sound effect bank's sounds and their sample blocks
  barrel quests INSTALL         the requests: target, points, prizes, the days they are offered
  barrel shops INSTALL [LANG]  every shop's wares: price, quality, stock and refills, sale window
  barrel battle EXE MONSTER p<member>:<level>...  party members against a monster, with their learnt skills and MP
  barrel item-effects EXE      each item effect's battle effects (Saves/item/item_effect.xml next to EXE)
  barrel party EXE             each party member's base stats by level (HP/MP/ATK/DEF/SPD)
  barrel party-skills EXE      each party member's skill list: command, level learnt, MP cost
  barrel duel EXE A B [--equip NAME[@Q]]  two fighters (monster numbers, or p<member>:<level>) trading normal attacks: a partial battle
  barrel recipes EXE           list the synthesis recipes compiled into Escha & Logy DX's executable
  barrel synth INSTALL ITEM MATERIAL...  simulate a synthesis (item numbers; names from Saves_EN)
  barrel play-event FILE.ebd --chain [LANG]  the event, then each one the story starts after it
  barrel play-event FILE.ebd [LANG] [--timed]  run an event and print what happens (LANG: EN, SC, TC; default JP)
  barrel play-event FILE.ebd --stage [--fps HZ]  run an event on a clock and print where its characters and camera are
      (--fps: drive it as a renderer at HZ would, and print the end state)";

unsafe extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

fn main() -> ExitCode {
    // Die quietly on a closed pipe (`barrel ls ... | head`) instead of panicking.
    unsafe { signal(13, 0) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["game", dir] => game(Path::new(dir)),
        ["ls", file] => ls(Path::new(file)),
        ["unpack", file, out] => unpack(Path::new(file), Path::new(out)),
        ["scan", dir] => scan(Path::new(dir)),
        ["ebd", file] => ebd(Path::new(file)),
        ["ebd-scan", dir] => ebd_scan(Path::new(dir)),
        ["ebd-decode", dir] => ebd_decode(Path::new(dir)),
        ["pssg-scan", dir] => pssg_scan(Path::new(dir)),
        ["g1co-scan", dir] => g1co_scan(Path::new(dir)),
        ["walk", field_dir, x, y, z, dx, dz, steps] => walk(Path::new(field_dir), [x, y, z], [dx, dz], steps),
        ["pssg", file] => pssg_tree(Path::new(file)),
        ["pssg-meshes", file] => pssg_meshes(Path::new(file)),
        ["pssg-obj", file, out] => pssg_obj(Path::new(file), Path::new(out)),
        ["pssg-textures", file, out] => pssg_textures(Path::new(file), Path::new(out)),
        ["ebd-measure", dir] => ebd_measure(Path::new(dir)),
        ["voice-scan", dir] => voice_scan(Path::new(dir)),
        ["ebd-ops", dir, word] => ebd_ops(Path::new(dir), word, None),
        ["ebd-ops", dir, word, filter] => ebd_ops(Path::new(dir), word, Some(filter)),
        ["xml", file] => xml(Path::new(file)),
        ["xml-scan", dir] => xml_scan(Path::new(dir)),
        ["g1t", file, out] => g1t(file, Path::new(out)),
        ["g1t-scan", dir] => g1t_scan(Path::new(dir)),
        ["g1m-obj", file, out] => g1m_obj(file, Path::new(out)),
        ["g1m-materials", file] => g1m_materials(file),
        ["g1m-bones", file] => g1m_bones(file),
        ["g1h", file] => g1h(file),
        ["g1h-scan", dir] => g1h_scan(Path::new(dir)),
        ["g1m-scan", dir] => g1m_scan(Path::new(dir)),
        ["oid-scan", dir] => oid_scan(Path::new(dir)),
        ["ebm", file] => ebm(Path::new(file)),
        ["voice-map", install] => voice_map(Path::new(install)),
        ["ebm-scan", dir] => ebm_scan(Path::new(dir)),
        ["opl2-scan", dir] => opl2_scan(Path::new(dir)),
        ["opl2-scan", paks @ ..] => opl2_scan_paks(paks),
        ["opl2", file] => opl2_list(file),
        ["ryza-field", root] => ryza_field(Path::new(root), None),
        ["ryza-field", root, field] => ryza_field(Path::new(root), Some(field)),
        ["ktf-scan", dir] => ktf_scan(Path::new(dir)),
        ["swg", file, oid] => swg(file, oid),
        ["g1a", file] => g1a(file, None),
        ["g1a", file, bone] => bone.parse().map_err(|_| format!("{bone}: not a bone number").into()).and_then(|b| g1a(file, Some(b))),
        ["g1a-moves", file, rest @ ..] => g1a_moves(file, rest.first().copied()),
        ["g1a-scan", dir] => g1a_scan(Path::new(dir)),
        ["pak-ls", file] => pak_ls(Path::new(file)),
        ["pak-get", file, name, out] => pak_get(Path::new(file), name, Path::new(out)),
        ["pak-extract", file, part, out] => pak_extract(Path::new(file), part, Path::new(out)),
        ["pak-scan", files @ ..] if !files.is_empty() => pak_scan(files),
        ["pak-bc6h", files @ ..] if !files.is_empty() => pak_bc6h(files),
        ["xml-e", file, out] => xml_e(Path::new(file), Path::new(out)),
        ["pak-xml-e", file] => pak_xml_e(Path::new(file)),
        ["faces", exe] => faces(Path::new(exe)),
        ["enum", exe, name] => enum_list(Path::new(exe), name),
        ["gather", install] => gather(Path::new(install), None),
        ["gather", install, list] => match list.parse() {
            Ok(n) => gather(Path::new(install), Some(n)),
            Err(_) => Err("the list is a number".into()),
        },
        ["gather-sim", install, point] => gather_sim(Path::new(install), point, None),
        ["gather-sim", install, point, save] => gather_sim(Path::new(install), point, Some(Path::new(save))),
        ["town-talk", exe, id] => match id.parse::<i32>() {
            Ok(id) => town_talk(Path::new(exe), id),
            Err(_) => Err("ID is a number".into()),
        },
        ["town-people", exe] => town_people(Path::new(exe), None, None),
        ["town-people", exe, map] => match map.parse() {
            Ok(m) => town_people(Path::new(exe), Some(m), None),
            Err(_) => Err("MAP is a number".into()),
        },
        ["town-people", exe, map, save] => match map.parse() {
            Ok(m) => town_people(Path::new(exe), Some(m), Some(Path::new(save))),
            Err(_) => Err("MAP is a number".into()),
        },
        ["world-points", exe] => world_points(Path::new(exe)),
        ["world-routes", exe] => world_routes(Path::new(exe)),
        ["world-places", exe] => world_places(Path::new(exe)),
        ["travel", exe, from, to] => travel(Path::new(exe), from, to),
        ["gimmicks", exe] => gimmicks(Path::new(exe), None),
        ["gimmicks", exe, area] => match area.parse() {
            Ok(n) => gimmicks(Path::new(exe), Some(n)),
            Err(_) => Err("the area is a number".into()),
        },
        ["import-save", install, save] => import_save(Path::new(install), Path::new(save)),
        ["routes", exe] => routes(Path::new(exe), None),
        ["routes", exe, save] => routes(Path::new(exe), Some(Path::new(save))),
        ["gestures", exe, numbers @ ..] => gestures(Path::new(exe), numbers),
        ["duel", exe, a, b] => duel(Path::new(exe), a, b, None),
        ["battle", exe, monster, "--from", save] => battle_from_save(Path::new(exe), monster, Path::new(save)),
        ["battle", exe, monster, party @ ..] if !party.is_empty() => battle(Path::new(exe), monster, party),
        ["duel", exe, a, b, "--equip", item] => duel(Path::new(exe), a, b, Some(item)),
        ["party", exe] => party(Path::new(exe)),
        ["item-effects", exe] => item_effects(Path::new(exe)),
        ["acts", exe] => acts(Path::new(exe)),
        ["trait-kinds", exe] => trait_kinds(Path::new(exe)),
        ["macros", root] => macros(Path::new(root)),
        ["nut-parse", root] => nut_parse(Path::new(root)),
        ["event-chain", root, start, walks @ ..] => event_chain(Path::new(root), start, walks),
        ["event-check", root, event] => event_check(Path::new(root), event),
        ["new-game", root, rest @ ..] => new_game(Path::new(root), rest.first() == Some(&"logy")),
        ["event-starts", root, save, group, values @ ..] => event_starts(Path::new(root), Path::new(save), group, values),
        ["subjects", root] => subjects(Path::new(root), None),
        ["subjects", root, save] => subjects(Path::new(root), Some(Path::new(save))),
        ["costumes", root] => costumes(Path::new(root), None),
        ["costumes", root, sysdata] => costumes(Path::new(root), Some(Path::new(sysdata))),
        ["field-sim", root, area, frames] => field_sim(Path::new(root), area, frames.parse().unwrap_or(600)),
        ["nut-run", root, file, ticks] => nut_run(Path::new(root), file, ticks.parse().unwrap_or(1)),
        ["party-skills", exe] => party_skills(Path::new(exe)),
        ["monsters", exe] => monsters(Path::new(exe)),
        ["encounters", exe] => encounters(Path::new(exe)),
        ["field-enemies", exe] => field_enemies(Path::new(exe)),
        ["encounter", exe, area, "--from", save] => encounter(Path::new(exe), area, "0", Path::new(save)),
        ["encounter", exe, area, "--from", save, which] => encounter(Path::new(exe), area, which, Path::new(save)),
        ["quests", install] => quests(Path::new(install)),
        ["deliveries", install] => deliveries(Path::new(install)),
        ["quakes", install] => quakes(Path::new(install)),
        ["vorbis-scan", fpl] => vorbis_scan(Path::new(fpl)),
        ["se-scan", wh2] => se_scan(Path::new(wh2)),
        ["g1s", file] => g1s(Path::new(file), None),
        ["g1s", file, n] => match n.parse() {
            Ok(n) => g1s(Path::new(file), Some(n)),
            Err(_) => Err("g1s: the program number is a number".into()),
        },
        ["telops", exe] => telops(Path::new(exe)),
        ["layouts", exe] => layouts(Path::new(exe)),
        ["layout-png", root, out, ids @ ..] if !ids.is_empty() => match ids.iter().map(|v| v.parse()).collect::<std::result::Result<Vec<u32>, _>>() {
            Ok(ids) => layout_png(Path::new(root), &ids, Path::new(out)),
            Err(e) => Err(format!("layout id: {e}").into()),
        },
        ["sprite-png", root, out, part] => match i64::from_str_radix(part.trim_start_matches("0x"), if part.starts_with("0x") { 16 } else { 10 }) {
            Ok(part) => sprite_png(Path::new(root), part as i32, Path::new(out)),
            Err(e) => Err(format!("part: {e}").into()),
        },
        ["font-text", exe, g1t, out, text] => font_text(Path::new(exe), Path::new(g1t), Path::new(out), text),
        ["stream-check", kvs] => stream_check(Path::new(kvs)),
        ["voice-wav", fpl, clip, out, rest @ ..] if rest.len() <= 1 => voice_wav(Path::new(fpl), clip, Path::new(out), rest.first().map(Path::new)),
        ["save", file] => save(Path::new(file)),
        ["bonuses", file] => bonuses(Path::new(file), None),
        ["bonuses", file, exe] => bonuses(Path::new(file), Some(Path::new(exe))),
        ["days", install, n] => days(Path::new(install), n, false, None),
        ["days", install, n, "--events"] => days(Path::new(install), n, true, None),
        ["days", install, n, "--from", save] => days(Path::new(install), n, false, Some(Path::new(save))),
        ["board", install, day, rest @ ..] if rest.iter().all(|a| *a == "--events" || a.starts_with("--offset=")) => {
            board(Path::new(install), day, rest.contains(&"--events"), rest.iter().find_map(|a| a.strip_prefix("--offset=")))
        }
        ["shops", install] => shops(Path::new(install), None),
        ["shops", install, lang] => shops(Path::new(install), Some(lang)),
        ["recipes", exe] => recipes(Path::new(exe)),
        ["cauldron", install, items @ ..] if !items.is_empty() => cauldron(Path::new(install), items),
        ["synth", install, item, materials @ ..] if !materials.is_empty() => {
            synth(Path::new(install), item, materials)
        }
        ["play-macro", script, name, "--timed", args @ ..] => play_macro(Path::new(script), name, args, true),
        ["play-macro", script, name, args @ ..] => play_macro(Path::new(script), name, args, false),
        ["play-event", file, "--stage"] => stage_event(Path::new(file), None),
        ["play-event", file, "--stage", "--fps", hz] => stage_event(Path::new(file), Some(hz)),
        ["play-event", file, "--chain"] => play_chain(Path::new(file), None),
        ["play-event", file, "--chain", lang] => play_chain(Path::new(file), Some(lang)),
        ["play-event", file] => play_event(Path::new(file), None, false),
        ["play-event", file, "--timed"] => play_event(Path::new(file), None, true),
        ["play-event", file, lang] => play_event(Path::new(file), Some(lang), false),
        ["play-event", file, lang, "--timed"] => play_event(Path::new(file), Some(lang), true),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("barrel: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()).into())
}

fn game(dir: &Path) -> Result<()> {
    match Game::detect(dir) {
        Some(g) => println!("{}", g.title()),
        None => return Err("no known Atelier game in that directory".into()),
    }
    Ok(())
}

fn kind(data: &[u8]) -> &'static str {
    magic::identify(data).unwrap_or("?")
}

fn ls(file: &Path) -> Result<()> {
    let bytes = read(file)?;
    let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", file.display()))?;
    println!(
        "version {}, flags {}, {} entries{}",
        e.version,
        e.flags,
        e.entries.len(),
        if e.compressed { ", zlib-chunked" } else { "" }
    );
    for entry in &e.entries {
        if entry.is_dummy() {
            println!("{:>10}  {:<18}{}", 0, "(dummy)", entry.name);
        } else {
            println!("{:>10}  {:<18}{}", entry.size, kind(e.data(entry)), entry.name);
        }
    }
    Ok(())
}

/// A file name for an entry that cannot escape the output directory.
fn safe_name(name: &str, index: usize) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .collect();
    let cleaned = cleaned.trim_start_matches('.');
    if cleaned.is_empty() { format!("entry{index}") } else { format!("{index:03}_{cleaned}") }
}

fn unpack(file: &Path, out: &Path) -> Result<()> {
    let bytes = read(file)?;
    let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", file.display()))?;
    std::fs::create_dir_all(out)?;
    let mut written = 0;
    for (i, entry) in e.entries.iter().enumerate() {
        if entry.is_dummy() {
            continue;
        }
        std::fs::write(out.join(safe_name(&entry.name, i)), e.data(entry))?;
        written += 1;
    }
    println!("{written} entries written to {}", out.display());
    Ok(())
}

fn files_ending(dir: &Path, suffix: &str, found: &mut Vec<PathBuf>) -> Result<()> {
    for item in std::fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() {
            files_ending(&path, suffix, found)?;
        } else if path.to_string_lossy().ends_with(suffix) {
            found.push(path);
        }
    }
    Ok(())
}

fn sjis(bytes: &[u8]) -> String {
    encoding_rs::SHIFT_JIS.decode_without_bom_handling(bytes).0.into_owned()
}

fn ebd(file: &Path) -> Result<()> {
    let bytes = read(file)?;
    let e = Ebd::parse(&bytes).map_err(|err| format!("{}: {err}", file.display()))?;
    for (i, a) in e.actions.iter().enumerate() {
        println!("action {i} \"{}\" ({}), {} words", sjis(&a.name), sjis(&a.description), a.words.len());
        let words: Vec<String> = a.words.iter().map(|&w| Word::classify(w).to_string()).collect();
        println!("  {}", words.join(" "));
    }
    for r in &e.resources {
        println!("resource \"{}\"", sjis(r));
    }
    Ok(())
}

fn ebd_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".ebd", &mut files)?;
    files.sort();
    let (mut actions, mut words) = (0, 0);
    let mut commands: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        match Ebd::parse(&bytes) {
            Ok(e) => {
                for a in &e.actions {
                    actions += 1;
                    words += a.words.len();
                    for &w in &a.words {
                        if let c @ Word::Command { .. } = Word::classify(w) {
                            *commands.entry(c.to_string()).or_default() += 1;
                        }
                    }
                }
            }
            Err(err) => failed.push(format!("{}: {err}", path.display())),
        }
    }
    println!(
        "{} events: {} read, {} failed; {actions} actions, {words} words, {} distinct commands",
        files.len(),
        files.len() - failed.len(),
        failed.len(),
        commands.len()
    );
    let mut commands: Vec<_> = commands.into_iter().collect();
    commands.sort_by_key(|c| std::cmp::Reverse(c.1));
    for (c, n) in commands.iter().take(40) {
        println!("{n:>7}  {c}");
    }
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    Ok(())
}

fn scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut ok, mut compressed, mut entries, mut dummies) = (0, 0, 0, 0);
    let mut versions: BTreeMap<u32, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        match Elixir::parse(&bytes) {
            Ok(e) => {
                ok += 1;
                compressed += e.compressed as usize;
                *versions.entry(e.version).or_default() += 1;
                for entry in &e.entries {
                    entries += 1;
                    if entry.is_dummy() {
                        dummies += 1;
                        continue;
                    }
                    let data = e.data(entry);
                    let k = match magic::identify(data) {
                        Some(k) => k.to_string(),
                        None => {
                            let ext = entry.name.rsplit_once('.').map_or("", |(_, x)| x);
                            format!("? .{ext}")
                        }
                    };
                    let slot = kinds.entry(k).or_default();
                    slot.0 += 1;
                    slot.1 += data.len() as u64;
                }
            }
            Err(err) => failed.push(format!("{}: {err}", path.display())),
        }
    }
    println!(
        "{} containers: {ok} read ({compressed} zlib-chunked), {} failed; versions {versions:?}",
        files.len(),
        failed.len()
    );
    println!("{entries} entries, {dummies} dummies");
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1.0));
    for (k, (n, bytes)) in kinds {
        println!("{n:>7}  {:>8.1} MB  {k}", bytes as f64 / 1e6);
    }
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    Ok(())
}

/// Measure an operand table from every event under `dir` (another game's) and print it as a
/// Rust table, with how many bodies it decodes.
fn ebd_measure(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".ebd", &mut files)?;
    files.sort();
    let events: Vec<Ebd> = files.iter().filter_map(|p| read(p).ok()).filter_map(|b| Ebd::parse(&b).ok()).collect();
    let table = OperandTable::measure(events.iter().flat_map(|e| e.actions.iter().map(|a| &a.words[..])));
    let (mut ok, mut bad) = (0, 0);
    for e in &events {
        for a in &e.actions {
            if ebd::decode(&a.words, &table).is_ok() { ok += 1 } else { bad += 1 }
        }
    }
    let entries = table.entries();
    let varying = entries.iter().filter(|e| e.1.is_none()).count();
    eprintln!("{} events, {} commands ({varying} varying); {ok} bodies decode, {bad} do not", events.len(), entries.len());
    for (w, c) in entries {
        let short = Word::classify(w).to_string();
        println!("    (0x{w:08x}, {}), // {short}", c.map_or("None".to_string(), |n| format!("Some({n})")));
    }
    Ok(())
}

/// Prints a PSSG file's node tree: each node's kind and attributes (as a string when they read
/// as one, else as numbers), and the size of its data.
fn pssg_tree(file: &Path) -> Result<()> {
    let raw = read(file)?;
    let d = barrel_formats::pssg::unwrap(&raw)?;
    let p = barrel_formats::pssg::Pssg::parse(&d)?;
    fn show(p: &barrel_formats::pssg::Pssg, n: &barrel_formats::pssg::Node, depth: usize) {
        let attrs = p.schema.nodes.get(&n.kind).map(|k| &k.1);
        let shown: Vec<String> = n
            .attributes
            .iter()
            .map(|(id, b)| {
                let name = attrs.and_then(|a| a.get(id)).map_or_else(|| p.attribute_name(*id), |s| s.as_str());
                let value = if b.len() > 4 && b[..4] == ((b.len() - 4) as u32).to_be_bytes() && b[4..].iter().all(|c| (0x20..0x7f).contains(c)) {
                    format!("{:?}", String::from_utf8_lossy(&b[4..]))
                } else if b.len() == 4 {
                    let v = u32::from_be_bytes(b[..4].try_into().unwrap());
                    let f = f32::from_bits(v);
                    if v < 0x0100_0000 { v.to_string() } else { format!("{f}") }
                } else {
                    format!("<{} bytes>", b.len())
                };
                format!("{name}={value}")
            })
            .collect();
        let data = if n.data.is_empty() { String::new() } else { format!(" [{} bytes]", n.data.len()) };
        println!("{}{} {}{data}", "  ".repeat(depth), p.name(n.kind), shown.join(" "));
        for c in &n.children {
            show(p, c, depth + 1);
        }
    }
    show(&p, &p.root, 0);
    Ok(())
}

/// A PSSG file's meshes: their triangles and vertex streams.
fn pssg_meshes(file: &Path) -> Result<()> {
    let raw = read(file)?;
    let d = barrel_formats::pssg::unwrap(&raw)?;
    let p = barrel_formats::pssg::Pssg::parse(&d)?;
    for m in p.meshes() {
        let streams: Vec<String> = m.streams.iter().map(|(t, v)| format!("{t}×{}", v.len())).collect();
        println!("{} {} triangles: {}", m.name, m.indices.len() / 3, streams.join(", "));
    }
    Ok(())
}

/// Writes a PSSG file's meshes as Wavefront OBJ (positions, texture coordinates and normals
/// where the mesh has them; each mesh a group), each placed by the node that draws it (normals
/// left as they are).
fn pssg_obj(file: &Path, out: &Path) -> Result<()> {
    use std::fmt::Write as _;
    let raw = read(file)?;
    let d = barrel_formats::pssg::unwrap(&raw)?;
    let p = barrel_formats::pssg::Pssg::parse(&d)?;
    let mut s = String::new();
    let mut base = 1usize;
    let placed = p.placements();
    for m in p.meshes() {
        // Placed by the node that draws it (the first, when several do), else as it is.
        let world = placed.iter().find(|(src, _)| *src == m.name).map(|(_, w)| *w);
        let pos: Vec<Vec<f32>> = m
            .positions()
            .iter()
            .map(|v| match world {
                Some(w) => (0..3).map(|c| v[0] * w[c] + v[1] * w[4 + c] + v[2] * w[8 + c] + w[12 + c]).collect(),
                None => v.clone(),
            })
            .collect();
        let pos = &pos[..];
        let uv = m.stream("ST").filter(|v| v.len() == pos.len());
        let nrm = m.stream("Normal").or_else(|| m.stream("SkinnableNormal")).filter(|v| v.len() == pos.len());
        writeln!(s, "g {}", m.name)?;
        for v in pos {
            writeln!(s, "v {} {} {}", v[0], v[1], v[2])?;
        }
        for t in uv.unwrap_or(&[]) {
            writeln!(s, "vt {} {}", t[0], 1.0 - t[1])?;
        }
        for n in nrm.unwrap_or(&[]) {
            writeln!(s, "vn {} {} {}", n[0], n[1], n[2])?;
        }
        let corner = |i: u32| {
            let k = base + i as usize;
            match (uv.is_some(), nrm.is_some()) {
                (true, true) => format!("{k}/{k}/{k}"),
                (true, false) => format!("{k}/{k}"),
                (false, true) => format!("{k}//{k}"),
                (false, false) => k.to_string(),
            }
        };
        for t in m.indices.chunks_exact(3) {
            writeln!(s, "f {} {} {}", corner(t[0]), corner(t[1]), corner(t[2]))?;
        }
        base += pos.len();
    }
    std::fs::write(out, s)?;
    Ok(())
}

/// Writes a PSSG file's textures as PNG files under OUT (their largest level).
fn pssg_textures(file: &Path, out: &Path) -> Result<()> {
    let raw = read(file)?;
    let d = barrel_formats::pssg::unwrap(&raw)?;
    let p = barrel_formats::pssg::Pssg::parse(&d)?;
    std::fs::create_dir_all(out)?;
    for t in p.textures() {
        let name = t.name.replace(['/', '\\'], "_");
        match t.to_rgba8() {
            Some(px) => {
                std::fs::write(out.join(format!("{name}.png")), barrel_formats::png::encode_rgba(t.width, t.height, &px))?;
                println!("{name}: {}×{} {}", t.width, t.height, t.format);
            }
            None => println!("{name}: {}×{} {} (format not read)", t.width, t.height, t.format),
        }
    }
    Ok(())
}

/// Walks the player (radius 48 and 150 tall: `FMrad` and `FMhig` in her `CharaBase.xml` row) from (X, Y, Z) by (DX, DZ) each step for STEPS
/// steps against FIELD_DIR's walls, on its ground (`game::walk`), printing where it stands.
fn walk(dir: &Path, at: [&str; 3], delta: [&str; 2], steps: &str) -> Result<()> {
    use barrel_engine::walk::Field;
    let fields = dir.parent().ok_or("FIELD_DIR has no parent folder")?;
    // The maps' names and gimmicks, from the executable beside the install's `Data` folder (none
    // without it: the walk then only follows walls and ground).
    let exe = dir.ancestors().map(|a| a.join("Atelier_Escha_and_Logy_EN.exe")).find(|e| e.is_file()).map(std::fs::read).transpose()?;
    let pe = exe.as_deref().map(barrel_formats::pe::Pe::parse).transpose()?;
    let maps = pe.as_ref().map(barrel_escha::data::maps::field_maps).unwrap_or_default();
    let areas = pe.as_ref().and_then(|pe| barrel_escha::data::gimmick::areas(pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT)).unwrap_or_default();
    use barrel_escha::data::gimmick::inside as inside_of;
    let open = |name: &str| -> Result<(Field, Vec<barrel_escha::data::gimmick::Gimmick>)> {
        let gimmicks = maps.iter().position(|n| n == name).and_then(|m| areas.get(m)).map(|a| a.gimmicks.clone()).unwrap_or_default();
        Ok((Field::load(fields, name)?, gimmicks))
    };
    let (mut field, mut gimmicks) = open(dir.file_name().and_then(|n| n.to_str()).ok_or("no field name")?)?;
    let [x, y, z] = at.map(|v| v.parse::<f32>());
    let (mut p, mut y) = ([x?, z?], y?);
    let d = [delta[0].parse::<f32>()?, delta[1].parse::<f32>()?];
    let mut inside = inside_of(&gimmicks, [p[0], y, p[1]]);
    for k in 0..steps.parse::<usize>()? {
        let next = field.walls.step(p, d, y, 150.0, 48.0);
        let blocked = (next[0] - p[0] - d[0]).abs() > 0.01 || (next[1] - p[1] - d[1]).abs() > 0.01;
        // Stand on the ground there (a step up of at most 50); nothing under it: stay put.
        match field.ground.height(next[0], next[1], y, 50.0) {
            Some(h) => {
                y = h;
                p = next;
            }
            None => {
                println!("{k:4}: ({:.1}, {:.1}) no ground ahead, stays", p[0], p[1]);
                continue;
            }
        }
        println!("{k:4}: ({:.1}, {:.1}, {y:.1}){}", p[0], p[1], if blocked { "  (against a wall)" } else { "" });
        let now = inside_of(&gimmicks, [p[0], y, p[1]]);
        let entered: Vec<usize> = now.iter().copied().filter(|i| !inside.contains(i)).collect();
        inside = now;
        for &i in &entered {
            println!("      enters {} at {:?}", gimmicks[i].kind_name(), gimmicks[i].position);
        }
        // A door: on to its map, standing where it says (metres to field units).
        if let Some(to) = entered.iter().find_map(|&i| gimmicks[i].destination) {
            let name = maps.get(to.map as usize).ok_or(format!("door to unknown map {}", to.map))?.clone();
            (field, gimmicks) = open(&name)?;
            let at = to.position.map(|v| v * 100.0);
            p = [at[0], at[2]];
            y = field.ground.height(p[0], p[1], at[1] + 50.0, 0.0).unwrap_or(at[1]);
            inside = inside_of(&gimmicks, [p[0], y, p[1]]);
            println!("      through the door to {name} (map {}) at ({:.1}, {:.1}, {y:.1}) facing {}", to.map, p[0], p[1], to.facing);
        }
    }
    Ok(())
}

/// The world map's routes with their ends (as their dots lie), from the executable and the
/// world map field beside it.
fn world_map(exe: &Path) -> Result<barrel_escha::travel::WorldMap> {
    let bytes = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    let field = read(&exe.parent().ok_or("no folder")?.join("Data/x64/Field/A15_WMAP/A15_WMAP.elixir.gz"))?;
    let c = Elixir::parse(&field)?;
    let e = c.entries.iter().find(|e| e.name.ends_with(".opl2")).ok_or("no placements")?;
    let placements = barrel_formats::opl2::parse(c.data(e))?;
    Ok(barrel_escha::travel::WorldMap::load(&pe, &placements).ok_or("no routes")?)
}

/// A trip on the world map from mark FROM to mark TO (`T0`, `A0`, `A0p0` ...) in a new game
/// (no events seen): the routes, the days, and the fields TO opens.
fn travel(exe: &Path, from: &str, to: &str) -> Result<()> {
    let map = world_map(exe)?;
    let (days, path) = map.trip(from, to, |_| false).ok_or(format!("no way from {from} to {to}"))?;
    println!("{from} to {to}: {days} days by {}", path.join(", "));
    let bytes = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
    let maps = barrel_escha::data::maps::field_maps(&pe);
    for (point, m, _) in barrel_escha::data::world::point_maps(&pe).unwrap_or_default() {
        let name = names.get(point as usize).map_or("", |s| s.as_str());
        if barrel_escha::data::world::point_tag(name).as_deref() == Some(to) {
            println!("  opens map {m} {}", maps.get(m as usize).map_or("?", |s| s.as_str()));
        }
    }
    Ok(())
}

/// The world map's routes and the marks at their ends, as their dots lie
/// (`barrel_escha::data::world::route_ends`), with each route's days.
fn world_routes(exe: &Path) -> Result<()> {
    for r in world_map(exe)?.routes {
        println!("{:6} {:>5} - {:<5} ({} days)", r.area.id, r.ends.0, r.ends.1, r.area.days[0].0);
    }
    Ok(())
}

/// The world map's points and the field maps they open (`barrel_escha::data::world::point_maps`).
fn world_points(exe: &Path) -> Result<()> {
    let bytes = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
    let maps = barrel_escha::data::maps::field_maps(&pe);
    // Where each point stands, from the world map field's placements beside the executable.
    let placements = exe
        .parent()
        .map(|d| d.join("Data/x64/Field/A15_WMAP/A15_WMAP.elixir.gz"))
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| {
            let c = Elixir::parse(&bytes).ok()?;
            let e = c.entries.iter().find(|e| e.name.ends_with(".opl2"))?;
            barrel_formats::opl2::parse(c.data(e)).ok()
        });
    for (point, map, event) in barrel_escha::data::world::point_maps(&pe).ok_or("no point table")? {
        let name = names.get(point as usize).map_or("?", |s| s.as_str());
        let field = maps.get(map as usize).map_or("?", |s| s.as_str());
        let after = if event >= 0 { format!(" after event {event}") } else { String::new() };
        let at = placements.as_ref().and_then(|p| barrel_escha::data::world::point_position(p, name)).map(|p| format!(" at ({:.0}, {:.0}, {:.0})", p[0], p[1], p[2])).unwrap_or_default();
        println!("point {point:3} {name:10} -> map {map:3} {field}{after}{at}");
    }
    Ok(())
}

/// Lists the world map's places (`world::places`): tag, kind, links (route numbers by way:
/// right, left, down, up, two more), the two words at +0x48, name and description.
fn world_places(exe: &Path) -> Result<()> {
    let bytes = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    for (k, p) in barrel_escha::data::world::places(&pe, barrel_escha::data::world::PLACES_EN).ok_or("no place table")?.iter().enumerate() {
        println!("{k:2} {:6} kind {} links {:?} extra {:?} {} | {}", p.tag, p.kind, p.links, p.extra, p.name, p.description.replace('\n', " "));
    }
    Ok(())
}

/// Reads every field's collision (`*_col.g1co` in the containers under DIR) and checks its
/// hierarchy.
fn g1co_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut read_ok, mut failed, mut triangles, mut vertices) = (0usize, Vec::new(), 0usize, 0usize);
    for f in &files {
        let bytes = read(f)?;
        let Ok(container) = Elixir::parse(&bytes) else { continue };
        for e in &container.entries {
            if !e.name.ends_with(".g1co") {
                continue;
            }
            match barrel_formats::g1co::parse(container.data(e)) {
                Ok(c) => match c.check() {
                    None => {
                        read_ok += 1;
                        triangles += c.triangles.len();
                        vertices += c.vertices.len();
                    }
                    Some(why) => failed.push(format!("{} {}: {why}", f.display(), e.name)),
                },
                Err(err) => failed.push(format!("{} {}: {err}", f.display(), e.name)),
            }
        }
    }
    println!("{} collision files: {read_ok} read and consistent ({triangles} triangles, {vertices} vertices), {} not", read_ok + failed.len(), failed.len());
    for f in failed.iter().take(10) {
        println!("  {f}");
    }
    Ok(())
}

/// Reads every PSSG file under DIR (`*.PSSG`, `*.PSSG.gz`: Ayesha DX's) and counts the node
/// kinds they hold.
fn pssg_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".PSSG", &mut files)?;
    files_ending(dir, ".PSSG.gz", &mut files)?;
    files.sort();
    let (mut ok, mut failed) = (0, Vec::new());
    let (mut meshes, mut triangles, mut bad_meshes) = (0usize, 0usize, Vec::new());
    let mut texture_formats: std::collections::BTreeMap<String, usize> = Default::default();
    let mut bad_textures = 0usize;
    let (mut skins, mut skin_mismatch, mut missing_joints, mut bind_errors) = (0usize, 0usize, 0usize, Vec::<f32>::new());
    let mut posed_files: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut motions, mut channels, mut bad_channels) = (0usize, 0usize, Vec::<String>::new());
    let mut bad_kinds: std::collections::BTreeMap<String, usize> = Default::default();
    let mut lead_in = 0usize;
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for f in &files {
        let raw = read(f)?;
        let d = match barrel_formats::pssg::unwrap(&raw) {
            Ok(d) => d,
            Err(e) => {
                failed.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        match barrel_formats::pssg::Pssg::parse(&d) {
            Ok(p) => {
                ok += 1;
                let joints = p.joints();
                for skin in p.skins() {
                    skins += 1;
                    if skin.joints.len() != skin.inverse_binds.len() {
                        skin_mismatch += 1;
                        continue;
                    }
                    // In the bind pose each joint's world matrix undoes its inverse bind matrix.
                    for (j, inv) in skin.joints.iter().zip(&skin.inverse_binds) {
                        let Some(joint) = joints.iter().find(|k| k.name == *j) else {
                            missing_joints += 1;
                            continue;
                        };
                        let m = barrel_formats::pssg::mul(inv, &joint.world);
                        let off = m.iter().zip(barrel_formats::pssg::IDENTITY).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
                        bind_errors.push(off);
                        if off > 0.01 {
                            *posed_files.entry(f.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()).or_default() += 1;
                        }
                    }
                }
                for m in p.motions() {
                    motions += 1;
                    for c in &m.channels {
                        channels += 1;
                        // Keys before time 0 lead the curve in (facial motions start at -1/15 s).
                        if c.times.first().is_some_and(|&t| t < -0.001) {
                            lead_in += 1;
                        }
                        let ordered = c.times.windows(2).all(|t| t[0] <= t[1]) && c.times.iter().all(|&t| t <= m.length + 0.001);
                        let unit = c.kind != "Rotation" || c.values.iter().all(|q| (q.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 0.01);
                        let why = if c.times.len() != c.values.len() {
                            "uneven"
                        } else if !c.times.windows(2).all(|t| t[0] <= t[1]) {
                            "out of order"
                        } else if !ordered {
                            "past the length"
                        } else if !unit {
                            "rotation not unit"
                        } else {
                            ""
                        };
                        if !why.is_empty() {
                            *bad_kinds.entry(format!("{} {why}", c.kind)).or_default() += 1;
                            bad_channels.push(format!("{}: {} {} {} {why} (length {}, times {:?})", f.display(), m.name, c.target, c.kind, m.length, &c.times[..c.times.len().min(4)]));
                        }
                    }
                }
                for t in p.textures() {
                    *texture_formats.entry(t.format.clone()).or_default() += 1;
                    if t.to_rgba8().is_none() {
                        println!("unread texture format {} ({}x{}): {} in {}", t.format, t.width, t.height, t.name, f.display());
                    }
                    if t.to_rgba8().is_some_and(|px| px.len() != (t.width * t.height * 4) as usize) {
                        bad_textures += 1;
                    }
                }
                for m in p.meshes() {
                    meshes += 1;
                    triangles += m.indices.len() / 3;
                    let vertices = m.positions().len();
                    if m.indices.iter().any(|&i| i as usize >= vertices) {
                        bad_meshes.push(format!("{}: {} ({} vertices)", f.display(), m.name, vertices));
                    }
                }
                let mut stack = vec![&p.root];
                while let Some(n) = stack.pop() {
                    *kinds.entry(p.name(n.kind).to_string()).or_default() += 1;
                    stack.extend(n.children.iter());
                }
            }
            Err(e) => failed.push(format!("{}: {e}", f.display())),
        }
    }
    println!("{} PSSG files: {ok} read, {} failed", files.len(), failed.len());
    println!("{meshes} meshes, {triangles} triangles; {} with an index past their vertices", bad_meshes.len());
    bind_errors.sort_by(f32::total_cmp);
    let pct = |q: f32| bind_errors.get(((bind_errors.len() as f32 - 1.0) * q) as usize).copied().unwrap_or(0.0);
    println!(
        "{skins} skins ({skin_mismatch} with joints and matrices in other counts, {missing_joints} joints not found); inverse bind × joint world off the identity by at most: median {}, 99% {}, max {}",
        pct(0.5),
        pct(0.99),
        pct(1.0)
    );
    let off = bind_errors.iter().filter(|&&e| e > 0.01).count();
    println!("  {off} of {} joints off by more than 0.01, in {} files: {:?}", bind_errors.len(), posed_files.len(), posed_files.iter().take(8).collect::<Vec<_>>());
    println!("{motions} motions, {channels} channels; {} with keys out of order, past the motion's length, uneven or a rotation not of unit length", bad_channels.len());
    println!("  by kind and reason: {bad_kinds:?}; {lead_in} channels with keys before time 0");
    for b in bad_channels.iter().take(3) {
        println!("  {b}");
    }
    println!("textures by format: {texture_formats:?}; {bad_textures} that did not decode to their size");
    for b in bad_meshes.iter().take(5) {
        println!("  {b}");
    }
    let mut by: Vec<_> = kinds.into_iter().collect();
    by.sort_by_key(|k| std::cmp::Reverse(k.1));
    for (k, n) in by.iter().take(30) {
        println!("{n:>8}  {k}");
    }
    for f in failed.iter().take(10) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some files did not read".into()) }
}

/// The operand table for the game installed above `dir`: Shallie DX's, Ayesha DX's, Sophie 2's or Ryza DX's measured
/// ones, Escha & Logy DX's otherwise.
fn operand_table_for(dir: &Path) -> OperandTable {
    let has = |name: &str| dir.ancestors().any(|p| p.join(name).is_file());
    if has("Atelier_Shallie.exe") {
        OperandTable::shallie()
    } else if has("Atelier_Ayesha.exe") {
        OperandTable::ayesha()
    } else if has("Atelier_Sophie_2.exe") {
        OperandTable::sophie2()
    } else if has("Atelier_Ryza_DX.exe") {
        OperandTable::ryza()
    } else {
        OperandTable::escha()
    }
}

fn ebd_decode(dir: &Path) -> Result<()> {
    let table = operand_table_for(dir);
    let mut files = Vec::new();
    files_ending(dir, ".ebd", &mut files)?;
    files.sort();
    let (mut bodies, mut commands) = (0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        let e = match Ebd::parse(&bytes) {
            Ok(e) => e,
            Err(err) => {
                failed.push(format!("{}: {err}", path.display()));
                continue;
            }
        };
        for (i, a) in e.actions.iter().enumerate() {
            match ebd::decode(&a.words, &table) {
                Ok(cmds) => {
                    bodies += 1;
                    commands += cmds.len();
                }
                Err(err) => failed.push(format!("{} action {i}: {err}", path.display())),
            }
        }
    }
    println!("{} events: {bodies} bodies decoded into {commands} commands, {} failures", files.len(), failed.len());
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some bodies did not decode".into()) }
}

/// For one command (a hex word such as `74000004`), how often each value appears at each
/// operand position across every event under `dir`, the commonest first. `filter` (`N=V`)
/// keeps only the uses whose operand N (from 1) is V.
fn ebd_ops(dir: &Path, word: &str, filter: Option<&str>) -> Result<()> {
    let word = u32::from_str_radix(word.trim_start_matches("0x"), 16).map_err(|_| "the word is hex, e.g. 74000004")?;
    let filter = filter
        .map(|f| f.split_once('=').and_then(|(n, v)| Some((n.parse::<usize>().ok()?.checked_sub(1)?, v.parse::<i32>().ok()?))))
        .map(|f| f.ok_or("the filter is N=V, e.g. 9=3"))
        .transpose()?;
    let table = match std::env::var("BARREL_GAME").as_deref() { Ok("sophie2") => OperandTable::sophie2(), Ok("ryza") => OperandTable::ryza(), _ => OperandTable::escha() };
    let mut files = Vec::new();
    files_ending(dir, ".ebd", &mut files)?;
    let mut uses = 0usize;
    let mut lengths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut values: Vec<BTreeMap<i32, usize>> = Vec::new();
    for path in &files {
        let Ok(e) = Ebd::parse(&read(path)?) else { continue };
        for a in &e.actions {
            let Ok(cmds) = ebd::decode(&a.words, &table) else { continue };
            let keep = |c: &&ebd::Command| filter.is_none_or(|(n, v)| c.operands.get(n).is_some_and(|&o| o as i32 == v));
            for c in cmds.iter().filter(|c| c.word == word).filter(keep) {
                uses += 1;
                *lengths.entry(c.operands.len()).or_default() += 1;
                if values.len() < c.operands.len() {
                    values.resize_with(c.operands.len(), BTreeMap::new);
                }
                for (i, &v) in c.operands.iter().enumerate() {
                    *values[i].entry(v as i32).or_default() += 1;
                }
            }
        }
    }
    println!("{word:08x}: {uses} uses, operand counts {lengths:?}");
    for (i, m) in values.iter().enumerate() {
        let mut v: Vec<(i32, usize)> = m.iter().map(|(&k, &n)| (k, n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let top: Vec<String> = v.iter().take(8).map(|(k, n)| format!("{k}×{n}")).collect();
        println!("  {}: {} distinct; {}", i + 1, v.len(), top.join(" "));
    }
    Ok(())
}

fn describe_position(p: &barrel_engine::camera::Position) -> String {
    use barrel_engine::camera::Anchor;
    let [x, y, z] = p.offset;
    match p.anchor {
        Anchor::World => format!("({x}, {y}, {z})"),
        Anchor::Saved(n) => format!("saved point {n} + ({x}, {y}, {z})"),
        Anchor::Thing { kind, id, point } => format!("thing {kind}/{id} point {point} + ({x}, {y}, {z})"),
    }
}

fn describe_move(m: &barrel_engine::camera::Move) -> String {
    use barrel_engine::camera::Move;
    let to = describe_position(m.to());
    match *m {
        Move::Straight { seconds, ease, .. } => format!("moves to {to} in {seconds:.2} s (ease {ease})"),
        Move::Speed { speed, accel, .. } if accel == [0.0; 3] => format!("goes to {to} at {speed}/s"),
        Move::Speed { speed, accel, .. } => format!("goes to {to} at {speed}/s, accelerating {accel:?}"),
        Move::Jump { speed, height, .. } => format!("jumps to {to}, {height} high at {speed}/s"),
        Move::Circle { r0, r1, rise, from, until, speed0, speed1, .. } => {
            format!("circles {to} from {from}° to {until}°, radius {r0} to {r1}, rising {rise}, at {speed0} to {speed1}°/s")
        }
        Move::Accel { speed0, speed1, .. } => format!("goes to {to} speeding from {speed0} to {speed1}/s"),
        Move::Other { kind, .. } => format!("moves (type {kind}) to {to}"),
    }
}

/// Every voice series under `dir` (`SeriesNNNN.fp` + `.fpl`): clips, lengths.
fn voice_scan(dir: &Path) -> Result<()> {
    let mut lists: Vec<_> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "fpl")).collect();
    lists.sort();
    let (mut clips, mut total, mut failed) = (0usize, 0f64, 0usize);
    for fpl in &lists {
        let fp = std::fs::read(fpl.with_extension("fp"))?;
        let (mut n, mut secs) = (0usize, 0f64);
        for (offset, size) in barrel_formats::voice::list(&read(fpl)?)?.into_iter().flatten() {
            match fp.get(offset..offset + size).ok_or(barrel_formats::Error::Corrupt("clip past the end")).and_then(barrel_formats::voice::ogg) {
                Ok(ogg) => {
                    n += 1;
                    secs += barrel_formats::voice::ogg_seconds(&ogg).unwrap_or(0.0) as f64;
                }
                Err(_) => failed += 1,
            }
        }
        println!("{}: {n} clips, {secs:.0} s", fpl.file_stem().unwrap_or_default().to_string_lossy());
        clips += n;
        total += secs;
    }
    println!("{clips} clips, {:.1} hours, {failed} failed", total / 3600.0);
    Ok(())
}

fn print_element(e: &barrel_formats::table::Element, depth: usize) {
    let attrs: Vec<String> = e.attributes.iter().map(|(k, v)| format!("{k}={v:?}")).collect();
    println!("{:indent$}{} {}", "", e.name, attrs.join(" "), indent = depth * 2);
    for c in &e.children {
        print_element(c, depth + 1);
    }
}

fn xml(file: &Path) -> Result<()> {
    let root = barrel_formats::table::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    print_element(&root, 0);
    Ok(())
}

fn open_pak(file: &Path) -> Result<barrel_formats::pak::Archive> {
    Ok(barrel_formats::pak::Archive::open(file)?)
}

fn pak_ls(file: &Path) -> Result<()> {
    let archive = open_pak(file)?;
    let pak = &archive.pak;
    for e in &pak.entries {
        println!("{:>10} {}", e.size, e.name);
    }
    println!("{} files", pak.entries.len());
    Ok(())
}

fn pak_get(file: &Path, name: &str, out: &Path) -> Result<()> {
    let archive = open_pak(file)?;
    let pak = &archive.pak;
    let entry = pak.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or("no such file in the archive")?;
    let data = archive.read(entry).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::write(out, &data).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

/// Every entry whose name contains `part` (any case), written under `out` by its path in the
/// archive.
fn pak_extract(file: &Path, part: &str, out: &Path) -> Result<()> {
    let archive = open_pak(file)?;
    let pak = &archive.pak;
    let part = part.to_ascii_lowercase();
    let mut n = 0;
    for entry in pak.entries.iter().filter(|e| e.name.to_ascii_lowercase().contains(&part)) {
        let data = archive.read(entry).map_err(|e| format!("{}: {e}", file.display()))?;
        let rel: PathBuf = entry.name.split(['\\', '/']).filter(|c| !c.is_empty() && *c != "..").collect();
        let path = out.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
        n += 1;
    }
    println!("{n} files written under {}", out.display());
    Ok(())
}

/// Which reader a piece of data goes to, by its magic, and whether it read.
fn check_entry(data: &[u8]) -> Option<(&'static str, std::result::Result<(), String>)> {
    use barrel_formats as f;
    let r = |x: std::result::Result<(), f::Error>| x.map_err(|e| e.to_string());
    Some(match data.get(..4)? {
        b"GT1G" => ("g1t", r(f::g1t::G1t::parse(data).map(|_| ()))),
        b"_M1G" => ("g1m", r(f::g1m::G1m::parse(data).map(|_| ()))),
        b"_A1G" => ("g1a", r(f::g1a::parse(data).map(|_| ()))),
        b"_H1G" => ("g1h", r(f::g1h::parse(data).map(|_| ()))),
        _ => return None,
    })
}

/// How often each BC6H mode occurs in the archives' textures, with a texture that uses it.
fn pak_bc6h(files: &[&str]) -> Result<()> {
    let mut modes: BTreeMap<u8, (usize, String)> = BTreeMap::new();
    for file in files {
        let archive = open_pak(Path::new(file))?;
    let pak = &archive.pak;
        for entry in &pak.entries {
            if !entry.name.ends_with(".g1t") || entry.size > 256 << 20 {
                continue;
            }
            let data = archive.read(entry).map_err(|e| format!("{file}: {e}"))?;
            let Ok(g) = barrel_formats::g1t::G1t::parse(&data) else { continue };
            for t in g.textures.iter().filter(|t| t.format == barrel_formats::g1t::Format::Bc6h) {
                for level in &t.levels {
                    for block in level.as_chunks::<16>().0 {
                        let low = block[0] & 3;
                        let m = if low < 2 { low } else { block[0] & 31 };
                        let e = modes.entry(m).or_insert((0, entry.name.clone()));
                        e.0 += 1;
                    }
                }
            }
        }
    }
    for (m, (n, sample)) in &modes {
        println!("mode bits {m:#04x}: {n} blocks, e.g. {sample}");
    }
    Ok(())
}

fn pak_scan(files: &[&str]) -> Result<()> {
    use barrel_formats as f;
    let mut stats: BTreeMap<&'static str, (usize, usize, Vec<String>)> = BTreeMap::new();
    let mut note = |kind: &'static str, name: &str, r: std::result::Result<(), String>| {
        let e = stats.entry(kind).or_default();
        match r {
            Ok(()) => e.0 += 1,
            Err(err) => {
                e.1 += 1;
                if e.2.len() < 4 {
                    e.2.push(format!("{name}: {err}"));
                }
            }
        }
    };
    for file in files {
        let archive = open_pak(Path::new(file))?;
    let pak = &archive.pak;
        for entry in &pak.entries {
            let name = entry.name.as_str();
            let interesting = [".g1t", ".g1m", ".g1a", ".g1h", ".elixir.gz", ".ebd", ".ebm"].iter().any(|x| name.ends_with(x));
            if !interesting || entry.size > 512 << 20 {
                continue;
            }
            let data = archive.read(entry).map_err(|e| format!("{file}: {e}"))?;
            if name.ends_with(".elixir.gz") {
                match Elixir::parse(&data) {
                    Ok(e) => {
                        note("elixir", name, Ok(()));
                        for inner in &e.entries {
                            if let Some((kind, r)) = check_entry(e.data(inner)) {
                                note(kind, &format!("{name}#{}", inner.name), r);
                            }
                        }
                    }
                    Err(err) => note("elixir", name, Err(err.to_string())),
                }
            } else if name.ends_with(".ebd") {
                note("ebd", name, f::ebd::Ebd::parse(&data).map(|_| ()).map_err(|e| e.to_string()));
            } else if name.ends_with(".ebm") {
                note("ebm", name, f::ebm::parse(&data).map(|_| ()).map_err(|e| e.to_string()));
            } else if let Some((kind, r)) = check_entry(&data) {
                note(kind, name, r);
            } else {
                note("unknown magic", name, Err(format!("starts {:02x?}", &data[..data.len().min(8)])));
            }
        }
    }
    for (kind, (ok, bad, examples)) in &stats {
        println!("{kind:14} {ok:6} read, {bad:6} failed");
        for e in examples {
            println!("    {e}");
        }
    }
    Ok(())
}

fn xml_e(file: &Path, out: &Path) -> Result<()> {
    let plain = barrel_formats::xml_e::decrypt(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::write(out, &plain).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

fn pak_xml_e(file: &Path) -> Result<()> {
    let archive = open_pak(file)?;
    let pak = &archive.pak;
    let by_name: BTreeMap<&str, &barrel_formats::pak::Entry> =
        pak.entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let (mut same, mut differ, mut alone, mut failed) = (0, 0, 0, 0);
    for e in pak.entries.iter().filter(|e| e.name.ends_with(".xml.e")) {
        let data = archive.read(e).map_err(|err| format!("{}: {err}", file.display()))?;
        let plain = match barrel_formats::xml_e::decrypt(&data) {
            Ok(p) => p,
            Err(err) => {
                println!("FAILED {}: {err}", e.name);
                failed += 1;
                continue;
            }
        };
        match by_name.get(&e.name[..e.name.len() - 2]) {
            Some(twin) => {
                let t = archive.read(twin).map_err(|err| format!("{}: {err}", file.display()))?;
                if t == plain {
                    same += 1;
                } else {
                    println!("differs from its twin: {}", e.name);
                    differ += 1;
                }
            }
            None => alone += 1,
        }
    }
    println!("{same} identical to their twin, {differ} differ, {alone} without a twin, {failed} failed");
    if failed > 0 { Err("some tables did not decrypt".into()) } else { Ok(()) }
}

/// Every request, from `Saves/quest/Quest{Alc,Btl,Gat}.xml`.
fn quests(install: &Path) -> Result<()> {
    use barrel_escha::quest::{requests, Kind};
    let mut total = 0;
    for (file, kind) in [("QuestAlc", Kind::Synthesis), ("QuestBtl", Kind::Monster), ("QuestGat", Kind::Gathering)] {
        let path = install.join(format!("Saves/quest/{file}.xml"));
        let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        let rs = requests(&table, kind);
        println!("{file}: {} requests", rs.len());
        for r in rs.iter().take(5) {
            let prizes: Vec<String> = r.prizes.iter().map(|(t, n)| format!("{t} x{n}")).collect();
            let date = |n: i32| {
                let d = barrel_escha::calendar::Date::of(n.max(0) as u32);
                format!("Y{} {}/{}", d.year, d.month, d.day)
            };
            println!("  {:<32} {:>6.2} pt  days {}-{} ({} to {})  {}", r.target, r.points, r.from_day, r.to_day, date(r.from_day), date(r.to_day), prizes.join(", "));
        }
        total += rs.len();
    }
    println!("{total} requests (the first five of each shown; points and counts as the tables give them)");
    Ok(())
}

/// The request board as it would be redrawn on `day` (see `barrel_escha::quest::draw_board`):
/// no assignment cleared, the seed offset given (0 by default), the targets' states
/// taken as known, battle candidates sorted by their monster's first stat block level.
fn board(install: &Path, day: &str, events: bool, offset: Option<&str>) -> Result<()> {
    let offset: i32 = offset.map_or(Ok(0), |v| v.parse().map_err(|_| format!("{v}: not a number")))?;
    use barrel_escha::quest::{draw_board, requests, Kind};
    let day: i32 = day.parse().map_err(|_| format!("{day}: not a day number"))?;
    let mut all = Vec::new();
    for (file, kind) in [("QuestAlc", Kind::Synthesis), ("QuestBtl", Kind::Monster), ("QuestGat", Kind::Gathering)] {
        let path = install.join(format!("Saves/quest/{file}.xml"));
        let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        all.extend(requests(&table, kind));
    }
    let exe = install.join("Atelier_Escha_and_Logy_EN.exe");
    let data = read(&exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = barrel_escha::data::battle_data::monsters(&pe, barrel_escha::data::battle_data::MONSTERS_EN).ok_or("no monster table")?;
    let (tags, _) = pe.enum_names("MONSTER_ROCKRAT_00").ok_or("no MONSTER_* enum")?;
    let level = |r: &barrel_escha::quest::Request| {
        // The array holds several enums; the monsters' numbers count from its first MONSTER_ tag.
        let first = tags.iter().position(|t| t.starts_with("MONSTER_")).unwrap_or(0);
        tags.iter().position(|t| *t == r.target).and_then(|i| i.checked_sub(first)).and_then(|i| monsters.get(i)).and_then(|m| m.blocks.first()).map_or(0, |b| b.level)
    };
    let starts: std::collections::HashSet<String> = all.iter().filter_map(|r| r.from_event.clone()).collect();
    let seen = |e: &str| events && starts.contains(e);
    let slots = draw_board(&all, day, offset, seen, |_| false, |_| true, level);
    let d = barrel_escha::calendar::Date::of(day.max(0) as u32);
    println!("day {day} (Y{} {}/{}){}", d.year, d.month, d.day, if barrel_escha::calendar::board_day(day.max(0) as u32) { ", a redraw day" } else { ", not a redraw day (drawn as if it were)" });
    for (k, s) in slots.iter().enumerate() {
        let what = ["synthesis", "synthesis", "synthesis", "battle", "battle", "battle", "battle", "gathering", "gathering", "gathering"][k];
        match s {
            Some(i) => println!("  {k} {what:<9} {} ({:.0}%, {:.2} pt)", all[*i].target, all[*i].priority, all[*i].points),
            None => println!("  {k} {what:<9} -"),
        }
    }
    Ok(())
}

/// A new game's first `n` days on `barrel_escha::days::World` (fixed generator state, everyone in
/// the list in slot order, no assignment cleared, the item states taken as high enough).
fn days(install: &Path, n: &str, events: bool, from: Option<&Path>) -> Result<()> {
    use barrel_escha::days::{Reason, Tables, World};
    use barrel_escha::quest::{requests, Kind};
    let n: u32 = n.parse().map_err(|_| format!("{n}: not a number of days"))?;
    let mut all = Vec::new();
    for (file, kind) in [("QuestAlc", Kind::Synthesis), ("QuestBtl", Kind::Monster), ("QuestGat", Kind::Gathering)] {
        let path = install.join(format!("Saves/quest/{file}.xml"));
        all.extend(requests(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?, kind));
    }
    let path = install.join("Saves/rank/RankData.xml");
    let ranks = barrel_escha::rank::ranks(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?);
    // A recipe's price: its item (the first word of its 0x120-byte record at 0x140d19c70) and
    // that item's itemData cost.
    let exe = install.join("Atelier_Escha_and_Logy_EN.exe");
    let data = read(&exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let at = pe.offset(0x1_40d1_9c70).ok_or("no recipe table")?;
    let path = install.join("Saves/item/itemData.xml");
    let items = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let costs: Vec<i32> = items.children.iter().map(|r| r.attr("cost").and_then(|v| v.parse().ok()).unwrap_or(0)).collect();
    let recipe_cost = |k: usize| {
        let o = at + k * 0x120;
        data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).and_then(|i| costs.get(i as usize)).copied().unwrap_or(0)
    };
    let starts: std::collections::HashSet<String> = all.iter().filter_map(|r| r.from_event.clone()).collect();
    // From a save: the events it has seen (its event records, in EVENT_* order).
    let saved = from.map(|file| -> Result<barrel_escha::data::save::Summary> {
        let data = read(file)?;
        let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?.body;
        let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?))
    }).transpose()?;
    let exe = install.join("Atelier_Escha_and_Logy_EN.exe");
    let exe_data = read(&exe)?;
    let exe_pe = barrel_formats::pe::Pe::parse(&exe_data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let seen_events: std::collections::HashSet<&str> = match (&saved, exe_pe.enum_names("EVENT_MM01_OP1_111")) {
        (Some(s), Some((names, _))) => s.events.iter().zip(names).filter(|(e, _)| e.seen).map(|(_, n)| n).collect(),
        _ => Default::default(),
    };
    let seen = |e: &str| if saved.is_some() { seen_events.contains(e) } else { events && starts.contains(e) };
    let path = install.join("Saves/shop/ShopData.xml");
    let wares = barrel_escha::shop::wares(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?);
    // From a save: the board's tests of a request's target against the library's levels
    // (`quest::monster_request_known`; a synthesis target at 6 or more with its recipe flagged).
    let (item_names, _) = pe.enum_names("ITEM_MIX_CRAFT").ok_or("no ITEM_* enum")?;
    let (monster_enum, _) = pe.enum_names("MONSTER_ROCKRAT_00").ok_or("no MONSTER_* enum")?;
    let monster_names = &monster_enum[monster_enum.iter().position(|t| t.starts_with("MONSTER_")).unwrap_or(0)..];
    let recipe_of = |item: usize| {
        (0..265).find(|&k| {
            let o = at + k * 0x120;
            data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap())) == Some(item as i32)
        })
    };
    let known = |r: &barrel_escha::quest::Request| {
        let Some(s) = &saved else { return true };
        let k = &s.knowledge;
        match r.kind {
            Kind::Synthesis => item_names.iter().position(|n| *n == r.target).is_some_and(|i| {
                k.item.get(i).is_some_and(|&l| l >= 6) && recipe_of(i).and_then(|j| k.recipe.get(j)).is_some_and(|&l| l != 0)
            }),
            Kind::Monster => monster_names.iter().position(|n| *n == r.target).is_some_and(|m| barrel_escha::quest::monster_request_known(&k.item, m)),
            Kind::Gathering => true,
        }
    };
    // From a save: an assignment counts as cleared for the board at state 7 or more (achieved
    // or cleared), its record found by its SubjectData row.
    // The rows' tags are the executable's SUBJECT_* names, in row order.
    let (subject_enum, _) = pe.enum_names("SUBJECT_CARD_00_A1_00").ok_or("no SUBJECT_* enum")?;
    let subject_names = &subject_enum[subject_enum.iter().position(|t| t.starts_with("SUBJECT_")).unwrap_or(0)..];
    let cleared = |name: &str| {
        let Some(s) = &saved else { return false };
        subject_names.iter().position(|n| *n == name).and_then(|i| s.subjects.get(i)).is_some_and(|r| r.state >= 7)
    };
    // The battle requests' sort key (`0x14035cc80`): how many of item row m the player holds
    // (the basket's and the container's stacks), m being the monster's number: the same slip as
    // `quest::monster_request_known`.
    let level = |r: &barrel_escha::quest::Request| {
        let Some(s) = &saved else { return 0 };
        let Some(m) = monster_names.iter().position(|n| *n == r.target) else { return 0 };
        barrel_escha::quest::held(s.basket.iter().chain(&s.container).chain(&s.materials).map(|u| (u.item as i32, u.counts[3] as i32)), m as i32)
    };
    let t = Tables { requests: &all, wares: &wares, ranks: &ranks, recipe_cost: &recipe_cost, seen: &seen, cleared: &cleared, known: &known, level: &level };
    let rng = barrel_engine::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    let mut w = if let (Some(file), Some(summary)) = (from, &saved) {
        // A save's calendar (the main generator is not saved: a fixed one stands in).
        let (items, _) = pe.enum_names("ITEM_MIX_CRAFT").ok_or("no ITEM_* enum")?;
        let (names, _) = pe.enum_names("MONSTER_ROCKRAT_00").ok_or("no MONSTER_* enum")?;
        let monsters = &names[names.iter().position(|t| t.starts_with("MONSTER_")).unwrap_or(0)..];
        let board = barrel_escha::quest::board_from_save(&summary.board, &all, &items, monsters);
        let w = World::from_save(summary, rng, &wares, board);
        println!("{} of {} events seen", seen_events.len(), summary.events.len());
        let posted: Vec<&str> = w.board.iter().map(|s| s.map_or("-", |i| all[i].target.as_str())).collect();
        println!("board as saved: {}", posted.join(", "));
        // Each saved slot's rolled rewards beside the ones `quest::rewards` gives for its day.
        for (slot, i) in summary.board.iter().zip(&board) {
            if let (Some(slot), Some(i)) = (slot, i) {
                let period = barrel_escha::calendar::period(slot.posted.max(0) as u32).unwrap_or(9);
                let left = barrel_escha::quest::days_to_deadline(period, slot.posted.max(0) as u32);
                let (points, prizes) = barrel_escha::quest::rewards(&all[*i], slot.wanted, left);
                let rolled: Vec<i32> = prizes.iter().map(|p| p.1).collect();
                println!("  {}: wanted {}, done {}, posted {}, until {}, saved {} {:?}, rolled {points} {rolled:?} ({left} days left)", all[*i].target, slot.wanted, slot.progress, slot.posted, slot.until, slot.value, slot.prizes.map(|p| p.1));
            }
        }
        let d = barrel_escha::calendar::Date::of(w.date);
        println!("from {}: day {} (Y{} {}/{}), rank {} ({} points), squad {:?}", file.display(), w.date, d.year, d.month, d.day, w.rank.rank, w.rank.points, w.joined);
        w
    } else {
        let mut w = World::new_game(rng, 0, &wares);
        // A buyer who empties every shop on the first day.
        for s in &mut w.stock {
            s.count = 0;
        }
        w.joined = (0..barrel_escha::days::MEMBERS).collect();
        for (k, s) in w.slot.iter_mut().enumerate() {
            *s = Some(k);
        }
        println!("new game: day {}, board offset {}", w.date, w.board_offset);
        w
    };
    for _ in 0..n {
        let board = w.board;
        let closed: Vec<u8> = w.months.iter().map(|m| m.state).collect();
        let refilled = w.advance(1, Reason::One, &t);
        let d = barrel_escha::calendar::Date::of(w.date);
        if !refilled.is_empty() {
            println!("day {} (Y{} {}/{}): {} wares restocked", w.date, d.year, d.month, d.day, refilled.len());
        }
        if w.board != board {
            let names: Vec<&str> = w.board.iter().map(|s| s.map_or("-", |i| all[i].target.as_str())).collect();
            println!("day {} (Y{} {}/{}): board {}", w.date, d.year, d.month, d.day, names.join(", "));
        }
        for (k, m) in w.months.iter().enumerate() {
            if m.state != closed[k] {
                println!("day {} (Y{} {}/{}): month {} closed, total {}", w.date, d.year, d.month, d.day, k, m.total);
            }
        }
    }
    println!("day {}: friendship {:?}", w.date, w.friendship.iter().map(|f| (f * 100.0).round() / 100.0).collect::<Vec<_>>());
    Ok(())
}

/// `Bonus.xml`'s rows and their actions.
fn bonuses(file: &Path, exe: Option<&Path>) -> Result<()> {
    use barrel_escha::bonus::{self, KINDS};
    // The item categories, for the synthesis bonuses kept per category (from the executable).
    let data = exe.map(read).transpose()?;
    let pe = data.as_deref().map(barrel_formats::pe::Pe::parse).transpose().map_err(|e| format!("{e}"))?;
    let categories: Vec<&str> = pe.as_ref().and_then(|pe| pe.enum_names("ITEM_CATEGORY_PLANT")).map(|(n, first)| n[first..].to_vec()).unwrap_or_default();
    let table = barrel_formats::table::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    let list = bonus::bonuses(&table);
    let mut values = bonus::State::default();
    for (i, b) in list.iter().enumerate() {
        let acts: Vec<String> = b.actions.iter().map(|a| format!("{}({})", KINDS[a.kind], a.params.iter().flatten().cloned().collect::<Vec<_>>().join(","))).collect();
        println!("{i} {}: {}", b.name, acts.join(" "));
        bonus::apply(&mut values, b, &categories);
    }
    let all: Vec<String> = values.values.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, v)| format!("{i}:{v}")).collect();
    let mix: Vec<String> = values.mix.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, v)| format!("{i}:{v}")).collect();
    println!("{} bonuses; all of them together: values {}, gathering {:?}, synthesis {}", list.len(), all.join(" "), values.collect_time, mix.join(" "));
    Ok(())
}

/// Each English event's first voice clip, by matching clip sizes to line lengths
/// (`voice::story_start`), with the correlation and whether the next event in clip order
/// starts right after it.
fn voice_map(install: &Path) -> Result<()> {
    let voice = install.join("Data/x64/Sound/Voice_EN");
    let mut sizes: Vec<u32> = Vec::new();
    for bank in 0..18 {
        let list = barrel_formats::voice::list(&read(&voice.join(format!("Series{:04}.fpl", bank * 1000)))?).map_err(|e| format!("bank {bank}: {e}"))?;
        let mut b: Vec<u32> = list.iter().map(|c| c.map_or(0, |(_, s)| s as u32)).collect();
        b.resize(1000, 0);
        sizes.extend(b);
    }
    let mut files = Vec::new();
    files_ending(&install.join("Event_EN"), ".ebm", &mut files)?;
    files.sort();
    let mut found: Vec<(usize, usize, f32, String)> = Vec::new();
    for f in &files {
        let Ok(msgs) = barrel_formats::ebm::parse(&read(f)?) else { continue };
        let lengths: Vec<usize> = msgs.iter().map(|m| m.repaired_text().chars().count()).collect();
        if let Some((start, r)) = barrel_escha::data::voice::story_start(&sizes, &lengths) {
            let name = f.file_stem().map(|s| s.to_string_lossy().trim_start_matches("EVENT_MESSAGE_").to_string()).unwrap_or_default();
            found.push((start, lengths.len(), r, name));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.2.total_cmp(&a.2)));
    // The executable's own table (`voice::first_clips`) beside each guess.
    let exe = read(&install.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("executable: {e}"))?;
    let (names, _) = pe.enum_names("EVENT_MM01_OP1_111").ok_or("no EVENT_* enum")?;
    let table = barrel_escha::data::voice::first_clips(&pe, barrel_escha::data::voice::FIRST_CLIPS_EN).ok_or("no voice table")?;
    let actual = |name: &str| names.iter().position(|n| n.strip_prefix("EVENT_") == Some(name)).and_then(|k| table.get(k).copied().flatten());
    let (mut agree, mut strong, mut strong_agree) = (0, 0, 0);
    for (start, n, r, name) in &found {
        let a = actual(name);
        let same = a == Some(*start as u32);
        agree += same as usize;
        if *r >= 0.85 {
            strong += 1;
            strong_agree += same as usize;
        }
        println!("{start:>6} {n:>3} {r:.3} {name}  table {}", a.map_or("-".to_string(), |c| c.to_string()));
    }
    let voiced = table.iter().filter(|c| c.is_some()).count();
    eprintln!("{} events guessed, {agree} as the table has it; {strong_agree} of the {strong} at correlation 0.85 or more; the table voices {voiced} events", found.len());
    Ok(())
}

/// Rebuilds each `Party` record's stats in use (`equipment::in_use`) and compares them with
/// the saved ones.
/// The game's save as a Barrel save (`barrel_escha::session::Session::load`'s text): the lead,
/// the date, money, the party (levels, experience, equipment), the basket, the items known, the
/// places visited, opened and worn, the field effect, the events seen and the event flags. The
/// game's save keeps no field position Barrel reads yet: it loads on the area map of the region
/// the party is in (`worldmap/chara`), so the Barrel save starts in that place's field, at its
/// arrival spot. The container, the battle item grids, requests, shops and the library's other
/// lists are not carried over yet.
fn import_save(install: &Path, file: &Path) -> Result<()> {
    use barrel_escha::data::save::{self, Body};
    let data = read(file)?;
    let body = save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?.body;
    let state = save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
    let records = save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?;
    let s = save::summary(&records);
    let exe = read(&install.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe)?;
    let places = barrel_escha::data::world::places(&pe, barrel_escha::data::world::PLACES_EN).ok_or("no place table")?;
    let group = |name: &str, child: &str| -> Option<Vec<u8>> {
        let r = records.iter().find(|r| r.name == name)?.child(child)?;
        match &r.body {
            Body::Data(d) | Body::Raw(d) => Some(d.to_vec()),
            Body::Group(_) => None,
        }
    };
    // Where the party is: its place on the world map, and the field that place opens.
    let (place, _) = s.world_position.ok_or("no worldmap/chara record")?;
    let place = usize::try_from(place).ok().filter(|&k| k < places.len()).ok_or("the party's place is not in the table")?;
    let names = barrel_escha::data::world::point_names(&pe).ok_or("no world map points")?;
    let field = names
        .iter()
        .position(|n| barrel_escha::data::world::point_tag(n).as_deref() == Some(places[place].tag.as_str()))
        .and_then(|point| barrel_escha::data::world::point_maps(&pe)?.into_iter().find(|(p, _, _)| *p as usize == point).map(|(_, m, _)| m))
        .ok_or_else(|| format!("place {place} ({}) opens no field", places[place].name))?;
    let arrival = barrel_escha::data::world::arrival(&pe, field).ok_or_else(|| format!("field {field} has no arrival spot"))?;
    eprintln!("the party is at place {place} ({}): field {field}", places[place].name);
    let mut out = format!(
        "barrel-save 1\nlead {}\nfield {field} {} {}\ntoday {}\nmoney {}\n",
        if s.hero == Some(1) { "logy" } else { "escha" },
        arrival.position[0] * 100.0,
        arrival.position[2] * 100.0,
        s.day.ok_or("no date record")?,
        s.money.unwrap_or(0),
    );
    // The party in member order: level, experience, and the four equipped items (+0x19c, four
    // item units; an empty slot's serial is 0xffff).
    let party = records.iter().find(|r| r.name == "party").ok_or("no party record")?;
    let Body::Group(children) = &party.body else { return Err("party is not a group".into()) };
    for (c, r) in children.iter().filter(|r| r.name == "Party").enumerate() {
        let (Body::Data(d) | Body::Raw(d)) = &r.body else { continue };
        let m = save::member(d).ok_or("a short Party record")?;
        let kit: Vec<String> = (0..4)
            .map(|k| {
                let at = 0x19c + k * save::ITEM_UNIT;
                d.get(at..at + save::ITEM_UNIT).and_then(|u| save::item_units(u).into_iter().next()).map_or(-1, |u| i32::from(u.item)).to_string()
            })
            .collect();
        out += &format!("member {c} {} {} {}\n", m.level, m.exp, kit.join(","));
    }
    for u in &s.basket {
        let traits: Vec<String> = u.traits.iter().filter(|&&t| t >= 0).map(i16::to_string).collect();
        // One line per item in the stack (the save's +0x22 count), as Barrel's basket holds them.
        for _ in 0..u.counts[3].max(1) {
            out += &format!("item {} {}{}\n", u.item, u.quality, if traits.is_empty() { String::new() } else { format!(" {}", traits.join(",")) });
        }
    }
    let known: Vec<String> = s.knowledge.item.iter().enumerate().filter(|(_, l)| **l >= 2).map(|(i, _)| i.to_string()).collect();
    if !known.is_empty() {
        out += &format!("known {}\n", known.join(","));
    }
    // The places: opened (a gimmick place no longer shut), worn (an obstacle's damage so far,
    // its durability less what is left) and visited (knowledge 15).
    let states = group("worldmap", "pointerStatus").map(|d| save::place_states(&d)).unwrap_or_default();
    for (k, (p, &(left, shut, _))) in places.iter().zip(&states).enumerate() {
        match p.extra {
            [0, _] if !shut => out += &format!("opened {k}\n"),
            [1, n] if left < n => out += &format!("worn {k} {}\n", n - left.max(0)),
            _ => {}
        }
    }
    for (k, &v) in s.place_knowledge.iter().enumerate() {
        if v == 15 {
            out += &format!("visited {k}\n");
        }
    }
    if let Some((gauge, mode, [a, b, c])) = group("fieldmap", "m_userDataFieldMapData").as_deref().and_then(save::field_effect) {
        out += &format!("field-effect {gauge} {mode} {a} {b} {c}\n");
    }
    for (k, t) in s.taken.iter().enumerate().take(barrel_escha::session::TAKEN_SLOTS) {
        if let Some(t) = t {
            out += &format!("taken {k} {} {} {} {} {} {}\n", t.kind, t.target, t.progress, t.wanted, t.posted, t.until);
        }
    }
    for (e, r) in s.events.iter().enumerate().filter(|(_, r)| r.seen) {
        out += &format!("seen {e} {}\n", r.day);
    }
    for (f, v) in s.event_flags.iter().enumerate().filter(|(_, v)| **v != 0) {
        out += &format!("flag {f} {v}\n");
    }
    print!("{out}");
    Ok(())
}

fn check_stats(exe: &Path, records: &[barrel_escha::data::save::Record], s: &barrel_escha::data::save::Summary) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    use barrel_escha::equipment::{self, ItemRow, Tables, STATS};
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
    let dir = exe.parent().ok_or("no install folder")?.join("Saves_EN/item");
    let table = |n: &str| -> Result<barrel_formats::table::Element> { let p = dir.join(n); barrel_formats::table::parse(&read(&p)?).map_err(|e| format!("{}: {e}", p.display()).into()) };
    let effects_root = table("item_effect.xml")?;
    let effects = bd::item_effects(&effects_root, &acts);
    let tags: Vec<&str> = effects.iter().map(|e| e.tag.as_str()).collect();
    let traits = bd::traits(&table("item_potential.xml")?, &tags);
    let items: Vec<ItemRow> = table("itemData.xml")?.children.iter().map(ItemRow::from_row).collect();
    let t = Tables { items: &items, effects: &effects, traits: &traits, acts: &acts };
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table")?;
    let lists = bd::party_skills(&pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists")?;
    let Some(party) = records.iter().find(|r| r.name == "party") else { return Err("no party record".into()) };
    let barrel_escha::data::save::Body::Group(children) = &party.body else { return Err("party is not a group".into()) };
    let words = |d: &[u8], at: usize| -> [i32; STATS] { std::array::from_fn(|k| d.get(at + 4 * k..at + 4 * k + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))) };
    let (mut same, mut all) = (0, 0);
    for r in children.iter().filter(|r| r.name == "Party") {
        let (barrel_escha::data::save::Body::Data(d) | barrel_escha::data::save::Body::Raw(d)) = &r.body else { continue };
        let who = words(d, 0)[0];
        let (base, extra, saved) = (words(d, 0x18), words(d, 0x98), words(d, 0x118));
        let equipped = barrel_escha::data::save::item_units(d.get(0x19c..0x19c + 4 * barrel_escha::data::save::ITEM_UNIT).unwrap_or(&[]));
        let hero = s.hero.is_some_and(|h| h == who);
        let mut lo = |a: i32, _b: i32| a;
        let mut hi = |_a: i32, b: i32| b;
        let mut low = equipment::in_use(&t, &base, &extra, &equipped, hero.then_some(&s.bonus_values[..]), &mut lo);
        let mut high = equipment::in_use(&t, &base, &extra, &equipped, hero.then_some(&s.bonus_values[..]), &mut hi);
        if let Some(list) = usize::try_from(who).ok().and_then(|w| lists.get(w)) {
            let level = words(d, 0)[1];
            equipment::passives(&t, list, &skills, level, &mut low, &mut lo);
            equipment::passives(&t, list, &skills, level, &mut high, &mut hi);
        }
        equipment::clamp(&mut low);
        equipment::clamp(&mut high);
        let off: Vec<String> = (0..STATS).filter(|&k| !(low[k].min(high[k])..=low[k].max(high[k])).contains(&saved[k])).map(|k| format!("{k}: {} (rule {}..{})", saved[k], low[k], high[k])).collect();
        all += 1;
        if off.is_empty() {
            same += 1;
        } else {
            println!("character {who}: {} stats differ: {}", off.len(), off.join(", "));
        }
    }
    println!("stats in use rebuilt for {same} of {all} characters");
    if let (Some(mix_skills), Some((level, _))) = (bd::mix_skills(&pe, bd::MIX_SKILLS_EN), s.alchemy) {
        let learnt: Vec<&str> = mix_skills.iter().zip(&s.mix_skills_learnt).filter(|(_, l)| **l).map(|(m, _)| m.name.as_str()).collect();
        println!("synthesis skills learnt: {}", learnt.join(", "));
        for (user, who) in ["Escha", "Logy"].iter().enumerate() {
            println!("{who} starts a synthesis with {} CP", barrel_escha::synthesis::start_cp(user as i32, level, &s.mix_bonus, &s.mix_skills_learnt, &mix_skills));
        }
    }
    Ok(())
}

/// The monsters' sizes (`Chara.xml` radii) from the install the executable is in, if found.
fn chara_radii_near(exe: &Path) -> Vec<f32> {
    exe.parent()
        .map(|root| root.join("Saves/chara/Chara.xml"))
        .and_then(|p| read(&p).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_escha::data::battle_data::chara_radii(&t))
        .unwrap_or_default()
}

/// A save's header.
fn save(file: &Path) -> Result<()> {
    let data = read(file)?;
    // SYSDATA (the settings) is coded whole; a GAMEDATA file after its plain header.
    let body = if file.file_name().is_some_and(|n| n.to_string_lossy().starts_with("SYSDATA")) {
        &data[..]
    } else {
        let s = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?;
        let t = s.saved_at;
        println!("saved {:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second);
        println!("summary {:?}", s.summary);
        s.body
    };
    let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("{} coded bytes → {} bytes of state", body.len(), state.len());
    // BARREL_SAVE_STATE=path: write the decoded state there.
    if let Ok(out) = std::env::var("BARREL_SAVE_STATE") {
        std::fs::write(&out, &state)?;
    }
    // BARREL_SAVE_BYTES=n: the first n bytes of each record as well.
    fn peek(d: &[u8]) -> String {
        let n: usize = std::env::var("BARREL_SAVE_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if n == 0 {
            return String::new();
        }
        let hex: Vec<String> = d.iter().take(n).map(|b| format!("{b:02x}")).collect();
        format!(" {}", hex.join(""))
    }
    fn show(r: &barrel_escha::data::save::Record, depth: usize) {
        use barrel_escha::data::save::Body;
        let pad = "  ".repeat(depth);
        match &r.body {
            Body::Data(d) => println!("{pad}{} [{}]{}", r.name, d.len(), peek(d)),
            Body::Raw(d) => println!("{pad}{} raw {}{}", r.name, d.len(), peek(d)),
            Body::Group(c) => {
                println!("{pad}{} {{{}}}", r.name, c.len());
                for x in c {
                    show(x, depth + 1);
                }
            }
        }
    }
    let records = barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?;
    for r in &records {
        show(r, 0);
    }
    let s = barrel_escha::data::save::summary(&records);
    let date = s.day.map(|d| {
        let c = barrel_escha::calendar::Date::of(d);
        format!("{d} (year {}, {}/{})", c.year, c.month, c.day)
    });
    println!("money {:?}, alchemy (level, exp) {:?}, day {}, sizes {:?}", s.money, s.alchemy, date.unwrap_or_else(|| "?".into()), s.limits);
    let k = &s.knowledge;
    let levels = |v: &[i32]| {
        let mut n = std::collections::BTreeMap::new();
        for &l in v {
            *n.entry(l).or_insert(0) += 1;
        }
        n.iter().map(|(l, c)| format!("{l}:{c}")).collect::<Vec<_>>().join(" ")
    };
    // The battle item grids, checked against the items' equip costs (BARREL_ITEMS).
    let costs: Vec<u32> = std::env::var("BARREL_ITEMS")
        .ok()
        .and_then(|p| read(Path::new(&p)).ok())
        .and_then(|d| barrel_formats::table::parse(&d).ok())
        .map(|t| t.children.iter().map(|r| r.attr("EquipCost").and_then(|v| v.trim().parse().ok()).unwrap_or(1)).collect())
        .unwrap_or_default();
    for (who, (slots, grid)) in ["Escha", "Logy"].iter().zip(s.battle_item_slots.iter().zip(&s.battle_grid)) {
        let open = grid.iter().take_while(|&&g| g != barrel_escha::explore::LOCKED).count();
        // Each item's own equip cost (unit +0x18), which traits can lower from itemData's.
        let placed: Vec<(usize, u32)> = slots.iter().enumerate().filter_map(|(k, u)| Some((k, u.as_ref()?.equip_cost.max(0) as u32))).collect();
        let check = if costs.is_empty() {
            String::new()
        } else {
            match barrel_escha::explore::grid(&placed, open) {
                Some(g) if g[..] == grid[..] => ", rebuilt from the items' equip costs".into(),
                _ => ", NOT what the items' costs give".into(),
            }
        };
        println!("{who}'s battle grid ({open} open): {grid:?}{check}");
    }
    // BARREL_EXE: rebuild every character's stats in use from base, equipment and bonuses.
    if let Some(exe) = std::env::var_os("BARREL_EXE")
        && let Err(e) = check_stats(Path::new(&exe), &records, &s)
    {
        println!("stats check: {e}");
    }
    let bonuses: Vec<String> = s.bonus_values.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, v)| format!("{i}:{v}")).collect();
    println!("bonus values {}", bonuses.join(" "));
    // BARREL_EXE=path/to/the executable: the recipes' revealed effects, by the recipe table.
    let recipes = std::env::var("BARREL_EXE").ok().and_then(|p| read(Path::new(&p)).ok()).and_then(|d| {
        let at = barrel_escha::data::recipe::find(&d, ESCHA_FIRST_RECIPE)?;
        barrel_escha::data::recipe::parse(&d, at).ok()
    });
    for (r, flags) in s.recipe_effects.iter().enumerate().filter(|(_, f)| f.iter().any(|&b| b)) {
        let shown: Vec<usize> = (0..10).filter(|&j| flags[j]).collect();
        match recipes.as_ref().and_then(|l| l.get(r)) {
            Some(rec) => {
                let list = rec.effect_list();
                let names: Vec<String> = list.iter().enumerate().map(|(j, e)| if flags[j] { e.to_string() } else { "???".into() }).collect();
                println!("recipe {r} (item {}): effects {}", rec.item, names.join(" "));
            }
            None => println!("recipe {r}: effects revealed {shown:?}"),
        }
    }
    for (name, v) in [("item", &k.item), ("recipe", &k.recipe), ("book", &k.book), ("fieldMap", &k.field_map), ("monster", &k.monster), ("mixEsp", &k.mix_esp), ("potential", &k.potential), ("eff", &k.eff)] {
        println!("knowledge {name} ({}): {}", v.len(), levels(v));
    }
    // BARREL_ITEMS=path/to/itemData.xml names the items.
    let names: Vec<String> = std::env::var("BARREL_ITEMS")
        .ok()
        .and_then(|p| read(Path::new(&p)).ok())
        .and_then(|d| barrel_formats::table::parse(&d).ok())
        .map(|t| t.children.iter().map(|r| r.attr("ItemName").unwrap_or("?").to_string()).collect())
        .unwrap_or_default();
    for m in &s.party {
        println!("character {}: level {} (exp {}), HP {}/{}, MP {}/{}, attack {} defense {} speed {}, equipped {:?}, friendship {:.2}", m.character, m.level, m.exp, m.hp, m.max_hp, m.mp, m.max_mp, m.attack, m.defense, m.speed, m.equipped, m.friendship);
    }
    println!("squad {:?}", s.squad);
    println!("board seed {:?}, rank (rank, points) {:?}", s.board_seed, s.rank);
    for (k, t) in s.taken.iter().enumerate() {
        if let Some(t) = t {
            println!("taken {k}: kind {} target {} {}/{} posted {} until {}", t.kind, t.target, t.progress, t.wanted, t.posted, t.until);
        }
    }
    println!("hero {}", match s.hero { Some(0) => "Escha", Some(1) => "Logy", Some(_) => "?", None => "-" });
    for (k, shop) in s.shops.iter().enumerate() {
        let listed: Vec<String> = shop.iter().take(8).map(|w| format!("{}/{}/{}", w[0], w[1], w[2])).collect();
        println!("shop {k}: {} wares (count/days/state) {} ...", shop.len(), listed.join(" "));
    }
    for (label, list) in [("basket", &s.basket), ("container", &s.container), ("materials", &s.materials)] {
        let shown: Vec<String> = list.iter().take(12).map(|u| format!("{} q{}", names.get(u.item as usize).cloned().unwrap_or_else(|| format!("#{}", u.item)), u.quality)).collect();
        println!("{label} {}: {}{}", list.len(), shown.join(", "), if list.len() > 12 { ", ..." } else { "" });
    }
    Ok(())
}

/// Every clip of a voice bank through the native Vorbis reader (headers and codebooks so far).
fn vorbis_scan(fpl: &Path) -> Result<()> {
    use barrel_formats::{voice, vorbis};
    // A folder of loose KOVS files (the music): the same as a bank of them, one after another.
    let (list, fp) = if fpl.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(fpl)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "kvs")).collect();
        names.sort();
        let mut fp = Vec::new();
        let mut list = Vec::new();
        for n in names {
            let d = read(&n)?;
            list.push(Some((fp.len(), d.len())));
            fp.extend_from_slice(&d);
        }
        (list, fp)
    } else {
        (voice::list(&read(fpl)?).map_err(|e| format!("{}: {e}", fpl.display()))?, read(&fpl.with_extension("fp"))?)
    };
    let (mut ok, mut books, mut failed) = (0usize, 0usize, Vec::new());
    for (k, c) in list.iter().enumerate() {
        let Some((off, size)) = *c else { continue };
        let result = (|| -> std::result::Result<(vorbis::Identification, usize), barrel_formats::Error> {
            let ogg = voice::ogg(fp.get(off..off + size).ok_or(barrel_formats::Error::Corrupt("clip past the end"))?)?;
            let packets = vorbis::packets(&ogg)?;
            let id = vorbis::identification(packets.first().ok_or(barrel_formats::Error::Corrupt("no packets"))?)?;
            let setup = vorbis::setup(packets.get(2).ok_or(barrel_formats::Error::Corrupt("no setup header"))?, id.channels)?;
            Ok((id, setup.codebooks.len()))
        })();
        match result {
            Ok((id, n)) => {
                if ok == 0 {
                    println!("clip {k}: {} channel(s), {} Hz, blocks {:?}, {n} codebooks", id.channels, id.rate, id.blocksize);
                }
                ok += 1;
                books += n;
            }
            Err(e) => failed.push(format!("clip {k}: {e}")),
        }
    }
    println!("{ok} clips read ({books} codebooks), {} failed", failed.len());
    for f in failed.iter().take(5) {
        println!("  {f}");
    }
    Ok(())
}

/// One voice clip decoded by the native Vorbis decoder, to a 16-bit PCM WAV.
fn voice_wav(fpl: &Path, clip: &str, out: &Path, ogg_out: Option<&Path>) -> Result<()> {
    use barrel_formats::{voice, vorbis};
    let k: usize = clip.parse().map_err(|_| format!("{clip}: not a clip number"))?;
    let list = voice::list(&read(fpl)?).map_err(|e| format!("{}: {e}", fpl.display()))?;
    let (off, size) = list.get(k).copied().flatten().ok_or("no such clip")?;
    let fp = read(&fpl.with_extension("fp"))?;
    let ogg = voice::ogg(fp.get(off..off + size).ok_or("clip past the end")?).map_err(|e| e.to_string())?;
    if let Some(o) = ogg_out {
        std::fs::write(o, &ogg)?;
    }
    let (id, pcm) = vorbis::decode(&ogg).map_err(|e| e.to_string())?;
    let frames = pcm.first().map_or(0, Vec::len);
    let ch = pcm.len() as u16;
    let mut w = Vec::new();
    let data = frames as u32 * u32::from(ch) * 2;
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&ch.to_le_bytes());
    w.extend_from_slice(&id.rate.to_le_bytes());
    w.extend_from_slice(&(id.rate * u32::from(ch) * 2).to_le_bytes());
    w.extend_from_slice(&(ch * 2).to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        for c in &pcm {
            w.extend_from_slice(&((c[i].clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
    }
    std::fs::write(out, w)?;
    println!("{frames} frames, {} channel(s), {} Hz", ch, id.rate);
    Ok(())
}

/// Decodes a KOVS file whole and as a [`barrel_formats::vorbis::Stream`], from the start and
/// from its loop point, and reports where they differ.
fn stream_check(kvs: &Path) -> Result<()> {
    use barrel_formats::{voice, vorbis};
    let kovs = read(kvs)?;
    let ogg = voice::ogg(&kovs).map_err(|e| format!("{}: {e}", kvs.display()))?;
    let t = std::time::Instant::now();
    let (_, whole) = vorbis::decode(&ogg).map_err(|e| format!("{}: {e}", kvs.display()))?;
    let whole_time = t.elapsed();
    let loop_start = voice::looping(&kovs).map(|l| u64::from(l.start)).unwrap_or(0);
    for from in [0, loop_start, whole[0].len() as u64 / 2] {
        let t = std::time::Instant::now();
        let mut s = vorbis::Stream::open(&ogg).map_err(|e| format!("{}: {e}", kvs.display()))?;
        let mut skip = s.seek(from) as usize;
        let mut got: Vec<Vec<f32>> = vec![Vec::new(); whole.len()];
        while let Some(chunk) = s.read() {
            for (c, x) in chunk.into_iter().enumerate() {
                got[c].extend_from_slice(&x[skip.min(x.len())..]);
            }
            skip = 0;
        }
        let want: Vec<&[f32]> = whole.iter().map(|c| &c[from as usize..]).collect();
        let worst = got.iter().zip(&want).flat_map(|(g, w)| g.iter().zip(w.iter()).map(|(a, b)| (a - b).abs())).fold(0f32, f32::max);
        println!("from frame {from}: {} frames (want {}), largest difference {worst:e}, {:?} (whole decode {whole_time:?})", got[0].len(), want[0].len(), t.elapsed());
    }
    Ok(())
}

/// Every narration card (`u55`): its layout and the texture pieces it shows.
fn telops(exe: &Path) -> Result<()> {
    let bytes = std::fs::read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    for card in 0..=50 {
        let Some(id) = barrel_escha::data::layout::telop_layout(&pe, card) else { continue };
        let pieces = barrel_escha::data::layout::pieces(&pe, id);
        let list: Vec<String> = pieces.iter().map(|p| format!("{} {:?} at {:?}", p.texture, p.src, p.at)).collect();
        println!("card {card:2} layout {id}: {}", list.join("; "));
    }
    Ok(())
}

/// Every interface layout: its place, then each node (part, cut, place, kind) and, for sprite
/// parts, the texture rectangle it shows.
fn layouts(exe: &Path) -> Result<()> {
    use barrel_escha::data::layout;
    let bytes = std::fs::read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    for id in layout::FIRST_LAYOUT..layout::FIRST_LAYOUT + 4096 {
        let Some((at, nodes)) = layout::nodes(&pe, id) else { break };
        println!("layout {id} at {at:?}: {} nodes", nodes.len());
        for t in layout::texts(&pe, id) {
            println!("  text at {:?} size {} colour {:02x?} kind {}{}: {:?}", t.at, t.size, t.colour, t.kind, if t.filled_in() { " (filled in)" } else { "" }, t.text);
        }
        for n in &nodes {
            let shown = layout::sprite(&pe, n.part, n.cut.max(0), n.at).map(|p| format!(" {}#{} {:?}", p.texture, p.index, p.src)).unwrap_or_default();
            println!("  part {:#x} cut {} at {:?} kind {}{shown}", n.part, n.cut, n.at, n.kind);
        }
    }
    Ok(())
}

/// Draws interface layouts `ids` (in order, over each other) on a 1920 × 1080 dark gray screen
/// from their `WinXls_EN` (else `WinXls`) pictures, alpha blended, to a PNG.
fn layout_png(root: &Path, ids: &[u32], out: &Path) -> Result<()> {
    let bytes = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    let pieces: Vec<_> = ids.iter().flat_map(|&id| barrel_escha::data::layout::pieces(&pe, id)).collect();
    draw_pieces(root, &pieces, 1920, 1080, out)
}

/// Every cut of interface sprite `part`, one under another with its number at the left.
fn sprite_png(root: &Path, part: i32, out: &Path) -> Result<()> {
    let bytes = std::fs::read(root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    let (mut pieces, mut y, mut w) = (Vec::new(), 0i32, 1i32);
    for cut in 0..256 {
        let Some(p) = barrel_escha::data::layout::sprite(&pe, part, cut, [0, y]) else { break };
        let [l, t, r, b] = p.src.map(i32::from);
        println!("cut {cut}: {}#{} [{l}, {t}, {r}, {b}] at y {y}", p.texture, p.index);
        w = w.max(r - l);
        y += (b - t).max(1) + 4;
        pieces.push(p);
    }
    draw_pieces(root, &pieces, w.max(1) as usize, y.max(1) as usize, out)
}

fn draw_pieces(root: &Path, pieces: &[barrel_escha::data::layout::Piece], w: usize, h: usize, out: &Path) -> Result<()> {
    use barrel_formats::g1t::G1t;
    let mut img: Vec<u8> = [48u8, 48, 48, 255].repeat(w * h);
    let mut sheets: std::collections::HashMap<(String, u32), Option<(usize, usize, Vec<u8>)>> = std::collections::HashMap::new();
    {
        for p in pieces {
            let id = &p.texture;
            let sheet = sheets.entry((p.texture.clone(), p.index)).or_insert_with(|| {
                let x64 = root.join("Data/x64");
                let data = std::fs::read(x64.join("WinXls_EN").join(&p.texture)).or_else(|_| std::fs::read(x64.join("WinXls").join(&p.texture))).ok()?;
                let g = G1t::parse(&data).ok()?;
                let t = g.textures.get(p.index as usize)?;
                Some((t.width as usize, t.height as usize, t.to_rgba8()))
            });
            let Some((tw, th, px)) = sheet.as_ref() else {
                eprintln!("layout {id}: {}#{} missing", p.texture, p.index);
                continue;
            };
            let [l, t, r, b] = p.src.map(usize::from);
            for sy in t..b.min(*th) {
                for sx in l..r.min(*tw) {
                    let (dx, dy) = (p.at[0] + (sx - l) as i32, p.at[1] + (sy - t) as i32);
                    if dx < 0 || dy < 0 || dx as usize >= w || dy as usize >= h {
                        continue;
                    }
                    let (s, d) = ((sy * tw + sx) * 4, (dy as usize * w + dx as usize) * 4);
                    let a = u32::from(px[s + 3]);
                    for ch in 0..3 {
                        img[d + ch] = ((u32::from(px[s + ch]) * a + u32::from(img[d + ch]) * (255 - a)) / 255) as u8;
                    }
                }
            }
        }
    }
    std::fs::write(out, barrel_formats::png::encode_rgba(w as u32, h as u32, &img))?;
    Ok(())
}

/// Writes `text` in the English font, white on dark gray, to a PNG (a check of the glyph
/// table against the atlas).
fn font_text(exe: &Path, g1t: &Path, out: &Path, text: &str) -> Result<()> {
    use barrel_formats::g1t as tex;
    use barrel_escha::data::font;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    // The English executable has its one table; the multi-language one a table per font folder,
    // picked by the atlas's folder (Font, Font_EN, Font_SC, Font_TC).
    let folder = g1t.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Font_EN");
    let table = font::FONTS_MULTI.iter().find(|(f, _)| *f == folder).map(|&(_, a)| a);
    let glyphs = font::glyphs(&pe, font::FONT_EN)
        .or_else(|| table.and_then(|a| font::glyphs(&pe, a)))
        .ok_or("no font table at the English or the multi-language executable's address")?;
    let atlas_data = read(g1t)?;
    let atlas = tex::G1t::parse(&atlas_data).map_err(|e| format!("{}: {e}", g1t.display()))?;
    let t = atlas.textures.first().ok_or("no texture")?;
    let rgba = t.to_rgba8();
    let line = glyphs.values().filter(|g| g.width > 3).map(|g| g.y_offset + i32::from(g.height)).max().unwrap_or(48);
    let lines: Vec<&str> = text.split("\\n").collect();
    let width = lines.iter().map(|l| l.chars().filter_map(|c| glyphs.get(&c)).map(|g| g.advance as i32).sum::<i32>()).max().unwrap_or(0) + 16;
    let height = line * lines.len() as i32 + 16;
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let mut img = vec![0u8; w * h * 4];
    for px in img.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&[40, 40, 48, 255]);
    }
    let mut missing = 0;
    for (k, l) in lines.iter().enumerate() {
        let mut pen = 8;
        let top = 8 + k as i32 * line;
        for c in l.chars() {
            let Some(g) = glyphs.get(&c) else {
                missing += 1;
                continue;
            };
            for gy in 0..i32::from(g.height) {
                for gx in 0..i32::from(g.width) {
                    let (x, y) = (pen + g.x_offset + gx, top + g.y_offset + gy);
                    if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                        continue;
                    }
                    let s = ((usize::from(g.y) + gy as usize) * t.width as usize + usize::from(g.x) + gx as usize) * 4;
                    let a = u32::from(rgba[s + 3]);
                    let d = (y as usize * w + x as usize) * 4;
                    for ch in 0..3 {
                        let src = u32::from(rgba[s + ch]);
                        img[d + ch] = ((src * a + u32::from(img[d + ch]) * (255 - a)) / 255) as u8;
                    }
                }
            }
            pen += g.advance as i32;
        }
    }
    std::fs::write(out, barrel_formats::png::encode_rgba(w as u32, h as u32, &img))?;
    println!("{} glyphs; line height {line}; {w}x{h}; {missing} characters missing", glyphs.len());
    Ok(())
}

/// A shader pack's DXBC programs and what each reads and writes.
fn g1s(file: &Path, only: Option<usize>) -> Result<()> {
    use barrel_formats::dxbc;
    let data = read(file)?;
    let mask = |m: u8| "xyzw".chars().enumerate().filter(|(i, _)| m >> i & 1 == 1).map(|(_, c)| c).collect::<String>();
    for (k, r) in dxbc::find(&data).into_iter().enumerate() {
        if only.is_some_and(|n| n != k) {
            continue;
        }
        let p = dxbc::program(&data[r.clone()]).map_err(|e| format!("program {k}: {e}"))?;
        println!("program {k} at {:#x}: {:?}, {} bytes of code", r.start, p.stage, p.code.len());
        let io = |v: &[dxbc::Element]| v.iter().map(|e| format!("{}{} v{}.{}", e.semantic, e.index, e.register, mask(e.mask))).collect::<Vec<_>>().join(", ");
        println!("  in:  {}", io(&p.inputs));
        println!("  out: {}", io(&p.outputs));
        for b in &p.bindings {
            println!("  binding {} (kind {}) slot {}", b.name, b.kind, b.slot);
        }
        for c in &p.buffers {
            let vars: Vec<String> = c.variables.iter().map(|v| format!("{}@{}", v.name, v.offset)).collect();
            println!("  cbuffer {} ({} bytes): {}", c.name, c.size, vars.join(" "));
        }
        if only.is_some() {
            for line in dxbc::disassemble(p.code) {
                println!("  {line}");
            }
        }
    }
    Ok(())
}

/// A KWB2 sound effect bank (`.wh2` beside its `.wb2`).
fn se_scan(wh2: &Path) -> Result<()> {
    use barrel_formats::kwb;
    let wb2 = read(&wh2.with_extension("wb2"))?;
    let sounds = kwb::sounds(&read(wh2)?, wb2.len()).map_err(|e| format!("{}: {e}", wh2.display()))?;
    let blocks: usize = sounds.iter().map(Vec::len).sum();
    let mut formats = std::collections::BTreeMap::new();
    let mut seconds = 0.0;
    let mut peak = 0f32;
    for b in sounds.iter().flatten() {
        *formats.entry((b.codec, b.channels, b.rate)).or_insert(0) += 1;
        seconds += b.samples as f32 / b.rate as f32;
        if let Ok(p) = kwb::pcm(&wb2, b) {
            peak = p.iter().flatten().fold(peak, |m, s| m.max(s.abs()));
        }
    }
    println!("{} sounds, {blocks} blocks, {seconds:.1} s in all, peak {peak:.3}", sounds.len());
    for ((codec, ch, rate), n) in formats {
        println!("  codec {codec}, {ch} channel(s), {rate} Hz: {n} blocks");
    }
    for (i, s) in sounds.iter().enumerate().take(4) {
        println!("  sound {i}: {}", s.iter().map(|b| format!("{} samples at {}", b.samples, b.offset)).collect::<Vec<_>>().join("; "));
    }
    Ok(())
}

/// Every shop's wares, from `Saves[_LANG]/shop/ShopData.xml`.
fn shops(install: &Path, lang: Option<&str>) -> Result<()> {
    let dir = lang.map_or_else(|| "Saves".to_string(), |l| format!("Saves_{l}"));
    let file = install.join(dir).join("shop/ShopData.xml");
    let table = barrel_formats::table::parse(&read(&file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    let wares = barrel_escha::shop::wares(&table);
    let mut last = "";
    for w in &wares {
        if w.shop != last {
            println!("{} ({})", w.shop_name, w.shop);
            last = &w.shop;
        }
        let window = match (&w.from_event, &w.until_event) {
            (None, None) => String::new(),
            (a, b) => format!("  [{} .. {}]", a.as_deref().unwrap_or(""), b.as_deref().unwrap_or("")),
        };
        println!(
            "  {:<28} {:>5} cole  q{:<3} stock {:>2}  Refill {} RefillCnt {} DayAdd {}{window}",
            w.item_name, w.cost, w.quality, w.stock, w.refill, w.refill_count, w.days
        );
    }
    println!("{} wares", wares.len());
    Ok(())
}

/// Each party member's base stats at levels 1, 10, 50 and 99.
/// Every monster: each stat block's level, HP/ATK/DEF/SPD and skills.
/// Lists the field enemies: area, position, model row, AI scripts and the group they fight as.
fn field_enemies(exe: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let enemies = bd::field_enemies(&pe, bd::FIELD_ENEMIES_BUILDER_EN).ok_or("no field enemy builder at the English executable's address")?;
    let groups = bd::encounter_groups(&pe, bd::ENCOUNTER_GROUPS_EN);
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).unwrap_or_default();
    let name = |m: i32| monsters.get(m as usize).map_or_else(|| format!("#{m}"), |x| x.name.clone());
    for (k, e) in enemies.iter().enumerate() {
        let fights: Vec<String> = groups.get(e.group as usize).map(|g| g.monsters.iter().map(|&(m, _)| name(m)).collect()).unwrap_or_default();
        println!(
            "{k:3} area {:3} at ({:.1}, {:.1}, {:.1}) chara {} model {} kind {} ai {}/{} walk {} run {} wait {} sight {} reach {} view {}  group {}: {}",
            e.area, e.position[0], e.position[1], e.position[2], e.chara, e.model, e.kind, e.ai.0, e.ai.1, e.speeds.0, e.speeds.1, e.wait, e.sight, e.reach, e.view, e.group, fights.join(", ")
        );
    }
    // With the install's tables beside the executable: the enemies whose model is not their
    // group's first monster's own (its Chara row's BaseTag, as a CharaBase row).
    let table = |name: &str| exe.parent().and_then(|d| std::fs::read(d.join("Saves/chara").join(name)).ok()).and_then(|b| barrel_formats::table::parse(&b).ok());
    if let (Some(charas), Some(bases)) = (table("Chara.xml"), table("CharaBase.xml")) {
        let own = |e: &bd::FieldEnemy| {
            let tag = barrel_escha::field_ai::body_row(&groups, e).and_then(|r| charas.children.get(r)?.attr("BaseTag"))?;
            bases.children.iter().position(|b| b.attr("BaseTag") == Some(tag))
        };
        let tag = |m: i32| usize::try_from(m).ok().and_then(|m| bases.children.get(m)?.attr("BaseTag")).unwrap_or("?");
        let other: Vec<(usize, &bd::FieldEnemy)> = enemies.iter().enumerate().filter(|(_, e)| own(e).map(|r| r as i32) != Some(e.model)).collect();
        println!("{} of {} enemies show a model other than their group's first monster's:", other.len(), enemies.len());
        for (k, e) in other {
            println!("  {k:3} area {:3} model {} ({}), first monster's {:?}", e.area, e.model, tag(e.model), own(e).map(|r| tag(r as i32)));
        }
    }
    Ok(())
}

/// Fights field enemy WHICH (counting from 0) of AREA (a field map number or name, e.g. 12 or
/// AREA_01_01) with SAVE's party: its encounter group's draw (`battle_from_save`).
fn encounter(exe: &Path, area: &str, which: &str, save: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let maps = barrel_escha::data::maps::field_maps(&pe);
    let area: i32 = match area.parse() {
        Ok(n) => n,
        Err(_) => maps.iter().position(|m| m == area).ok_or_else(|| format!("{area}: no such field map"))? as i32,
    };
    // The save's seen events decide who is out (`battle::field_enemy_appears`).
    let save_data = read(save)?;
    let body = barrel_escha::data::save::parse(&save_data).map_err(|e| format!("{}: {e}", save.display()))?.body;
    let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", save.display()))?;
    let summary = barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", save.display()))?);
    let seen = |k: i32| usize::try_from(k).ok().and_then(|k| summary.events.get(k)).is_some_and(|e| e.seen);
    let all: Vec<bd::FieldEnemy> = bd::field_enemies(&pe, bd::FIELD_ENEMIES_BUILDER_EN).ok_or("no field enemies")?.into_iter().filter(|e| e.area == area).collect();
    let total = all.len();
    let here: Vec<bd::FieldEnemy> = all.into_iter().filter(|e| barrel_escha::battle::field_enemy_appears(e.conditions, seen)).collect();
    let map = maps.get(area as usize).map_or("?", String::as_str);
    println!("{map} (field map {area}): {} of its {total} enemies out at this point of the save", here.len());
    for (k, e) in here.iter().enumerate() {
        println!("  {k}: group {} at ({:.1}, {:.1}, {:.1})", e.group, e.position[0], e.position[1], e.position[2]);
    }
    let k: usize = which.parse().map_err(|_| format!("{which}: enemy number"))?;
    let e = here.get(k).ok_or("no such enemy here")?;
    battle_from_save(exe, &format!("g{}", e.group), save)
}

/// Lists the encounter groups: counts and monsters (by name) with their weights.
fn encounters(exe: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table at the English executable's address")?;
    let name = |m: i32| monsters.get(m as usize).map_or_else(|| format!("#{m}"), |x| x.name.clone());
    for g in bd::encounter_groups(&pe, bd::ENCOUNTER_GROUPS_EN) {
        let drawn: Vec<String> = g.monsters.iter().map(|&(m, most)| format!("{} up to {most}", name(m))).collect();
        let extra: Vec<String> = g.extra.iter().map(|&m| name(m)).collect();
        println!("{:3} kind {} {}..{}: {}{}", g.id, g.kind, g.count.0, g.count.1, drawn.join(", "), if extra.is_empty() { String::new() } else { format!("  + {}", extra.join(", ")) });
    }
    Ok(())
}

fn monsters(exe: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table at the English executable's address")?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table at the English executable's address")?;
    for (n, m) in monsters.iter().enumerate() {
        let drops: Vec<String> = m.drops.iter().map(|d| format!("item {} {}%", d.item, d.chance)).collect();
        println!("{n:3} {}{}  drops {}", m.name, if m.death_immune { " (immune to instant death)" } else { "" }, drops.join(", "));
        for b in &m.blocks {
            let names: Vec<String> = b.skills.iter().map(|(s, _)| skills.get(*s as usize).map_or_else(|| s.to_string(), |k| k.name.clone())).collect();
            println!("      lv {:2}  {}/{}/{}/{}  {}", b.level, b.hp, b.attack, b.defense, b.speed, names.join(", "));
            for r in &b.reactions {
                println!("        reaction (type {}): at {}% HP or less, skill list entry {}, {} use(s)", r.kind, r.percent, r.skill, r.uses);
            }
            for r in &b.rules {
                let when = match r.condition {
                    0 => "always".to_string(),
                    1 => format!("every {} turns", r.value),
                    2 => format!("HP <= {}%", r.value),
                    3 => format!("after {}", usize::try_from(r.value).ok().and_then(|i| names.get(i)).map_or_else(|| format!("skill {}", r.value), String::clone)),
                    c => format!("condition {c} {}", r.value),
                };
                let skill = usize::try_from(r.slot)
                    .ok()
                    .and_then(|k| b.slots.get(k))
                    .and_then(|&i| usize::try_from(i).ok())
                    .and_then(|i| names.get(i))
                    .map_or("?", String::as_str);
                let limit = if r.limit >= 0 { format!(", at most {}x", r.limit) } else { String::new() };
                println!("          {when}: {}% {skill} (slot {}){limit}", r.chance, r.slot);
            }
        }
    }
    Ok(())
}

fn party(exe: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let growth = bd::party_growth(&pe, bd::PARTY_GROWTH_EN).ok_or("no party growth table at the English executable's address")?;
    let names = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
    println!("level:HP/MP/attack/defence/speed");
    for (name, levels) in names.iter().zip(&growth) {
        let show = |l: usize| {
            let s = levels[l - 1];
            format!("{}:{}/{}/{}/{}/{}", l, s.hp, s.mp, s.attack, s.defense, s.speed)
        };
        println!("{name:<8} {}  {}  {}  {}", show(1), show(10), show(50), show(99));
    }
    Ok(())
}

/// Each party member's skill list: command, skill, the level it is learnt at, MP cost.
fn party_skills(exe: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table at the English executable's address")?;
    let lists = bd::party_skills(&pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists at the English executable's address")?;
    let names = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
    for (name, list) in names.iter().zip(&lists) {
        println!("{name}");
        for e in list {
            let s = skills.get(e.skill as usize).map_or("?", |s| s.name.as_str());
            let level = e.level.map_or("-".to_string(), |l| l.to_string());
            println!("  command {:2}  {:3} {:<22} level {:>3}  MP {:2}", e.command, e.skill, s, level, e.mp);
        }
    }
    Ok(())
}

/// A partial battle between two monsters (by number) trading normal attacks; see
/// `barrel_escha::duel` for what it leaves out.
fn duel(exe: &Path, a: &str, b: &str, equip: Option<&str>) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table at the English executable's address")?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table at the English executable's address")?;
    // The monsters' normal attack is SKILL_ATTACK_ENEMY; a party member's is command 0 of its
    // skill list.
    let monster_attack = skills.get(1).ok_or("no skill 1")?;
    let lists = bd::party_skills(&pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists at the English executable's address")?;
    let growth = bd::party_growth(&pe, bd::PARTY_GROWTH_EN);
    let party_names = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
    // A fighter: a monster number, or p<member>:<level> for a party member.
    let pick = |s: &str| -> Result<(String, bd::StatBlock, &bd::Skill)> {
        if let Some(rest) = s.strip_prefix('p') {
            let (m, l) = rest.split_once(':').ok_or_else(|| format!("{s}: write p<member>:<level>"))?;
            let (m, l): (usize, usize) = (m.parse().map_err(|_| format!("{s}: member"))?, l.parse().map_err(|_| format!("{s}: level"))?);
            let g = growth.as_ref().ok_or("no party growth table")?;
            let stats = g.get(m).and_then(|levels| levels.get(l.wrapping_sub(1))).ok_or_else(|| format!("{s}: no such member or level"))?;
            let name = format!("{} (level {l})", party_names.get(m).unwrap_or(&"?"));
            let attack = lists[m].iter().find(|e| e.command == 0).and_then(|e| skills.get(e.skill as usize)).ok_or("no party attack")?;
            return Ok((name, barrel_escha::duel::party_block(l as i32, stats), attack));
        }
        let n: usize = s.parse().map_err(|_| format!("{s}: not a monster number"))?;
        let m = monsters.get(n).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("{s}: no such monster"))?;
        Ok((m.name.clone(), m.blocks[0].clone(), monster_attack))
    };
    let ((na, mut ba, aa), (nb, bb, ab)) = (pick(a)?, pick(b)?);
    // --equip "NAME[@QUALITY]": an item from Saves_EN/item/itemData.xml for the first fighter.
    if let Some(spec) = equip {
        let (name, quality) = spec.rsplit_once('@').map_or((spec, 50.0), |(n, q)| (n, q.parse().unwrap_or(50.0)));
        let root = exe.parent().ok_or("no install folder")?;
        let path = root.join("Saves_EN/item/itemData.xml");
        let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        let k = table.children.iter().position(|r| r.attr("ItemName") == Some(name)).ok_or_else(|| format!("no item named {name}"))?;
        // The item as made at that quality, with no effects or traits (equipment::item_stat).
        let items: Vec<barrel_escha::equipment::ItemRow> = table.children.iter().map(barrel_escha::equipment::ItemRow::from_row).collect();
        let t = barrel_escha::equipment::Tables { items: &items, effects: &[], traits: &[], acts: &[] };
        let unit = barrel_escha::data::save::ItemUnit { serial: 0, item: k as i16, quality, traits: [-1; 3], word_e: 0, effects: [-1; 4], equip_cost: 0, word_1a: 0, counts: [0; 4] };
        let mut roll = |lo: i32, _hi: i32| lo;
        let mut add = |stat: usize, base: i32| base + barrel_escha::equipment::item_stat(&t, &unit, stat, base, &mut roll);
        ba.hp = add(0, ba.hp);
        ba.attack = add(2, ba.attack);
        ba.defense = add(3, ba.defense);
        ba.speed = add(4, ba.speed);
        println!("{na} equips {name} (quality {quality}): attack {}, defence {}, speed {}", ba.attack, ba.defense, ba.speed);
    }
    let mut rng = barrel_engine::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    // Monsters pick by their rules from the skill table; party members use the normal attack.
    let table = |s: &str| (!s.starts_with('p')).then_some(&skills[..]);
    let fa = barrel_escha::duel::Fighter { block: ba, normal: aa, skills: table(a), death_immune: false, chara: -1 };
    let fb = barrel_escha::duel::Fighter { block: bb, normal: ab, skills: table(b), death_immune: false, chara: -1 };
    let hits = barrel_escha::duel::duel(&fa, &fb, &mut rng, 1000);
    for h in &hits {
        let (x, y) = if h.attacker == 0 { (&na, &nb) } else { (&nb, &na) };
        if h.skill == "poison" {
            println!("{x} takes {} poison damage ({} HP left)", h.damage, h.hp_left);
        } else if h.damage < 0 {
            println!("{x} uses {}: heals {} ({} HP)", h.skill, -h.damage, h.hp_left);
        } else {
            println!("{x} uses {} on {y}: {} damage ({} HP left)", h.skill, h.damage, h.hp_left);
        }
    }
    println!("{} hits (partial battle: no traits or start rules, states only poison and stat buffs; see barrel_escha::duel)", hits.len());
    Ok(())
}

/// Each row of `Saves/item/item_effect.xml` with its battle effects (act names from EXE).
fn item_effects(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
    let path = exe.parent().ok_or("no install folder")?.join("Saves_EN/item/item_effect.xml");
    let root = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows = barrel_escha::data::battle_data::item_effects(&root, &acts);
    let elements = ["fire", "water", "wind", "earth"];
    for (i, r) in rows.iter().enumerate() {
        let effects: Vec<String> = r
            .effects
            .iter()
            .map(|e| {
                let act = acts.get(e.act as usize).map_or("?", |a| a.trim_start_matches("ACT_"));
                let element = e.element.map_or("physical", |k| elements[k as usize]);
                format!("{act} {element} {}..{} ({}..{})", e.min, e.max, e.min2, e.max2)
            })
            .collect();
        println!("{i:3} {:<28} {}", r.name, effects.join(", "));
    }
    Ok(())
}

/// Party members against a monster: see `barrel_escha::skirmish`.
fn battle(exe: &Path, monster: &str, party: &[&str]) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    use barrel_escha::fight::Event;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table at the English executable's address")?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table at the English executable's address")?;
    let lists = bd::party_skills(&pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists at the English executable's address")?;
    let growth = bd::party_growth(&pe, bd::PARTY_GROWTH_EN).ok_or("no party growth table")?;
    let member_names = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
    let mut names = Vec::new();
    let mut members = Vec::new();
    // Item effect rows, for members given items (`+<id>,<id>@<quality>x<count>`).
    let item_rows = if party.iter().any(|s| s.contains('+')) {
        let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
        let path = exe.parent().ok_or("no install folder")?.join("Saves_EN/item/item_effect.xml");
        let root = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        bd::item_effects(&root, &acts)
    } else {
        Vec::new()
    };
    for s in party {
        let mut parts = s.split('+');
        let s = parts.next().unwrap_or("");
        let (m, l) = s.strip_prefix('p').and_then(|r| r.split_once(':')).ok_or_else(|| format!("{s}: write p<member>:<level>"))?;
        let (m, l): (usize, usize) = (m.parse().map_err(|_| format!("{s}: member"))?, l.parse().map_err(|_| format!("{s}: level"))?);
        let stats = growth.get(m).and_then(|levels| levels.get(l.wrapping_sub(1))).ok_or_else(|| format!("{s}: no such member or level"))?;
        let block = barrel_escha::duel::party_block(l as i32, stats);
        let mut member = barrel_escha::skirmish::Member::new(block, stats.mp, &lists[m], &skills).ok_or("no party attack")?;
        member.size = barrel_escha::data::battle_data::party_size(&chara_radii_near(exe), m).unwrap_or(0.0);
        for item in parts {
            // <ids>@<quality>x<count>: heals and revives aim at the user's side, the rest at the
            // foe; the action's wait is not read yet (the normal attack's 600 is taken).
            // An optional item id first (#<item>:, its itemData.xml row) gives its target and wait
            // from the battle item table.
            let (row, item) = match item.strip_prefix('#').and_then(|r| r.split_once(':')) {
                Some((r, rest)) => {
                    let id: i32 = r.parse().map_err(|_| format!("{item}: item id"))?;
                    let table = bd::item_battles(&pe, bd::ITEM_BATTLE_EN, bd::ITEM_BATTLE_END_EN);
                    (Some(*table.iter().find(|b| b.item == id).ok_or_else(|| format!("{item}: no battle item {id}"))?), rest)
                }
                None => (None, item),
            };
            let (ids, rest) = item.split_once('@').ok_or_else(|| format!("{item}: write [#<row>:]<id>[,<id>]@<quality>x<count>"))?;
            // ...x<count>[t<trait>,<trait>]: traits by item_potential.xml row.
            let (rest, trait_ids) = rest.split_once('t').unwrap_or((rest, ""));
            let (quality, count) = rest.split_once('x').unwrap_or((rest, "1"));
            // Either effect ids, or ~fire,water,wind,earth: the effects the item's recipe lines give
            // at those element values (`recipe::effect_at`), as synthesis would make it.
            let ids: Vec<i16> = if let Some(values) = ids.strip_prefix('~') {
                let b = row.ok_or_else(|| format!("{item}: ~ needs the item id (#<item>:)"))?;
                let values: Vec<i32> = values.split(',').map(|v| v.parse().map_err(|_| format!("{item}: element value"))).collect::<std::result::Result<_, String>>()?;
                let at = barrel_escha::data::recipe::find(&data, ESCHA_FIRST_RECIPE).ok_or("no recipe table found")?;
                let recipes = barrel_escha::data::recipe::parse(&data, at).map_err(|e| format!("recipes: {e}"))?;
                let r = recipes.iter().find(|r| i32::from(r.item) == b.item).ok_or_else(|| format!("{item}: no recipe makes item {}", b.item))?;
                (0..4).filter_map(|k| barrel_escha::data::recipe::effect_at(&r.lines[k], values.get(k).copied().unwrap_or(0))).map(|e| e as i16).collect()
            } else {
                ids.split(',').map(|v| v.parse().map_err(|_| format!("{item}: effect id"))).collect::<std::result::Result<_, String>>()?
            };
            let quality: f32 = quality.parse().map_err(|_| format!("{item}: quality"))?;
            let count: u32 = count.parse().map_err(|_| format!("{item}: count"))?;
            let effect_names = ids.iter().filter_map(|&i| item_rows.get(i as usize)).map(|r| r.name.clone()).collect::<Vec<_>>().join(" + ");
            // The item's own name (itemData.xml) when it is given, else its effects'.
            let name = match row {
                Some(b) => {
                    let path = exe.parent().ok_or("no install folder")?.join("Saves_EN/item/itemData.xml");
                    let data = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
                    data.children.get(b.item as usize).and_then(|r| r.attr("ItemName")).map_or(effect_names.clone(), str::to_string)
                }
                None => effect_names,
            };
            let mut action = barrel_escha::battle::item_action(&name, 2, row.map_or(0, |b| b.area), 600, &ids, &item_rows);
            if action.effects.iter().any(|e| matches!(e.act, 19 | 21 | 81)) {
                action.target = 1;
            }
            if let Some(b) = row {
                (action.target, action.wait) = (b.target, b.wait);
            }
            let traits = if trait_ids.is_empty() {
                Vec::new()
            } else {
                let path = exe.parent().ok_or("no install folder")?.join("Saves_EN/item/item_potential.xml");
                let tags: Vec<&str> = item_rows.iter().map(|r| r.tag.as_str()).collect();
                let all = bd::traits(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?, &tags);
                trait_ids.split(',').map(|t| t.parse::<usize>().ok().and_then(|k| all.get(k).cloned()).ok_or_else(|| format!("{item}: trait {t}"))).collect::<std::result::Result<Vec<_>, String>>()?
            };
            member.items.push(barrel_escha::skirmish::Held { action, power: barrel_escha::battle::item_power(quality), count, traits });
        }
        members.push(member);
        names.push(format!("{} ({l})", member_names[m]));
    }
    let n: usize = monster.parse().map_err(|_| format!("{monster}: not a monster number"))?;
    let m = monsters.get(n).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("{monster}: no such monster"))?;
    names.push(m.name.clone());
    let foe = barrel_escha::duel::Fighter { block: m.blocks[0].clone(), normal: skills.get(1).ok_or("no skill 1")?, skills: Some(&skills[..]), death_immune: m.death_immune, chara: barrel_escha::data::battle_data::MONSTER_CHARA_FIRST as i32 + n as i32 };
    let mut rng = barrel_engine::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    let acts = bd::acts(&pe, bd::ACTS_EN);
    let kinds = bd::trait_kinds(&pe);
    let rules = barrel_escha::skirmish::ItemRules { acts: &acts, kinds: &kinds, effects: &item_rows };
    let radii = chara_radii_near(exe);
    let sizes: Vec<f32> = barrel_escha::data::battle_data::monster_size(&radii, n).into_iter().collect();
    let (events, winner) = barrel_escha::skirmish::skirmish(&members, std::slice::from_ref(&foe), &sizes, rules, &mut rng, 1000);
    for e in &events {
        match e {
            Event::Felled { who, skill, target } => println!("{} uses {skill} on {}: felled", names[*who], names[*target]),
            Event::Poisoned { who, damage, hp_left } => println!("{} takes {damage} poison damage ({hp_left} HP left)", names[*who]),
            Event::Acted { who, skill, target, damage, hp_left, mp_left } => {
                let mp = if *who < party.len() { format!(", {mp_left} MP left") } else { String::new() };
                println!("{} uses {skill} on {}: {damage} damage ({hp_left} HP left{mp})", names[*who], names[*target]);
            }
            Event::Healed { who, skill, target, amount, hp_left, mp_left } => {
                let mp = if *who < party.len() { format!(", {mp_left} MP left") } else { String::new() };
                println!("{} uses {skill} on {}: heals {amount} ({hp_left} HP{mp})", names[*who], names[*target]);
            }
        }
    }
    match winner {
        Some(0) => println!("the party wins"),
        Some(_) => println!("{} wins", m.name),
        None => println!("no winner after {} events", events.len()),
    }
    Ok(())
}

/// Lists the gathering lists (or one): each item with its weight, quality range and traits.
fn gather(install: &Path, only: Option<u32>) -> Result<()> {
    let exe = install.join("Atelier_Escha_and_Logy_EN.exe");
    let data = read(&exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let rows = barrel_escha::data::gather::gathering(&pe, barrel_escha::data::gather::GATHER_EN).ok_or("no gathering table")?;
    let items = table_rows(&install.join("Saves_EN/item/itemData.xml"))?;
    let name = |n: usize| items.get(n).and_then(|r| r.attr("ItemName")).unwrap_or("?").to_string();
    let lists: std::collections::BTreeSet<u32> = rows.iter().map(|g| g.list).collect();
    println!("{} records in {} lists", rows.len(), lists.len());
    let points = barrel_escha::data::gather::points(&pe, barrel_escha::data::gather::POINTS_EN).ok_or("no gathering points")?;
    for (n, p) in points.iter().enumerate().filter(|(_, p)| only.is_none_or(|l| p.lists.contains(&l))) {
        println!("point {n:3}: map {:3} at ({:.1}, {:.1}, {:.1}), lists {:?}", p.map, p.position[0], p.position[1], p.position[2], p.lists);
    }
    for g in rows.iter().filter(|g| only.is_none_or(|l| g.list == l)) {
        let name = if g.item < 0 { "(none)".to_string() } else { name(g.item as usize) };
        let quality = if g.quality == (0, 0) { String::new() } else { format!(", quality {} to {}", g.quality.0, g.quality.1) };
        println!("{:4} {:4} {name}: weight {}{quality}, traits {:?}, unread {:?}", g.list, g.item, g.weight, g.traits, g.unread);
    }
    Ok(())
}

/// One gather at a gathering point: its list (by the table level, BARREL_TABLE_LEVEL, 0 by
/// default), BARREL_PICKS picks (1), each item with its quality and traits; with a save, items
/// it knows well are not boosted.
fn gather_sim(install: &Path, point: &str, save: Option<&Path>) -> Result<()> {
    let env = |k: &str, d: i32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let g = barrel_escha::gather::Gathering::load(install)?;
    let n: usize = point.parse().map_err(|_| "POINT is a number")?;
    let p = g.points.get(n).ok_or("no such point")?;
    let items = table_rows(&install.join("Saves_EN/item/itemData.xml"))?;
    let knowledge: Vec<i32> = match save {
        Some(file) => {
            let data = read(file)?;
            let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?.body;
            let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
            barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?).knowledge.item
        }
        None => Vec::new(),
    };
    let mut rng = barrel_engine::rng::Rng::from_state([env("BARREL_SEED", 1) as u32, 2, 3, 4]);
    let level = env("BARREL_TABLE_LEVEL", 0);
    println!("point {point}: map {}, at {:?} m, lists {:?}", p.map, p.position, p.lists);
    for got in g.gather(n, barrel_escha::gather::Levels::tables(level), |item| knowledge.get(item as usize).copied().unwrap_or(0), env("BARREL_PICKS", 1).max(0) as u32, &mut rng) {
        let name = items.get(got.item.max(0) as usize).and_then(|r| r.attr("ItemName")).unwrap_or("?");
        println!("  {name} at quality {}, traits {:?}", got.quality, got.traits);
    }
    Ok(())
}

/// Prints the lines of one talk id (a town person's placement +0x80). Text is shown only on
/// request: it can tell the story ahead.
fn town_talk(exe: &Path, id: i32) -> Result<()> {
    let data = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    for l in barrel_escha::data::npc::lines(&pe).ok_or("no talk table")?.iter().filter(|l| l.id == id) {
        println!("chara {:4} ({:3}) {}{}", l.chara, l.word_8, l.text, if l.second.is_empty() { String::new() } else { format!("  [{}]", l.second) });
    }
    Ok(())
}

/// Lists the compiled placements of town people (or one map's): character, and the events their
/// conditions want seen and not seen.
fn town_people(exe: &Path, only: Option<i32>, save: Option<&Path>) -> Result<()> {
    use barrel_escha::data::npc;
    let data = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let places = npc::places(&pe).ok_or("no placement table")?;
    let events = pe.enum_names("EVENT_MM01_OP1_111").map(|(n, _)| n).unwrap_or_default();
    let name = |e: &i32| usize::try_from(*e).ok().and_then(|e| events.get(e)).map_or(e.to_string(), |n| n.trim_start_matches("EVENT_").to_string());
    let show = |c: &i32| match npc::condition(&pe, *c) {
        Some((seen, not)) => format!("#{c}[{} / not {}]", seen.iter().map(name).collect::<Vec<_>>().join(" "), not.iter().map(name).collect::<Vec<_>>().join(" ")),
        None => format!("#{c}"),
    };
    let seen: Option<Vec<bool>> = match save {
        Some(f) => {
            let data = read(f)?;
            let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", f.display()))?.body;
            let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", f.display()))?;
            let sum = barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| e.to_string())?);
            Some(sum.events.iter().map(|e| e.seen).collect())
        }
        None => None,
    };
    let is_seen = |e: &i32| seen.as_ref().and_then(|s| usize::try_from(*e).ok().and_then(|e| s.get(e))).copied().unwrap_or(false);
    let holds = |c: &i32| npc::condition(&pe, *c).is_some_and(|(want, not)| want.iter().all(is_seen) && !not.iter().any(is_seen));
    for (k, p) in places.iter().enumerate().filter(|(_, p)| only.is_none_or(|m| m == p.map)) {
        let passes = (p.conditions[0].is_empty() || p.conditions[0].iter().any(holds)) && !p.conditions[1..].iter().flatten().any(holds);
        if seen.is_some() {
            print!("{} ", if passes { "PASS" } else { "    " });
        }
        let lists: Vec<String> = p.conditions.iter().map(|l| l.iter().map(show).collect::<Vec<_>>().join(" ")).collect();
        let [x, y, z] = p.position;
        println!(
            "{k:3} map {:3} chara {:4} at ({x:.1}, {y:.1}, {z:.1}) facing {:.0}  any of: {}  none of: {}",
            p.map,
            p.chara,
            p.angle,
            lists[0],
            lists[1..].iter().filter(|l| !l.is_empty()).cloned().collect::<Vec<_>>().join(" | ")
        );
    }
    Ok(())
}

/// Lists each map's gimmicks (or one map's), by map number.
fn gimmicks(exe: &Path, only: Option<usize>) -> Result<()> {
    let data = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let areas = barrel_escha::data::gimmick::areas(&pe, barrel_escha::data::gimmick::MAPS_EN, barrel_escha::data::gimmick::MAP_COUNT).ok_or("no gimmick table")?;
    for (k, a) in areas.iter().enumerate().filter(|(k, _)| only.is_none_or(|o| o == *k)) {
        println!("{k:3} {} ({}), words {:?}: {} gimmicks", a.name, a.place, a.words, a.gimmicks.len());
        if only.is_some() {
            for g in &a.gimmicks {
                let [x, y, z] = g.position;
                println!("    {:20} at ({x:.1}, {y:.1}, {z:.1}) turned {}, size {:?}, events {:?}, id {}", g.kind_name(), g.angle, g.size, g.events, g.id);
            }
        }
    }
    Ok(())
}

/// Lists the routes between world-map places and their trip days: with a save, the days now
/// (the shorter times whose events the save has seen).
fn routes(exe: &Path, save: Option<&Path>) -> Result<()> {
    let data = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let areas = barrel_escha::data::world::areas(&pe, barrel_escha::data::world::AREAS_EN).ok_or("no area table")?;
    let events = pe.enum_names("EVENT_MM01_OP1_111").map(|(n, _)| n).unwrap_or_default();
    let seen: Vec<bool> = match save {
        Some(file) => {
            let data = read(file)?;
            let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?.body;
            let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
            let summary = barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?);
            summary.events.iter().map(|e| e.seen).collect()
        }
        None => Vec::new(),
    };
    for (n, a) in areas.iter().enumerate().filter(|(_, a)| a.id.starts_with('R')) {
        let event = |e: i32| events.get(e as usize).copied().unwrap_or("?");
        let [(base, _), (fewer, after), (fewest, last)] = a.days;
        let now = if save.is_some() { format!(", now {}", a.trip_days(|e| seen.get(e as usize).copied().unwrap_or(false))) } else { String::new() };
        let days = |d: i32| if d == 1 { "1 day".to_string() } else { format!("{d} days") };
        println!("{n:3} {:4} {}, {} after {}, {} after {}{now}", a.id, days(base), days(fewer), event(after), days(fewest), event(last));
    }
    Ok(())
}

/// Numbers the names of the executable's enum that holds `name`, in order.
fn enum_list(exe: &Path, name: &str) -> Result<()> {
    let data = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (names, _) = pe.enum_names(name).ok_or_else(|| format!("no enum holds {name}"))?;
    for (i, n) in names.iter().enumerate() {
        println!("{i:5} {n}");
    }
    Ok(())
}

fn faces(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let t = barrel_escha::face::Tables::read(&pe);
    let mut rows: Vec<&u32> = t.brow.keys().chain(t.eye.keys()).chain(t.mouth.keys()).collect();
    rows.sort();
    rows.dedup();
    for r in &rows {
        let first = |m: &std::collections::HashMap<u32, Vec<Option<String>>>| {
            m.get(r).map(|v| v.iter().map(|n| n.as_deref().unwrap_or("-")).collect::<Vec<_>>().join(" ")).unwrap_or_default()
        };
        println!("row {r}:\n  brow  {}\n  eye   {}\n  mouth {}", first(&t.brow), first(&t.eye), first(&t.mouth));
    }
    println!("{} characters: {} brow, {} eye, {} mouth tables", rows.len(), t.brow.len(), t.eye.len(), t.mouth.len());
    Ok(())
}

/// Escha & Logy DX's first synthesizable item (Craft).
const ESCHA_FIRST_RECIPE: u32 = 195;

fn recipes(exe: &Path) -> Result<()> {
    use barrel_escha::data::recipe::{self, Ingredient};
    let data = read(exe)?;
    let at = recipe::find(&data, ESCHA_FIRST_RECIPE).ok_or("no recipe table found")?;
    let list = recipe::parse(&data, at).map_err(|e| format!("recipes: {e}"))?;
    println!("{} recipes at file offset {at:#x}", list.len());
    for r in &list {
        let slots: Vec<String> = r
            .slots
            .iter()
            .map(|s| match s.ingredient {
                Ingredient::Item(i) => format!("item {i} x{}", s.count),
                Ingredient::Category(c) => format!("(category {c}) x{}", s.count),
            })
            .collect();
        let lines: Vec<String> = r
            .lines
            .iter()
            .zip(["fire", "water", "wind", "earth"])
            .map(|(l, element)| {
                let steps: Vec<String> = l
                    .iter()
                    .take_while(|st| st.up_to >= 0)
                    .map(|st| match st.effect {
                        Some(e) => format!("{e}<{}", st.up_to),
                        None => format!("-<{}", st.up_to),
                    })
                    .collect();
                format!("{element} [{}]", steps.join(" "))
            })
            .collect();
        println!(
            "{} kind {} cost {} days {} | {} | {}",
            r.item,
            r.kind,
            r.cost,
            r.days,
            slots.join(", "),
            lines.join(" ")
        );
    }
    Ok(())
}

fn table_rows(path: &Path) -> Result<Vec<barrel_formats::table::Element>> {
    let root = barrel_formats::table::parse(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(root.children)
}

/// What a basket (`ITEM:QUALITY` each) can make (`barrel_escha::cauldron`), and the first of
/// those made from it.
fn cauldron(install: &Path, items: &[&str]) -> Result<()> {
    let exe = read(&install.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let book = barrel_escha::cauldron::Book::load(&exe, install)?;
    let mut basket = Vec::new();
    for it in items {
        let (item, quality) = it.split_once(':').unwrap_or((it, "50"));
        basket.push(barrel_escha::gather::Got { item: item.parse().map_err(|_| format!("{it}: ITEM:QUALITY"))?, quality: quality.parse().map_err(|_| format!("{it}: ITEM:QUALITY"))?, traits: Vec::new() });
    }
    let makeable = book.makeable(&basket);
    for &r in &makeable {
        let item = i32::from(book.recipes[r].item);
        println!("recipe {r}: {} ({item}), {} days", book.name(item), book.recipes[r].days);
    }
    let Some(&r) = makeable.first() else {
        println!("nothing can be made; the first recipes take:");
        for recipe in book.recipes.iter().filter(|r| !r.slots.is_empty()).take(6) {
            let wants: Vec<String> = barrel_escha::synthesis::order(recipe).into_iter().map(|i| book.ingredient(i)).collect();
            println!("  {} ({}): {}", book.name(i32::from(recipe.item)), recipe.item, wants.join(", "));
        }
        return Ok(());
    };
    let picks = book.fill(r, &basket).ok_or("no fill")?;
    let made = book.make(r, &picks, &basket, &mut |lo, _| lo)?;
    let used: Vec<&str> = picks.iter().map(|&k| book.name(basket[k].item)).collect();
    println!("made {} from {}: quality {}, effects {:?}, {} days", book.name(made.item), used.join(", "), made.quality, made.effects, made.days);
    Ok(())
}

fn synth(install: &Path, item: &str, materials: &[&str]) -> Result<()> {
    use barrel_escha::data::recipe;
    use barrel_escha::synthesis::{Material, Synthesis};
    let number = |s: &str| s.parse::<usize>().map_err(|_| format!("not an item number: {s}"));
    // The install has the game for each language plus a launcher and a settings tool; use
    // the first that holds the table.
    let mut exes: Vec<PathBuf> = std::fs::read_dir(install)
        .map_err(|e| format!("{}: {e}", install.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "exe"))
        .collect();
    exes.sort();
    let mut found = None;
    for exe in &exes {
        let data = read(exe)?;
        if let Some(at) = recipe::find(&data, ESCHA_FIRST_RECIPE) {
            let recipes = recipe::parse(&data, at).map_err(|e| format!("{}: recipes: {e}", exe.display()))?;
            let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
            let (names, first) = pe.enum_names("ITEM_CATEGORY_PLANT").ok_or("no item category enum")?;
            let categories: Vec<String> = names[first..].iter().map(|n| n.to_string()).collect();
            found = Some((recipes, categories));
            break;
        }
    }
    let (recipes, categories) = found.ok_or("no executable in the install folder holds a recipe table")?;
    let item = number(item)?;
    let recipe = recipes.iter().find(|r| r.item as usize == item).ok_or("that item has no recipe")?;
    let items = table_rows(&install.join("Saves_EN/item/itemData.xml"))?;
    let effects = table_rows(&install.join("Saves_EN/item/item_effect.xml"))?;
    let name = |rows: &[barrel_formats::table::Element], n: usize, attr: &str| {
        rows.get(n).and_then(|r| r.attr(attr)).unwrap_or("?").to_string()
    };
    println!("{} ({item})", name(&items, item, "ItemName"));
    let wanted = barrel_escha::synthesis::order(recipe);
    // The cost points to spend: BARREL_CP, or 99 (more than any alchemist starts with).
    let cp = std::env::var("BARREL_CP").ok().and_then(|v| v.parse().ok()).unwrap_or(99);
    let mut s = Synthesis::with_cp(cp);
    for (i, m) in materials.iter().enumerate() {
        let m = number(m)?;
        let row = items.get(m).ok_or_else(|| format!("no item {m}"))?;
        let cats: Vec<u16> = (0..4)
            .filter_map(|k| row.attr(&format!("category_{k:02}")))
            .filter_map(|tag| categories.iter().position(|c| c == tag))
            .map(|n| n as u16)
            .collect();
        match wanted.get(i) {
            Some(&w) if barrel_escha::synthesis::fits(w, m as u16, &cats) => {}
            Some(w) => println!("  ! material {} does not fit {w:?} (the game takes them in order without Freestyle)", i + 1),
            None => println!("  ! material {} is one more than the recipe takes ({})", i + 1, wanted.len()),
        }
        let material = Material::from_item(row);
        if !s.add(&material) {
            println!("  ! {} ({m}) costs {} CP, {} left: not taken", name(&items, m, "ItemName"), material.cp, s.cp);
            continue;
        }
        println!("  + {} ({m}, {} CP, {} left): attributes {:?}, effect value {}", name(&items, m, "ItemName"), material.cp, s.cp, s.attributes, s.effect_value());
    }
    if materials.len() < wanted.len() {
        println!("  ! the recipe takes {} materials", wanted.len());
    }
    for (k, (element, e)) in ["Fire", "Water", "Wind", "Earth"].iter().zip(s.effects(&recipe.lines)).enumerate() {
        let what = e.map_or("-".to_string(), |e| name(&effects, e as usize, "ItemEffectName"));
        println!("  {element:5} {:>3}: {what}", s.attributes[k] as i32);
    }
    // The finished item (no traits picked, one made, a first synthesis at alchemy level 1).
    let r = recipes.iter().position(|x| std::ptr::eq(x, recipe)).unwrap_or(0);
    let int = |n: usize, attr: &str| items.get(n).and_then(|row| row.attr(attr)).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let f = barrel_escha::synthesis::Finish { recipe: (r, recipe), level: int(item, "level"), equip_cost: int(item, "EquipCost"), traits: [-1; 3], trait_table: &[], count: 1, exp_rate: 0, alchemy_level: 1, categories: [-1; 4], mix: &[], extra_uses: 0, compression: 0 };
    let mut k = barrel_escha::data::save::Knowledge { item: vec![0; 560], recipe: vec![0; 265], eff: vec![0; 698], potential: vec![0; 400], mix_esp: vec![0; 90], ..Default::default() };
    let mut revealed = vec![[false; 10]; recipes.len()];
    let (unit, exp) = barrel_escha::synthesis::finish(&s, &f, 0, &mut k, &mut revealed, &mut |lo, _| lo);
    let made: Vec<String> = unit.effects.iter().filter(|&&e| e >= 0).map(|&e| name(&effects, e as usize, "ItemEffectName")).collect();
    println!("made: quality {}, uses {}, equip cost {}, effects {}; {exp} experience; recipe effects revealed {:?}", unit.quality, unit.counts[1], unit.equip_cost, made.join(", "), (0..10).filter(|&j| revealed[r][j]).collect::<Vec<_>>());
    Ok(())
}

fn xml_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".xml", &mut files)?;
    files.sort();
    let mut failed = Vec::new();
    for path in &files {
        match barrel_formats::table::parse(&read(path)?) {
            Ok(root) => {
                let mut rows: BTreeMap<&str, usize> = BTreeMap::new();
                for c in &root.children {
                    *rows.entry(c.name.as_str()).or_default() += 1;
                }
                let rel = path.strip_prefix(dir).unwrap_or(path);
                let summary: Vec<String> = rows.iter().map(|(k, n)| format!("{n} {k}")).collect();
                println!("{}: {}", rel.display(), summary.join(", "));
            }
            Err(err) => failed.push(format!("{}: {err}", path.display())),
        }
    }
    println!("{} tables, {} failed", files.len(), failed.len());
    for f in &failed {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some tables did not parse".into()) }
}

/// The bytes of FILE, or of entry N of a container written FILE#N.
fn file_or_entry(spec: &str) -> Result<Vec<u8>> {
    if let Some((file, n)) = spec.rsplit_once('#') {
        let n: usize = n.parse()?;
        let bytes = read(Path::new(file))?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{file}: {err}"))?;
        let entry = e.entries.get(n).ok_or("no such entry")?;
        return Ok(e.data(entry).to_vec());
    }
    read(Path::new(spec))
}

fn g1t(spec: &str, out: &Path) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let g = barrel_formats::g1t::G1t::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    std::fs::create_dir_all(out)?;
    for (i, t) in g.textures.iter().enumerate() {
        let name = out.join(format!("{i:03}.png"));
        std::fs::write(&name, barrel_formats::png::encode_rgba(t.width, t.height, &t.to_rgba8()))?;
        println!("{} {}x{} {:?} ({:#04x}), {} levels, {} layers", name.display(), t.width, t.height, t.format, t.format_code, t.levels.len(), t.layers);
        // Every further layer of the largest level (cube faces, array or volume slices).
        for l in 1..t.layers {
            if let Some(px) = t.layer_rgba8(l) {
                std::fs::write(out.join(format!("{i:03}_{l}.png")), barrel_formats::png::encode_rgba(t.width, t.height, &px))?;
            }
        }
    }
    Ok(())
}

fn g1t_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".g1t", &mut files)?;
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut packs, mut textures) = (0usize, 0usize);
    let mut formats: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed = Vec::new();
    let mut check = |name: String, data: &[u8], failed: &mut Vec<String>| match barrel_formats::g1t::G1t::parse(data) {
        Ok(g) => {
            packs += 1;
            for t in &g.textures {
                textures += 1;
                *formats.entry(format!("{:?} ({:#04x})", t.format, t.format_code)).or_default() += 1;
            }
        }
        Err(e) => failed.push(format!("{name}: {e}")),
    };
    for path in &files {
        let bytes = read(path)?;
        if path.to_string_lossy().ends_with(".g1t") {
            check(path.display().to_string(), &bytes, &mut failed);
            continue;
        }
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            let data = e.data(entry);
            if data.starts_with(b"GT1G") {
                check(format!("{}#{i}", path.display()), data, &mut failed);
            }
        }
    }
    println!("{packs} texture packs, {textures} textures, {} failed", failed.len());
    for (f, n) in &formats {
        println!("{n:>7}  {f}");
    }
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some textures did not read".into()) }
}

/// A model's chunks, joints (parent, place in model space) and the joints each submesh's
/// palette names.
fn g1m_bones(spec: &str) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let m = barrel_formats::g1m::G1m::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    let magics: Vec<String> = m.chunks.iter().map(|c| format!("{} ({} bytes)", String::from_utf8_lossy(&c.magic), c.body.len())).collect();
    println!("chunks: {}", magics.join(", "));
    if let Some(sk) = &m.skeleton {
        let world = sk.world_positions();
        println!("{} joints, index map {} entries, layer {}", sk.joints.len(), sk.index_map.len(), sk.layer);
        let odd: Vec<String> = sk.index_map.iter().enumerate().filter(|(i, v)| **v as usize != *i).take(40).map(|(i, v)| format!("{i}->{v:#x}")).collect();
        println!("index map entries not to themselves: {}", odd.join(" "));
        for (i, j) in sk.joints.iter().enumerate() {
            println!("  joint {i}: parent {:?} at {:.2?}", j.parent, world[i]);
        }
    }
    if let Some(g) = &m.geometry {
        for (k, s) in g.submeshes.iter().enumerate() {
            let map = g.bone_maps.get(s.bone_map).map(|p| p.iter().map(|r| format!("{}{}{}", if r.external { "x" } else { "" }, r.joint, if r.flags != 0x8000_0000 { format!("/f{:x}", r.flags) } else { String::new() })).collect::<Vec<_>>().join(" ")).unwrap_or_default();
            println!("submesh {k}: palette {} [{map}]", s.bone_map);
        }
    }
    Ok(())
}

fn g1m_materials(spec: &str) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let m = barrel_formats::g1m::G1m::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    let g = m.geometry.as_ref().ok_or("model has no geometry")?;
    for (i, mat) in g.materials.iter().enumerate() {
        let users: Vec<usize> = g.submeshes.iter().enumerate().filter_map(|(k, s)| (g.simple_mesh(s).ok()?.material == i).then_some(k)).collect();
        let tex: Vec<String> = mat.textures.iter().map(|t| format!("#{} kind {} sub {} layer {}", t.index, t.kind, t.subtype, t.layer)).collect();
        // Vertex colours over the material's submeshes: how many vertices have them, and their average.
        let (mut n, mut sum) = (0usize, [0f32; 4]);
        for s in g.submeshes.iter().filter_map(|s| g.simple_mesh(s).ok()).filter(|m| m.material == i) {
            for c in &s.colors {
                n += 1;
                for k in 0..4 {
                    sum[k] += c[k];
                }
            }
        }
        let colours = if n == 0 { String::new() } else { format!(", vertex colours on {n} vertices, average {:.2?}", sum.map(|v| v / n as f32)) };
        println!("material {i}: {} (submeshes {users:?}){colours}", tex.join(", "));
    }
    for (k, l) in g.lights.iter().enumerate() {
        println!("light {k}: diffuse {:?} ambient {:?}", l.diffuse, l.ambient);
    }
    for (kind, count, body) in &g.other_sections {
        let words: Vec<String> = body.chunks_exact(4).take(400).map(|w| format!("{:x}", u32::from_le_bytes(w.try_into().unwrap()))).collect();
        println!("section {kind:#x}: {count} entries, {} bytes: {}", body.len(), words.join(" "));
    }
    for m in &g.mesh_groups {
        println!("lod {} pass {} shader {} {:?}: submeshes {:?}", m.lod, if m.group == 0 { "colour" } else { "shadow" }, m.shader, m.words, m.submeshes);
    }
    for (k, m) in g.submeshes.iter().filter_map(|s| g.simple_mesh(s).ok()).enumerate().filter(|(_, m)| !m.corners.is_empty()) {
        let first: Vec<String> = m.corners.iter().take(4).map(|c| format!("{c:.1?}")).collect();
        println!("submesh {k}: billboard, {} corners, first quad {}", m.corners.len(), first.join(" "));
    }
    for (k, l) in g.layouts.iter().enumerate() {
        let attrs: Vec<String> = l.attributes.iter().map(|a| format!("{:?}{} {:?} @{}:{}", a.semantic, a.layer, a.kind, a.buffer, a.offset)).collect();
        println!("layout {k} (buffers {:?}): {}", l.buffers, attrs.join(", "));
    }
    for (k, set) in g.shader_params.iter().enumerate() {
        let ps: Vec<String> = set.iter().map(|p| format!("{}={:?}", p.name, p.values)).collect();
        println!("shader parameters {k}: {}", ps.join(" "));
    }
    Ok(())
}

fn g1m_obj(spec: &str, out: &Path) -> Result<()> {
    use std::fmt::Write as _;
    let bytes = file_or_entry(spec)?;
    let m = barrel_formats::g1m::G1m::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    let g = m.geometry.as_ref().ok_or("model has no geometry")?;
    let mut obj = String::new();
    let mut base = 1u32;
    for (i, s) in g.submeshes.iter().enumerate() {
        let mesh = g.simple_mesh(s)?;
        writeln!(obj, "o submesh{i}_material{}", mesh.material)?;
        for p in &mesh.positions {
            writeln!(obj, "v {} {} {}", p[0], p[1], p[2])?;
        }
        for t in &mesh.uvs {
            writeln!(obj, "vt {} {}", t[0], 1.0 - t[1])?;
        }
        for n in &mesh.normals {
            writeln!(obj, "vn {} {} {}", n[0], n[1], n[2])?;
        }
        let (has_uv, has_n) = (!mesh.uvs.is_empty(), !mesh.normals.is_empty());
        for t in &mesh.triangles {
            let f: Vec<String> = t
                .iter()
                .map(|&v| {
                    let v = v + base;
                    match (has_uv, has_n) {
                        (true, true) => format!("{v}/{v}/{v}"),
                        (true, false) => format!("{v}/{v}"),
                        (false, true) => format!("{v}//{v}"),
                        (false, false) => format!("{v}"),
                    }
                })
                .collect();
            writeln!(obj, "f {}", f.join(" "))?;
        }
        base += mesh.positions.len() as u32;
    }
    std::fs::write(out, obj)?;
    println!("{} submeshes written to {}", g.submeshes.len(), out.display());
    Ok(())
}

fn swg(spec: &str, oid: &str) -> Result<()> {
    let s = barrel_formats::swg::parse(&file_or_entry(spec)?).map_err(|e| format!("{spec}: {e}"))?;
    let list = barrel_formats::oid::parse(&file_or_entry(oid)?).map_err(|e| format!("{oid}: {e}"))?;
    let names = barrel_formats::oid::by_id(&list);
    println!("{} ({}): {} records", s.name, s.value, s.records.len());
    for r in &s.records {
        let f: Vec<String> = r.floats.iter().map(|v| format!("{v}")).collect();
        println!("  {:3} {:4} {:22} [{}] {:?}", r.group, r.bone, names.get(&(r.bone as u32)).copied().unwrap_or("?"), f.join(" "), r.words);
    }
    Ok(())
}

fn g1a(spec: &str, bone: Option<u32>) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let a = barrel_formats::g1a::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    println!("{:.3} s, {} tracks", a.duration, a.tracks.len());
    if let Some(bone) = bone {
        for t in a.tracks.iter().filter(|t| t.bone == bone) {
            let mut x = 0.0;
            while x <= a.duration + 1e-4 {
                match t.pose(x) {
                    Some((r, p, s)) => println!("  {x:5.2} s  position {p:8.2?}  rotation {r:6.3?}  scale {s:5.2?}"),
                    None => println!("  {x:5.2} s  rotation {:6.3?}", t.rotation(x)),
                }
                x += 0.1;
            }
        }
        return Ok(());
    }
    for t in &a.tracks {
        let times = [0.0, a.duration * 0.25, a.duration * 0.5, a.duration * 0.75, a.duration];
        match t.scalar(0.0) {
            Some(_) => {
                let v: Vec<String> = times.iter().map(|&x| format!("{:.2}", t.scalar(x).unwrap())).collect();
                println!("  bone {:4} weight {}", t.bone, v.join(" "));
            }
            None => println!("  bone {:4} flag {:#x}, {} components", t.bone, t.flag, t.components.len()),
        }
    }
    Ok(())
}

/// Which bones an animation turns, and by how much at most (degrees from their first frame),
/// named from a bone list (`*Oid.bin`) when one is given.
fn g1a_moves(spec: &str, oid: Option<&str>) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let a = barrel_formats::g1a::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    let names = match oid {
        Some(o) => barrel_formats::oid::parse(&file_or_entry(o)?).map_err(|e| format!("{o}: {e}"))?,
        None => Vec::new(),
    };
    let frames = (a.duration * 30.0).round() as usize;
    for t in &a.tracks {
        let Some((first, ..)) = t.pose(0.0) else { continue };
        let most = (0..=frames)
            .filter_map(|f| t.pose(f as f32 / 30.0))
            .map(|(q, ..)| {
                let dot: f32 = q.iter().zip(&first).map(|(a, b)| a * b).sum::<f32>().abs().min(1.0);
                2.0 * dot.acos().to_degrees()
            })
            .fold(0.0f32, f32::max);
        if most > 0.5 {
            let name = names.get(t.bone as usize).map_or("", String::as_str);
            println!("  bone {:4} {name:24} turns up to {most:.1}°", t.bone);
        }
    }
    Ok(())
}

fn g1h(spec: &str) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let sets = barrel_formats::g1h::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    for s in &sets {
        let moved: Vec<usize> = s
            .targets
            .iter()
            .map(|t| t.iter().filter(|v| v.iter().any(|c| c.abs() > 1e-5)).count())
            .collect();
        let n = s.targets.first().map_or(0, |t| t.len());
        println!("layout {}: {} targets of {n} vertices; vertices moved: {moved:?}", s.layout, s.targets.len());
    }
    Ok(())
}

/// Read every morph file in the containers under `dir` and check it against the model of
/// the same name: each set's layout must exist and its targets must have that layout's
/// vertex count.
fn g1h_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut ok, mut sets, mut targets) = (0usize, 0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for entry in &e.entries {
            let data = e.data(entry);
            if !data.starts_with(b"_H1G") {
                continue;
            }
            let stem = entry.name.trim_end_matches(".g1h");
            let base = e.entries.iter().find(|b| b.name == format!("{stem}.g1m")).map(|b| e.data(b));
            let result = barrel_formats::g1h::parse(data).and_then(|parsed| {
                let model = barrel_formats::g1m::G1m::parse(base.ok_or(barrel_formats::Error::Corrupt("no base model"))?)?;
                let g = model.geometry.ok_or(barrel_formats::Error::Corrupt("base model has no geometry"))?;
                for s in &parsed {
                    let layout = g.layouts.get(s.layout as usize).ok_or(barrel_formats::Error::Corrupt("set layout not in the base model"))?;
                    let count = g.vertex_buffers.get(layout.buffers[0] as usize).map_or(0, |b| b.count);
                    if s.targets.iter().any(|t| t.len() != count) {
                        return Err(barrel_formats::Error::Corrupt("target vertex count differs from its layout"));
                    }
                    targets += s.targets.len();
                }
                sets += parsed.len();
                Ok(())
            });
            match result {
                Ok(()) => ok += 1,
                Err(err) => failed.push(format!("{}: {}: {err}", path.display(), entry.name)),
            }
        }
    }
    println!("{ok} morph files read and matched to their models: {sets} sets, {targets} targets; {} failed", failed.len());
    for f in failed.iter().take(10) {
        println!("  {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some morph files did not read".into()) }
}

fn g1m_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut models, mut submeshes, mut vertices, mut triangles) = (0usize, 0usize, 0usize, 0usize);
    let (mut skeletons, mut joints) = (0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            let data = e.data(entry);
            if !data.starts_with(b"_M1G") {
                continue;
            }
            let result = barrel_formats::g1m::G1m::parse(data).and_then(|m| {
                if let Some(s) = &m.skeleton {
                    skeletons += 1;
                    joints += s.joints.len();
                }
                let g = m.geometry.ok_or(barrel_formats::Error::Corrupt("no geometry"))?;
                for s in &g.submeshes {
                    if let Some(slot) = g.max_bone_slot(s) {
                        let palette = g.bone_maps.get(s.bone_map).map_or(0, |m| m.len());
                        if slot >= palette {
                            return Err(barrel_formats::Error::Corrupt("vertex bone index past its palette"));
                        }
                    }
                    let mesh = g.simple_mesh(s)?;
                    vertices += mesh.positions.len();
                    triangles += mesh.triangles.len();
                }
                Ok(g.submeshes.len())
            });
            match result {
                Ok(n) => {
                    models += 1;
                    submeshes += n;
                }
                Err(err) => failed.push(format!("{}#{i}: {err}", path.display())),
            }
        }
    }
    println!("{models} models read, {} failed: {submeshes} submeshes, {vertices} vertices, {triangles} triangles", failed.len());
    println!("{skeletons} skeletons, {joints} joints");
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some models did not read".into()) }
}

fn oid_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut lists, mut names) = (0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            if !entry.name.ends_with("Oid.bin") {
                continue;
            }
            match barrel_formats::oid::parse(e.data(entry)) {
                Ok(n) => {
                    lists += 1;
                    names += n.len();
                }
                Err(err) => failed.push(format!("{}#{i}: {err}", path.display())),
            }
        }
    }
    println!("{lists} name lists, {names} names, {} failed", failed.len());
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some lists did not read".into()) }
}

fn ebm(file: &Path) -> Result<()> {
    let msgs = barrel_formats::ebm::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    for m in &msgs {
        println!("[{} speaker {} {:?}] {}", m.kind, m.speaker, m.fields, m.repaired_text().replace("<CR>", " / "));
    }
    Ok(())
}

fn ebm_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".ebm", &mut files)?;
    files.sort();
    let (mut messages, mut repaired) = (0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        match barrel_formats::ebm::parse(&read(path)?) {
            Ok(msgs) => {
                messages += msgs.len();
                repaired += msgs.iter().filter(|m| m.repaired_text() != m.text).count();
            }
            Err(e) => failed.push(format!("{}: {e}", path.display())),
        }
    }
    println!("{} files, {messages} messages ({repaired} repaired from mangled Shift-JIS), {} failed", files.len(), failed.len());
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some message files did not read".into()) }
}

/// A field's placement list: each object type, its instance count and their tags.
fn opl2_list(spec: &str) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let p = barrel_formats::opl2::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    for t in &p.objects {
        let mut tags: Vec<&str> = t.instances.iter().map(|i| i.tags.as_str()).collect();
        tags.sort();
        tags.dedup();
        println!("{:4} {:40} tags {:?}", t.instances.len(), t.name, tags);
        if std::env::var("BARREL_VERBOSE").is_ok() {
            for i in &t.instances {
                println!("       at {:.1?} turned {:.1?} scaled {:.2?} {}", i.position, i.rotation, i.scale, i.tags);
            }
        }
    }
    for t in &p.effects {
        println!("{:4} effect {:33} at {:.1?}", t.instances.len(), t.name, t.instances.first().map(|i| i.position));
    }
    Ok(())
}

fn opl2_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut lists, mut objects, mut effects) = (0usize, 0usize, 0usize);
    let mut failed = Vec::new();
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            if !entry.name.ends_with(".opl2") {
                continue;
            }
            match barrel_formats::opl2::parse(e.data(entry)) {
                Ok(p) => {
                    lists += 1;
                    objects += p.objects.iter().map(|t| t.instances.len()).sum::<usize>();
                    effects += p.effects.iter().map(|t| t.instances.len()).sum::<usize>();
                }
                Err(err) => failed.push(format!("{}#{i}: {err}", path.display())),
            }
        }
    }
    println!("{lists} placement lists: {objects} objects, {effects} effects; {} failed", failed.len());
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some lists did not read".into()) }
}

/// `opl2-scan` over PAK archives: each field container's placement list, read in memory.
fn opl2_scan_paks(paks: &[&str]) -> Result<()> {
    let (mut lists, mut objects, mut effects, mut types) = (0usize, 0usize, 0usize, 0usize);
    let mut failed = Vec::new();
    for file in paks {
        let archive = open_pak(Path::new(file))?;
    let pak = &archive.pak;
        for entry in pak.entries.iter().filter(|e| e.name.ends_with(".elixir.gz") && e.name.contains("\\field\\") && !e.name.ends_with("_obj.elixir.gz") && !e.name.ends_with("_env.elixir.gz")) {
            let data = archive.read(entry).map_err(|e| format!("{file}: {e}"))?;
            let Ok(c) = Elixir::parse(&data) else { continue };
            for e in c.entries.iter().filter(|e| e.name.ends_with(".opl2")) {
                match barrel_formats::opl2::parse(c.data(e)) {
                    Ok(p) => {
                        lists += 1;
                        types += p.objects.len();
                        objects += p.objects.iter().map(|t| t.instances.len()).sum::<usize>();
                        effects += p.effects.iter().map(|t| t.instances.len()).sum::<usize>();
                    }
                    Err(err) => failed.push(format!("{}: {err}", entry.name)),
                }
            }
        }
    }
    println!("{lists} placement lists: {types} object types, {objects} objects, {effects} effects; {} failed", failed.len());
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some lists did not read".into()) }
}

/// Ryza DX fields through `barrel_engine::field::load` (no tags read yet: nothing casts or
/// sways), with what the scene got.
fn ryza_field(root: &Path, only: Option<&str>) -> Result<()> {
    let install = barrel_ryza::install::Install::open(root)?;
    let names = match only {
        Some(f) => vec![f.to_string()],
        None => install.fields(),
    };
    let mut failed = 0;
    for name in &names {
        let loaded = (|| -> Result<String> {
            let f = install.field(name)?;
            let (main, objects) = (Elixir::parse(&f.main)?, Elixir::parse(&f.objects)?);
            let mut scene = barrel_render::Scene::new();
            let place = |_: &barrel_formats::opl2::PlacedType, _: &barrel_formats::opl2::Instance| barrel_engine::field::Placement::default();
            let (pack, _) = barrel_engine::field::load(&f.area, &main, &objects, &mut scene, 0, &place)?;
            let textures = barrel_formats::g1t::G1t::parse(&pack)?.textures.len();
            if let Some(ktf) = barrel_engine::field::named(&main, &format!("{}.ktf", f.area))
                && !barrel_formats::ktf::parse(ktf)?.layers.textures.is_empty()
            {
                let m = barrel_formats::ktf::mesh(ktf)?;
                let mut kinds = std::collections::BTreeMap::new();
                for d in m.draws.iter().filter(|d| d.kind != 0) {
                    *kinds.entry((d.kind, d.flags)).or_insert(0usize) += 1;
                }
                println!(
                    "terrain: {} patches ({} leaves), {} draws, {} vertices, {} indices; not depth draws by (kind, flags {{program, kind}}): {kinds:?}",
                    m.patches.len(),
                    m.patches.iter().filter(|p| p.leaf).count(),
                    m.draws.len(),
                    m.vertices.len(),
                    m.indices.len()
                );
            }
            let lights = barrel_engine::field::named(&main, &format!("{}_light.bin", f.area)).map(barrel_ryza::light::parse).transpose()?.map_or(0, |l| l.len());
            Ok(format!("{} draws, {} textures, {} vertices, {lights} lights, env {}", scene.draws.len(), textures, scene.vertices.len(), if f.env.is_some() { "yes" } else { "no" }))
        })();
        match loaded {
            Ok(s) => println!("{name}: {s}"),
            Err(e) => {
                failed += 1;
                println!("{name}: FAILED {e}");
            }
        }
    }
    if names.len() > 1 {
        println!("{} fields, {failed} failed", names.len());
    }
    if failed == 0 { Ok(()) } else { Err("some fields did not load".into()) }
}

fn ktf_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let mut terrains = 0usize;
    let mut layers: BTreeMap<usize, usize> = BTreeMap::new();
    let mut blends: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut failed = Vec::new();
    let (mut meshes, mut heights) = (0usize, (0usize, 0usize));
    let mut layer_records_differ = 0usize;
    let mut draw_flags: std::collections::BTreeMap<[u8; 2], usize> = std::collections::BTreeMap::new();
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            if !entry.name.ends_with(".ktf") {
                continue;
            }
            match barrel_formats::ktf::parse(e.data(entry)) {
                Ok(t) => {
                    terrains += 1;
                    *layers.entry(t.layers.textures.len()).or_default() += 1;
                    *blends.entry((t.layers.textures.len(), t.blend.textures.len())).or_default() += 1;
                    let list = t.layer_list();
                    if list.len() != t.layers.textures.len() || list.iter().enumerate().any(|(k, l)| usize::from(l.colour) != k || l.normal.is_some()) {
                        layer_records_differ += 1;
                        println!("  {}: {} textures, layers {list:?}", path.display(), t.layers.textures.len());
                    }
                    match barrel_formats::ktf::mesh(e.data(entry)) {
                        Ok(m) => {
                            meshes += 1;
                            for d in &m.draws {
                                *draw_flags.entry(d.flags).or_default() += 1;
                            }
                            // The vertices' heights against the heightmap's at the same cell.
                            let hm = &t.heightmap;
                            let (mut near, mut all) = (0usize, 0usize);
                            for p in &m.patches {
                                let end = m.patches.iter().map(|q| q.first_vertex).filter(|&v| v > p.first_vertex).min().unwrap_or(m.vertices.len() as u32);
                                for v in &m.vertices[p.first_vertex as usize..end as usize] {
                                    if v.x >= 0 && v.z >= 0 && (v.x as usize) <= hm.cells_x && (v.z as usize) <= hm.cells_z {
                                        all += 1;
                                        near += usize::from((p.height(v.height) - hm.height(v.x as usize, v.z as usize)).abs() < 0.5);
                                    }
                                }
                            }
                            if all > 0 && near * 10 < all * 8 {
                                println!("  {}: {near} of {all} near, cell {} / heightmap {}", entry.name, m.cell_size, hm.cell_size);
                            }
                            heights.0 += near;
                            heights.1 += all;
                        }
                        Err(err) => failed.push(format!("{}#{i} mesh: {err}", path.display())),
                    }
                }
                Err(err) => failed.push(format!("{}#{i}: {err}", path.display())),
            }
        }
    }
    println!("{terrains} terrains, {} failed; layer counts {layers:?}", failed.len());
    println!("(layers, blend maps) {blends:?}");
    println!("{meshes} meshes read; {} of {} vertices within 0.5 of the heightmap", heights.0, heights.1);
    println!("draws by flags {{program, kind}}: {draw_flags:?}");
    println!("{layer_records_differ} terrains whose layer records are not texture k for layer k without normal maps");
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some terrains did not read".into()) }
}

fn g1a_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let (mut anims, mut tracks, mut keys, mut packed) = (0usize, 0usize, 0usize, 0usize);
    let mut failed: BTreeMap<String, usize> = BTreeMap::new();
    let mut example = None;
    for path in &files {
        let bytes = read(path)?;
        let e = Elixir::parse(&bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        for (i, entry) in e.entries.iter().enumerate() {
            let data = e.data(entry);
            if !data.starts_with(b"_A1G") {
                continue;
            }
            match barrel_formats::g1a::parse(data) {
                Ok(a) => {
                    anims += 1;
                    tracks += a.tracks.len();
                    packed += a.tracks.iter().filter(|t| t.flag == barrel_formats::g1a::SCALAR).count();
                    keys += a.tracks.iter().flat_map(|t| &t.components).map(|c| c.len()).sum::<usize>();
                }
                Err(err) => {
                    *failed.entry(err.to_string()).or_default() += 1;
                    example.get_or_insert_with(|| format!("{}#{i}", path.display()));
                }
            }
        }
    }
    println!("{anims} animations read: {tracks} tracks ({packed} single-scalar morph tracks), {keys} keys; failures {failed:?}");
    if let Some(e) = example {
        println!("first failure: {e}");
    }
    if failed.is_empty() { Ok(()) } else { Err("some animations did not read".into()) }
}

/// The message file for an event: Event[_LANG]/event/<dir>/EVENT_MESSAGE_<name>.ebm.
fn message_file(event: &Path, lang: Option<&str>) -> Option<PathBuf> {
    // Ryza DX's, unpacked from its archives: `event/event_jp/<chapter>/event_<name>.ebd`, the
    // messages in `event/event_<lang>/<chapter>/event_message_<name>.ebm` (Japanese by default).
    if let Some(name) = event.file_name()?.to_str()?.strip_prefix("event_").and_then(|n| n.strip_suffix(".ebd")) {
        let dir = event.parent()?;
        let lang_dir = dir.parent()?.with_file_name(format!("event_{}", lang.unwrap_or("jp").to_ascii_lowercase()));
        return Some(lang_dir.join(dir.file_name()?).join(format!("event_message_{name}.ebm")));
    }
    let name = event.file_name()?.to_str()?.strip_prefix("EVENT_")?.strip_suffix(".ebd")?;
    let dir = event.parent()?;
    let chapter = dir.file_name()?;
    let root = dir.parent()?.parent()?; // .../Event
    let base = match lang {
        Some(l) => root.with_file_name(format!("Event_{l}")),
        None => root.to_path_buf(),
    };
    Some(base.join("event").join(chapter).join(format!("EVENT_MESSAGE_{name}.ebm")))
}

/// Runs an event's timed steps on a stage and prints, whenever the clock moves on, where every
/// character and the camera are (positions in field units, facings in degrees).
fn stage_event(file: &Path, fps: Option<&str>) -> Result<()> {
    let fps: Option<f64> = fps.map(|h| h.parse()).transpose().map_err(|_| "--fps HZ is a number")?;
    use barrel_engine::event::Player;
    use barrel_engine::stage::Stage;
    let event = Ebd::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    let table = OperandTable::escha();
    let messages = match message_file(file, None) {
        Some(m) if m.is_file() => barrel_formats::ebm::parse(&read(&m)?).unwrap_or_default(),
        _ => Vec::new(),
    };
    let steps = Player::new(&event, &messages, &table)?.schedule();
    // Story events number their characters by Chara.xml row.
    let measures = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| read(&root.join("Saves/chara/Chara.xml")).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_engine::stage::Measures::table(&t))
        .unwrap_or_default();
    let mut last = None;
    let show = |p: [f32; 3]| format!("({:.0}, {:.0}, {:.0})", p[0], p[1], p[2]);
    let print = |stage: &Stage| {
        let mut parts: Vec<String> = stage
            .characters
            .iter()
            .map(|(n, b)| {
                let face = match (b.expression, b.talking) {
                    (Some(e), true) => format!(" face {e}, talking"),
                    (Some(e), false) => format!(" face {e}"),
                    (None, true) => " talking".to_string(),
                    (None, false) => String::new(),
                };
                format!("{n}{} {} {:.0}°{face}", if b.moving() { "*" } else { "" }, show(b.position), b.facing.to_degrees())
            })
            .collect();
        if let (Some(e), Some(t)) = (stage.eye.position, stage.target.position) {
            parts.push(format!("camera {} -> {}", show(e), show(t)));
        }
        println!("{:8.2}  {}", stage.time, parts.join("  "));
    };
    let curves = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| read(&root.join("Saves/curve/curve_data.xml")).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_engine::curve::Curve::table(&t))
        .unwrap_or_default();
    println!("({} curves)", curves.len());
    // As a renderer drawing HZ frames a second drives it: the clock decides the ticks; the
    // end state must not depend on HZ (docs/engine.md, "Timing").
    if let Some(hz) = fps {
        use barrel_engine::clock::Clock;
        let end = steps.last().map_or(0.0, |t| t.time as f64) + 1.0;
        let mut clock = Clock::default();
        let mut play = barrel_engine::stage::Playback::new(Stage { measures, curves, ..Stage::default() }, steps);
        let mut frames = 0u64;
        while clock.time() < end {
            for _ in 0..clock.advance(1.0 / hz).ticks {
                play.tick();
            }
            frames += 1;
        }
        println!("{frames} frames at {hz} Hz, {} ticks", clock.ticks);
        print(&play.stage);
        return Ok(());
    }
    let stage = Stage { measures, curves, ..Stage::default() }.play(&steps, |t, stage| {
        if last != Some(t.time) {
            last = Some(t.time);
            print(stage);
        }
    });
    print(&stage);
    println!("({} positions could not be placed; * = moving)", stage.unresolved);
    Ok(())
}

fn play_event(file: &Path, lang: Option<&str>, timed: bool) -> Result<()> {
    let event = Ebd::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    play(file, event, lang, timed)
}

/// Plays an event, then the events the story starts after it in a new game, one after another
/// (`event_start::Story`: each event seen and its process data applied, its end firing group
/// 11), until none starts.
fn play_chain(file: &Path, lang: Option<&str>) -> Result<()> {
    let root = file.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("the event is not inside a game install")?.to_path_buf();
    let mut chain = story_chain(&root)?;
    let mut path = file.to_path_buf();
    for _ in 0..100 {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        println!("=== {stem}");
        play_event(&path, lang, false)?;
        let Some(next) = chain.after(&stem) else {
            println!("=== (no event starts when {stem} ends)");
            break;
        };
        path = root.join("Event/event").join(barrel_escha::event_start::event_file(next));
        if !path.is_file() {
            return Err(format!("{}: no such event file", path.display()).into());
        }
    }
    Ok(())
}

/// The story from a new game, over the install at ROOT (`event_start::Chain`).
/// Prints an event's start conditions (`EventCheckDataAll.ecd`): its words, then each group's
/// commands by name with their operands, triggers marked with their group.
fn event_check(root: &Path, event: &str) -> Result<()> {
    let checks = barrel_escha::data::event_check::parse(&read(&root.join("Event/eventCheck/EventCheckDataAll.ecd"))?).map_err(|e| e.to_string())?;
    let exe_data = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe_data).map_err(|e| e.to_string())?;
    let (names, _) = pe.enum_names("EVENT_MM01_OP1_111").ok_or("no EVENT_* enum")?;
    let n = event.parse::<usize>().ok().or_else(|| names.iter().position(|n| *n == event)).ok_or("no such event")?;
    let c = checks.get(n).ok_or("no check for that event")?;
    println!("{} (event {n}): words {:?}", names.get(n).copied().unwrap_or("?"), c.words);
    for (g, group) in c.groups.iter().enumerate() {
        for cmd in barrel_escha::data::event_check::commands(group).map_err(|e| e.to_string())? {
            let trigger = cmd.trigger().map_or(String::new(), |t| format!(" [trigger group {t}]"));
            println!("  group {g}: {} {:?}{trigger}", cmd.name(), cmd.operands);
        }
    }
    Ok(())
}

/// A new game run headless: each event played to its end (its steps read for the field it
/// leaves the player in), until free play.
fn new_game(root: &Path, logy: bool) -> Result<()> {
    use barrel_escha::session::{Scene, Session};
    let exe = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| e.to_string())?;
    let maps = barrel_escha::data::maps::field_maps(&pe);
    let mut session = Session::new_game(story_chain(root)?, logy);
    let table = OperandTable::escha();
    for _ in 0..100 {
        let name = match session.scene() {
            Scene::Event(name) => name.clone(),
            Scene::Field(map) => {
                println!("free play in field {map} ({})", maps.get(*map as usize).map_or("?", |s| s.as_str()));
                // The session's save there (the spot unknown here: 0, 0).
                print!("{}", session.save([0.0, 0.0]).unwrap_or_default());
                return Ok(());
            }
        };
        let file = root.join("Event/event").join(barrel_escha::event_start::event_file(&name));
        let event = Ebd::parse(&read(&file)?).map_err(|e| format!("{}: {e}", file.display()))?;
        let messages = match message_file(&file, None) {
            Some(m) if m.is_file() => barrel_formats::ebm::parse(&read(&m)?).unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut player = barrel_engine::event::Player::new(&event, &messages, &table)?;
        if logy {
            player = player.hero(1);
        }
        let steps = player.schedule();
        let last = barrel_engine::event::last_field(&steps);
        let end = steps.last().map_or(0.0, |t| t.time);
        println!("{name}: {end:.1} s{}", last.map_or(String::new(), |m| format!(", ends in field {m} ({})", maps.get(m as usize).map_or("?", |s| s.as_str()))));
        session.event_ended(last);
    }
    Err("the session did not reach free play in 100 scenes".into())
}

fn story_chain(root: &Path) -> Result<barrel_escha::event_start::Chain> {
    let checks = barrel_escha::data::event_check::parse(&read(&root.join("Event/eventCheck/EventCheckDataAll.ecd"))?).map_err(|e| e.to_string())?;
    let process = barrel_escha::data::event_process::parse(&read(&root.join("Event/eventprocess/EventProcessDataAll.epd"))?).map_err(|e| e.to_string())?;
    let exe_data = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe_data).map_err(|e| e.to_string())?;
    let (names, _) = pe.enum_names("EVENT_MM01_OP1_111").ok_or("no EVENT_* enum")?;
    Ok(barrel_escha::event_start::Chain::new(checks, process, names.iter().map(|n| n.to_string()).collect()))
}

/// Plays a script macro (ROOT/Script/FILE.nut's function NAME) as an event with one action,
/// its parameters given the characters ARGS (Chara.xml rows).
fn play_macro(script: &Path, name: &str, args: &[&str], timed: bool) -> Result<()> {
    let root = script.ancestors().find(|p| p.join("Saves").is_dir()).ok_or("no game root above the script")?;
    let exe = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("exe: {e}"))?;
    let names = barrel_escha::macros::Names::escha(&barrel_escha::gesture::motions(&pe));
    let bytes = read(script)?;
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
    let m = barrel_escha::macros::read(&text).into_iter().find(|m| m.name == name).ok_or_else(|| format!("{name}: no such macro"))?;
    let args: Vec<u32> = args.iter().map(|a| a.parse()).collect::<std::result::Result<_, _>>()?;
    let words = m.words(&names, &args).map_err(|u| format!("{name}: {} unknown (arguments: {})", u.0, m.params.join(", ")))?;
    let event = Ebd { actions: vec![barrel_formats::ebd::Action { flags: 0, name: name.as_bytes().to_vec(), description: vec![], words }], resources: vec![] };
    play(script, event, None, timed)
}

/// Prints an event's steps (FILE: where it came from, to find the game's tables).
fn play(file: &Path, event: Ebd, lang: Option<&str>, timed: bool) -> Result<()> {
    use barrel_engine::event::{Player, Step};
    let messages = match message_file(file, lang) {
        Some(m) if m.is_file() => barrel_formats::ebm::parse(&read(&m)?).map_err(|e| format!("{}: {e}", m.display()))?,
        _ => Vec::new(),
    };
    // Speakers are character numbers: rows of Saves[_LANG]/chara/Chara.xml.
    let names: Vec<String> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .map(|root| {
            let lang_table = lang.map(|l| root.join(format!("Saves_{l}/chara/Chara.xml")));
            let table = lang_table.filter(|p| p.is_file()).unwrap_or_else(|| root.join("Saves/chara/Chara.xml"));
            read(&table)
                .ok()
                .and_then(|b| barrel_formats::table::parse(&b).ok())
                .map(|r| r.children.iter().map(|c| c.attr("Name").unwrap_or("").to_string()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    // Motion names (CharaBase columns) from the executable's MOTION_* enum, when the install's
    // executable is found next to Saves.
    let motions: Vec<String> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| {
            ["Atelier_Escha_and_Logy_EN.exe", "Atelier_Escha_and_Logy.exe"].iter().map(|n| root.join(n)).find(|p| p.is_file())
        })
        .and_then(|exe| read(&exe).ok())
        .map(|data| barrel_formats::pe::Pe::parse(&data).map(|pe| barrel_escha::gesture::motions(&pe)).unwrap_or_default())
        .unwrap_or_default();
    let fields: Vec<String> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| ["Atelier_Escha_and_Logy_EN.exe", "Atelier_Escha_and_Logy.exe"].iter().map(|n| root.join(n)).find(|p| p.is_file()))
        .and_then(|exe| read(&exe).ok())
        .map(|data| barrel_formats::pe::Pe::parse(&data).map(|pe| barrel_escha::data::maps::field_maps(&pe)).unwrap_or_default())
        .unwrap_or_default();
    // The music list, from the English executable (where its address is known).
    let tracks: Vec<barrel_escha::data::bgm::Track> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .map(|root| root.join("Atelier_Escha_and_Logy_EN.exe"))
        .and_then(|exe| read(&exe).ok())
        .and_then(|data| barrel_formats::pe::Pe::parse(&data).ok().and_then(|pe| barrel_escha::data::bgm::tracks(&pe, barrel_escha::data::bgm::TRACKS_EN)))
        .unwrap_or_default();
    // BARREL_GAME=sophie2: an Atelier Sophie 2 event, carried over to Escha's commands by name;
    // BARREL_GAME=ryza: an Atelier Ryza DX one, the same way.
    let sophie2 = std::env::var("BARREL_GAME").is_ok_and(|g| g == "sophie2");
    let ryza = std::env::var("BARREL_GAME").is_ok_and(|g| g == "ryza");
    let table = if sophie2 { OperandTable::sophie2() } else if ryza { OperandTable::ryza() } else { OperandTable::escha() };
    // A Shallie event (its install beside it): read with Shallie's table, its commands put into
    // Escha's words where they mean the same (`ebd::shallie_to_escha`).
    let shallie = file.ancestors().any(|p| p.join("Atelier_Shallie.exe").is_file());
    let (event, table) = if shallie { event.translate(&OperandTable::shallie(), barrel_formats::ebd::shallie_to_escha)? } else { (event, table) };
    let player = if sophie2 {
        Player::translated(&event, &messages, &table, barrel_formats::ebd_names::SOPHIE2, &[])?
    } else if ryza {
        Player::translated(&event, &messages, &table, barrel_formats::ebd_names::RYZA, barrel_formats::ebd_names::RYZA_SAME_AS_ESCHA)?
    } else {
        Player::new(&event, &messages, &table)?
    };
    let steps: Vec<(Option<f32>, Step)> = if timed {
        // BARREL_READ: hold each line its unattended reading time, as the viewer does.
        let player = if std::env::var("BARREL_READ").is_ok() { player.reading_time(barrel_engine::event::reading_time) } else { player };
        player.schedule().into_iter().map(|t| (Some(t.time), t.step)).collect()
    } else {
        player.map(|s| (None, s)).collect()
    };
    for (time, step) in steps {
        if let Some(t) = time {
            print!("{t:8.2} ");
        }
        match step {
            Step::StartAction { action } => println!("{action:3} --- action {action} starts"),
            Step::Message { index, speaker, text, .. } => {
                let who = usize::try_from(speaker).ok().and_then(|i| names.get(i)).map_or("", String::as_str);
                println!("  [{index:3}] {who:>10}: {}", text.replace("<CR>", " "));
            }
            Step::Wait { seconds } => println!("      (wait {seconds:.2} s)"),
            Step::EventFlag { flag, value } => println!("      (event flag {flag} = {value})"),
            Step::EventFlagAdd { flag, amount } => println!("      (event flag {flag} += {amount})"),
            Step::EndGroup { group } => println!("      (the end runs process group {group})"),
Step::Planboard { first } => println!("      (the planning board opens from {first})"),
                        Step::GameMode { mode } => println!("      (then game mode {})", barrel_engine::event::GAME_MODES.get(mode as usize).unwrap_or(&"?")),
            Step::Trophy { trophy } => println!("      (achievement {trophy}: left out)"),
            Step::Movie { movie } => println!("      movie {movie} (not played yet)"),
            Step::MarkEvent { event, seen } => println!("      (event {event} marked {})", if seen { "seen" } else { "unseen" }),
            Step::CharacterWait { action, character, seconds } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("{action:3} {who} waits {seconds:.2} s");
            }
            Step::Place { action, character, position } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("{action:3} {who} at ({}, {}, {})", position[0], position[1], position[2]);
            }
            Step::PlaceNear { action, character, of, offset } => {
                let who = |n: u32| names.get(n as usize).map_or("?", String::as_str).to_string();
                println!("{action:3} {} at ({}, {}, {}) from {}", who(character), offset[0], offset[1], offset[2], who(of));
            }
            Step::FieldChange { action, map } => {
                let name = fields.get(map as usize).map_or("?", String::as_str);
                println!("{action:3} --- field {map} ({name})");
            }
            Step::Collision { action, character, solid, flag } => {
                println!("{action:3} {} {} (flag {flag})", names.get(character as usize).map_or("?", String::as_str), if solid { "collides" } else { "passes through" })
            }
            Step::RemoveAll { action } => println!("{action:3} --- everyone leaves"),
            Step::Quake { action, quake, seconds } => println!("{action:3} --- camera quake {quake} for {seconds} s"),
            Step::EventObjects { action, shown } => println!("{action:3} --- event objects (xEV_) {}", if shown { "shown" } else { "hidden" }),
            Step::QuakeStop { action } => println!("{action:3} --- camera quake stops"),
            Step::CameraRoll { action, from, to, seconds, curve } => println!("{action:3} --- camera rolls {from}° -> {to}° over {seconds} s (curve {curve})"),
            Step::CameraDistance { action, length, seconds, curve } => println!("{action:3} --- eye slides to {length} from the target over {seconds} s (curve {curve})"),
            Step::MapVisible { action, version, shown } => println!("{action:3} --- field objects of version {version} {}", if shown { "shown" } else { "hidden" }),
            Step::Remove { action, character } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("{action:3} {who} leaves");
            }
            Step::Music { action, track, volume, fade } => match track {
                Some(t) => {
                    let name = tracks.get(t as usize).map_or("?".to_string(), |k| format!("{} ({})", k.file, k.title));
                    println!("{action:3} --- music {t}: {name} at {:.0}%", volume * 100.0);
                }
                None => println!("{action:3} --- music stops over {fade} s"),
            },
            Step::Effect { action, character, effect, volume, pan } => {
                let who = character.map_or("", |c| names.get(c as usize).map_or("?", String::as_str));
                let what = match effect {
                    0..=127 => format!("bank sound {effect}"),
                    128..=151 => format!("ambience {}", effect - 128),
                    _ => "nothing (past the player's 152)".to_string(),
                };
                println!("{action:3} {who} effect {effect}: {what} at {:.0}%, pan {pan:+.2}", volume * 100.0);
            }
            Step::Voice { action, character, clip } => {
                let who = character.map_or("", |c| names.get(c as usize).map_or("?", String::as_str));
                println!("{action:3} {who} voice {clip} (bank Series{:04}, clip {})", clip / 1000 * 1000, clip % 1000);
            }
            Step::MotionDivision { action, character, division } => {
                println!("{action:3} {} moves to division {division} of its motion", names.get(character as usize).map_or("?", String::as_str))
            }
            Step::Motion { action, character, motion, blend, .. } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                let name = motions.get(motion as usize).map_or_else(|| motion.to_string(), Clone::clone);
                println!("{action:3} {who} plays {name} (blend {blend:.2} s)");
            }
            Step::WaitForCharacter { character } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("      (wait for {who})");
            }
            Step::HeadGesture { action, character, segment, rate, weight } => println!("{action:3} {} head gesture {segment} at {rate}x, weight {weight}", names.get(character as usize).map_or("?", String::as_str)),
            Step::HeadStop { action, character } => println!("{action:3} {} head gesture ends", names.get(character as usize).map_or("?", String::as_str)),
            Step::Still { action, picture, seconds } => println!("{action:3} --- still picture {picture} (fading over {seconds} s)"),
            Step::Telop { action, card } => println!("{action:3} --- narration card {card}"),
            Step::TelopAlpha { action, from, to, seconds } => println!("{action:3} --- narration card opacity {from} -> {to} over {seconds} s"),
            Step::TelopPart { action, part, shown, seconds } => println!("{action:3} --- narration card piece {part} {} over {seconds} s", if shown { "shown" } else { "hidden" }),
            Step::Fade { action, preset, seconds } => println!("{action:3} --- fade (preset {preset}) over {seconds} s"),
            Step::Light { action, change } => println!("{action:3} --- light {change:?}"),
            Step::ScreenEffect { action, kind, param, seconds } => {
                const KINDS: [&str; 17] = [
                    "fade", "blur", "old sepia", "cross-fade", "battle start", "negative", "glow", "refraction", "?", "hue", "gamma", "colour",
                    "blur", "sepia", "monotone", "texture filter", "wave",
                ];
                let name = KINDS.get(kind as usize).copied().unwrap_or("?");
                println!("{action:3} --- screen effect {kind} ({name}), {param}, over {seconds} s");
            }
            Step::FieldCamera { action, change } => {
                use barrel_engine::field_camera::Change as C;
                let rel = |r: bool| if r { "by" } else { "to" };
                let what = match change {
                    C::All { yaw, fov, pitch, distance, target, offset } => format!("yaw {yaw}°, pitch {pitch}°, field of view {fov}°, {distance} away from {target:?} + {offset:?}"),
                    C::Yaw { value, relative, seconds } => format!("yaw {} {value}° over {seconds} s", rel(relative)),
                    C::Fov { value, relative, seconds } => format!("field of view {} {value}° over {seconds} s", rel(relative)),
                    C::Pitch { value, relative, seconds } => format!("pitch {} {value}° over {seconds} s", rel(relative)),
                    C::Distance { value, relative, seconds } => format!("distance {} {value} over {seconds} s", rel(relative)),
                    C::Target { value, relative, seconds } => format!("target {} {value:?} over {seconds} s", rel(relative)),
                    C::DataEffect { on } => format!("the field's camera data {}", if on { "on (back to play)" } else { "off" }),
                    C::Follow { character } => match character {
                        Some(n) => format!("follows {}", names.get(n as usize).map_or("?", String::as_str)),
                        None => "follows no one".to_string(),
                    },
                };
                println!("{action:3} --- field camera: {what}");
            }
            Step::Camera { action, part, change } => {
                use barrel_engine::camera::Change;
                let what = match change {
                    Change::Set(p) => format!("at {}", describe_position(&p)),
                    Change::Move(m) => describe_move(&m),
                };
                println!("{action:3} camera {part:?} {what}");
            }
            Step::Turn { action, character, rotation } => {
                let who = names.get(character as usize).map_or_else(|| character.to_string(), Clone::clone);
                println!("{action:3} {who} turns to {:.0}°", rotation[1].to_degrees());
            }
            Step::TurnOver { action, character, rotation, seconds, curve } => {
                let who = names.get(character as usize).map_or_else(|| character.to_string(), Clone::clone);
                println!("{action:3} {who} turns to {:.0}° in {seconds:.2} s (curve {curve})", rotation[1].to_degrees());
            }
            Step::Face { action, character, toward } => {
                let who = names.get(character as usize).map_or_else(|| character.to_string(), Clone::clone);
                println!("{action:3} {who} faces {}", describe_position(&toward));
            }
            Step::CopyFacing { action, character, from } => {
                let who = names.get(character as usize).map_or_else(|| character.to_string(), Clone::clone);
                println!("{action:3} {who} takes the facing of {}", describe_position(&from));
            }
            Step::CharacterMove { action, character, extra, movement } => {
                let who = names.get(character as usize).map_or_else(|| character.to_string(), Clone::clone);
                println!("{action:3} {who} {} (extra {extra})", describe_move(&movement));
            }
            Step::Command { action, word, name, operands } => {
                let ops: Vec<String> = operands.iter().map(|&o| (o as i32).to_string()).collect();
                let short = Word::classify(word).to_string();
                println!("{action:3} {short:<10} {:<40} {}", name.unwrap_or(""), ops.join(" "));
            }
        }
    }
    Ok(())
}

/// Message gestures (`barrel_escha::gesture`): each entry's two forms as motion names.
fn gestures(exe: &Path, numbers: &[&str]) -> Result<()> {
    let data = read(exe)?;
    let table = barrel_escha::gesture::read(&data).ok_or("gesture table not found")?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{e}"))?;
    let names = barrel_escha::gesture::motions(&pe);
    let name = |m: u32| names.get(m as usize).cloned().unwrap_or_else(|| m.to_string());
    let picked: Vec<usize> = if numbers.is_empty() { (0..table.len()).collect() } else { numbers.iter().filter_map(|n| n.parse().ok()).collect() };
    for n in picked {
        let Some(entry) = table.get(n) else { continue };
        for (k, form) in entry.iter().enumerate() {
            let Some(f) = form else { continue };
            let steps: Vec<String> = f.steps.iter().map(|s| match s {
                barrel_escha::gesture::Step::Motion(m) => name(*m),
                barrel_escha::gesture::Step::Repeat(r) => format!("x{r}"),
            }).collect();
            println!("{n:3}.{k}: {} {} end {}", name(f.first), steps.join(" "), f.end);
        }
    }
    Ok(())
}

/// The delivery table (`barrel_escha::delivery`): each assignment card's items and bonuses.
fn deliveries(install: &Path) -> Result<()> {
    let path = install.join("Saves/delivery/DeliveryData.xml");
    let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let all = barrel_escha::delivery::deliveries(&table);
    for d in all.iter().filter(|d| !d.tag.starts_with("DELIVERY_TEST")) {
        let items: Vec<String> = d.items.iter().map(|(t, s)| format!("{t} {s}")).collect();
        let mut extra = Vec::new();
        if let Some(c) = &d.category { extra.push(c.clone()); }
        if let Some(b) = &d.effect { extra.push(format!("effect {} {}", b.value, b.score)); }
        if let Some(b) = &d.level { extra.push(format!("level>={} {}", b.value, b.score)); }
        if let Some(b) = &d.quality { extra.push(format!("quality>={} {}", b.value, b.score)); }
        if let Some(e) = d.elements { extra.push(format!("elements {e:?}")); }
        println!("{:<24} pass {:>4}  {}  {}", d.tag, d.pass_score, items.join(", "), extra.join("; "));
    }
    println!("{} deliveries ({} tests not shown)", all.len(), all.iter().filter(|d| d.tag.starts_with("DELIVERY_TEST")).count());
    Ok(())
}

/// The camera quakes (`barrel_formats::quake`): each one's six tracks, key by key.
fn quakes(install: &Path) -> Result<()> {
    let path = install.join("Saves/cameraQuake/camera_quake_data.xml");
    let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut all = barrel_formats::quake::read(&table);
    all.sort_by_key(|q| q.number());
    for q in &all {
        let tracks: Vec<String> = q.tracks.iter().map(|t| {
            let keys: Vec<String> = t.keys.iter().map(|k| format!("{}@{}", k.value, k.frame)).collect();
            format!("[{}{}]", keys.join(" "), if t.looping { " loop" } else { "" })
        }).collect();
        println!("{:<11} {}", q.id, tracks.join(" "));
    }
    println!("{} quakes", all.len());
    Ok(())
}

/// The per-act table (`battle_data::acts`): each act's name, set flags and bonus limit.
fn acts(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (names, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
    for (k, a) in barrel_escha::data::battle_data::acts(&pe, barrel_escha::data::battle_data::ACTS_EN).iter().enumerate() {
        let flags: Vec<String> = a.flags.iter().enumerate().filter(|(_, f)| **f).map(|(i, _)| i.to_string()).collect();
        println!("{k:3} {:32} flags {:20} limit {} last {}", names.get(k).map_or("?".to_string(), |n| n.to_string()), flags.join(","), a.limit, a.last);
    }
    Ok(())
}

/// Reads every command macro in ROOT/Script/macro_*.nut, resolves it with placeholder
/// arguments and decodes it as an event action, listing its commands or what it names that is
/// not known.
/// Runs a creature script for TICKS frames (1/60 s) on a `Creature` 600 units from the player,
/// printing what changes.
fn nut_run(root: &Path, file: &str, ticks: usize) -> Result<()> {
    let exe = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("exe: {e}"))?;
    let motions = barrel_escha::gesture::motions(&pe);
    let names = barrel_escha::macros::Names::escha(&motions);
    let load = |name: &str| -> Result<Vec<barrel_engine::squirrel::Stmt>> {
        let bytes = read(&root.join("Script").join(name))?;
        let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
        barrel_engine::squirrel::parse(&text).map_err(|e| format!("{name}: {e}").into())
    };
    let mut c = barrel_escha::creature::Creature::new([0.0; 3], barrel_engine::rng::Rng::from_state([1, 2, 3, 4]));
    c.player = [600.0, 0.0, 0.0];
    c.constants = names.all().map(|(n, v)| (n.to_string(), v as i32 as i64)).collect();
    let mut vm = barrel_engine::squirrel::Interp::new();
    vm.run(&load("utility.nut")?, &mut c).map_err(|e| format!("utility.nut: {e}"))?;
    let script = load(file)?;
    let mut last = String::new();
    for tick in 0..ticks {
        vm.run(&script, &mut c).map_err(|e| format!("{file}, frame {tick}: {e}"))?;
        c.frame(1.0 / 60.0);
        let motion = c.motion.and_then(|m| motions.get(m as usize)).map_or("-", |m| m.as_str());
        let now = format!("state {} motion {motion} emotion {:?} moving {}", c.state, c.emotion, c.moving.is_some());
        if now != last {
            println!("{:6.2} s: {now} at ({:.0}, {:.0})", tick as f64 / 60.0, c.position[0], c.position[2]);
            last = now;
        }
    }
    Ok(())
}

/// Runs field area AREA's enemies (by the field enemy table) under their scripts for FRAMES
/// frames while a stand-in player walks west through them from 15 m east of the first,
/// printing each creature's changes.
fn field_sim(root: &Path, area: &str, frames: usize) -> Result<()> {
    let exe = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("exe: {e}"))?;
    let motions = barrel_escha::gesture::motions(&pe);
    let script = |name: &str| -> std::result::Result<Vec<barrel_engine::squirrel::Stmt>, String> {
        let bytes = read(&root.join("Script").join(format!("{name}.nut"))).map_err(|e| format!("{name}: {e}"))?;
        let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
        barrel_engine::squirrel::parse(&text).map_err(|e| format!("{name}: {e}"))
    };
    let area: i32 = area.parse().map_err(|_| "the area is a field map number (12 is AREA_01_01)")?;
    let xml = |name: &str| read(&root.join("Saves/chara").join(name)).ok().and_then(|b| barrel_formats::table::parse(&b).ok());
    let (chara, base) = (xml("Chara.xml"), xml("CharaBase.xml"));
    let (mut field, enemies) = barrel_escha::field_ai::build(&pe, area, script, chara.as_ref(), base.as_ref())?;
    let player_radius = field.player_radius;
    println!("radii: player {player_radius}, enemies {:?}", field.radii);
    let start = field.mobs[0].creature.position;
    let mut player = [start[0] + 1500.0, start[1], start[2]];
    let facing = -std::f64::consts::FRAC_PI_2;
    let mut last: Vec<String> = vec![String::new(); field.mobs.len()];
    for f in 0..frames {
        player[0] -= 200.0 / 60.0;
        if let Some(t) = field.step(player, facing, 1.0 / 60.0).map_err(|e| format!("frame {f}: {e}"))? {
            println!("{:6.2} s  #{} touches the player at ({:.0}, {:.0}): encounter group {}", f as f64 / 60.0, t.enemy, player[0], player[2], t.group);
            break;
        }
        for (k, m) in field.mobs.iter().enumerate() {
            let c = &m.creature;
            let motion = c.motion.and_then(|v| motions.get(v as usize)).map_or("-", |v| v.as_str());
            let now = format!("{} state {} motion {motion}", if c.aware { "aware" } else { "idle " }, c.state);
            if now != last[k] {
                println!("{:6.2} s  #{k} {:<12} at ({:6.0}, {:6.0}), player at ({:6.0}, {:6.0}): {now}", f as f64 / 60.0, enemies[k].ai.1, c.position[0], c.position[2], player[0], player[2]);
                last[k] = now;
            }
        }
    }
    Ok(())
}

/// The assignments (`Saves/subject/SubjectData.xml`, English names from `Saves_EN`), with a game
/// save's `subject_save` state of each.
/// Follows the story from event START (an `EVENT_*` name) in a new game: each event is played
/// (seen, its process data applied), then its end fires (group 11) and the event that starts is
/// next. When none does, the player takes the next step of STEPS: a walk (`K.M>K.M`: from kind
/// and map to kind and map, kind 0 a field, 1 the world map) or days passing (`sleep:N`,
/// `mix:N`, `travel:N`) or a world map trip (`trip:FROM>TO`, between marks: `T0`, `A0p0`
/// ..., its days on the road, then into the field TO opens), and the story goes on from what
/// that starts.
fn event_chain(root: &Path, start: &str, walks: &[&str]) -> Result<()> {
    let mut chain = story_chain(root)?;
    let place = |s: &str| -> Result<[i32; 2]> {
        let (k, m) = s.split_once('.').ok_or("a place is KIND.MAP")?;
        Ok([k.parse()?, m.parse()?])
    };
    let mut walks = walks.iter();
    let mut event = Some(start.to_string());
    for _ in 0..200 {
        if let Some(e) = event.take() {
            println!("{e}");
            event = chain.after(&e).map(str::to_string);
            continue;
        }
        let Some(w) = walks.next() else { break };
        use barrel_escha::event_start::Days;
        event = match w.split_once(':') {
            // trip:FROM>TO: a world map trip between two marks, into the field TO opens first.
            Some(("trip", ends)) => {
                let (from, to) = ends.split_once('>').ok_or("trip:FROM>TO")?;
                let exe = root.join("Atelier_Escha_and_Logy_EN.exe");
                let map = world_map(&exe)?;
                let bytes = read(&exe)?;
                let pe = barrel_formats::pe::Pe::parse(&bytes)?;
                let names = barrel_escha::data::world::point_names(&pe).unwrap_or_default();
                let field = barrel_escha::data::world::point_maps(&pe)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|(p, _, _)| names.get(*p as usize).and_then(|n| barrel_escha::data::world::point_tag(n)).as_deref() == Some(to))
                    .map(|(_, m, _)| m as i32)
                    .ok_or(format!("{to} opens no field"))?;
                let (started, days) = chain.travel(&map, from, to, field);
                println!("  {w}: {} days", days.ok_or(format!("no way from {from} to {to}"))?);
                started
            }
            Some((what, n)) => {
                let what = match what {
                    "sleep" => Days::Sleep,
                    "mix" => Days::Mix,
                    "travel" => Days::Travel,
                    _ => return Err(format!("{w}: sleep:N, mix:N or travel:N").into()),
                };
                chain.days(what, n.parse()?)
            }
            None => {
                let (from, to) = w.split_once('>').ok_or("a step is FROM>TO, sleep:N, mix:N or travel:N")?;
                chain.walk(place(from)?, place(to)?)
            }
        }
        .map(str::to_string);
        println!("  {w}: {}", event.as_deref().unwrap_or("nothing starts"));
    }
    Ok(())
}

/// Which events the save would start when trigger group GROUP fires with VALUES (in the town or
/// a field nobody is in; the commands not modelled yet count as failing and are tallied).
fn event_starts(root: &Path, save: &Path, group: &str, values: &[&str]) -> Result<()> {
    use barrel_escha::event_start::{self as es, World as _};
    let group: u8 = group.parse().map_err(|_| "GROUP is a number")?;
    let fired: Vec<i32> = values.iter().map(|v| v.parse()).collect::<std::result::Result<_, _>>().map_err(|_| "VALUES are numbers")?;
    let checks = barrel_escha::data::event_check::parse(&read(&root.join("Event/eventCheck/EventCheckDataAll.ecd"))?).map_err(|e| e.to_string())?;
    // `new`: a new game's state (nothing seen, no flags, April 1st of year 1: day 90).
    let sum = if save == Path::new("new") {
        // BARREL_SEEN=a,b,...: events taken as seen.
        let mut events = vec![barrel_escha::data::save::EventRecord { seen: false, day: -31, count: 0, roll: 0, value: 0 }; 1165];
        for e in std::env::var("BARREL_SEEN").unwrap_or_default().split(',').filter_map(|v| v.trim().parse::<usize>().ok()) {
            if let Some(r) = events.get_mut(e) {
                r.seen = true;
                r.day = 90;
            }
        }
        barrel_escha::data::save::Summary { day: Some(90), events, ..Default::default() }
    } else {
        let data = read(save)?;
        let body = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", save.display()))?.body;
        let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", save.display()))?;
        barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| e.to_string())?)
    };
    let exe_data = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe_data).map_err(|e| e.to_string())?;
    let (names, _) = pe.enum_names("EVENT_MM01_OP1_111").ok_or("no EVENT_* enum")?;
    struct W<'a>(&'a barrel_escha::data::save::Summary);
    impl es::World for W<'_> {
        fn seen(&self, e: i32) -> bool {
            usize::try_from(e).ok().and_then(|e| self.0.events.get(e)).is_some_and(|r| r.seen)
        }
        fn seen_day(&self, e: i32) -> i32 {
            usize::try_from(e).ok().and_then(|e| self.0.events.get(e)).map_or(0, |r| r.day)
        }
        fn today(&self) -> i32 {
            self.0.day.unwrap_or(0) as i32
        }
        fn flag(&self, f: i32) -> i32 {
            usize::try_from(f).ok().and_then(|f| self.0.event_flags.get(f)).copied().unwrap_or(0)
        }
        fn region_zero(&self, map: i32) -> bool {
            usize::try_from(map).is_ok_and(|m| m < barrel_escha::data::gimmick::TOWN_MAPS)
        }
        fn field_map(&self) -> Option<i32> {
            None
        }
        fn money(&self) -> i32 {
            self.0.money.unwrap_or(0)
        }
        fn field_area(&self) -> Option<i32> {
            None
        }
        fn level(&self, c: i32) -> Option<i32> {
            (0..=10).contains(&c).then(|| self.0.party.iter().find(|m| m.character == c)).flatten().map(|m| m.level)
        }
        // The save's squad is its slot list.
        #[allow(clippy::misnamed_getters)]
        fn squad(&self) -> &[i32] {
            &self.0.squad_slots
        }
        fn alchemy_level(&self) -> i32 {
            self.0.alchemy.map_or(0, |a| a.0)
        }
        fn friendship(&self, c: i32) -> Option<Option<f32>> {
            // 0x140330120: characters 0-10 are the records' own indices, any other has none.
            Some((0..=10).contains(&c).then(|| self.0.party.iter().find(|m| m.character == c)).flatten().map(|m| m.friendship))
        }
        fn item_count(&self, item: Option<i32>, low: f32, high: f32) -> Option<i32> {
            let held = self.0.basket.iter().chain(&self.0.container);
            let hit = |u: &&barrel_escha::data::save::ItemUnit| item.is_none_or(|i| i32::from(u.item) == i) && (low..=high).contains(&u.quality);
            Some(held.filter(hit).map(|u| i32::from(u.counts[3])).sum())
        }
    }
    let w = W(&sum);
    let mut unread: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut passes = |event: usize, g: usize| -> bool {
        let check = &checks[event];
        let gate = es::Gate { day_flag: false, seen: w.seen(event as i32), flag_1: w.flag(1), today: w.today() };
        if !es::gate_open(&check.words, &gate) {
            return false;
        }
        let Some(words) = check.groups.get(g) else { return false };
        let Ok(cmds) = barrel_escha::data::event_check::commands(words) else { return false };
        cmds.iter().all(|c| match es::command(c, &es::Fired { group, values: &fired, day: w.today(), event: event as i32 }, &w) {
            Some(ok) => ok,
            None => {
                *unread.entry(c.name()).or_default() += 1;
                false
            }
        })
    };
    let lists = es::listings(&checks);
    let list = lists.get(&group).map_or(&[][..], |l| l.as_slice());
    println!("day {}, {} events listed under group {group}", w.today(), list.len());
    for l in list {
        if passes(l.event, 0) && passes(l.event, l.group) {
            println!("  passes: {} (priority {}, group {})", names.get(l.event).unwrap_or(&"?"), l.priority, l.group);
        }
    }
    for (n, k) in unread {
        println!("  not modelled: {n} ({k}×)");
    }
    Ok(())
}

fn subjects(root: &Path, save: Option<&Path>) -> Result<()> {
    use barrel_escha::data::subject as sb;
    let rows = sb::subjects(&barrel_formats::table::parse(&read(&root.join("Saves/subject/SubjectData.xml"))?)?);
    let en = read(&root.join("Saves_EN/subject/SubjectData.xml")).ok().and_then(|b| barrel_formats::table::parse(&b).ok()).map(|t| sb::subjects(&t)).unwrap_or_default();
    let state = match save {
        Some(p) => {
            let data = read(p)?;
            let s = barrel_escha::data::save::parse(&data).map_err(|e| format!("{}: {e}", p.display()))?;
            let st = barrel_escha::data::save::decode(s.body).map_err(|e| e.to_string())?;
            let recs = barrel_escha::data::save::state_records(&st).map_err(|e| e.to_string())?;
            fn find<'a>(rs: &'a [barrel_escha::data::save::Record<'a>], name: &str) -> Option<&'a [u8]> {
                use barrel_escha::data::save::Body;
                rs.iter().find_map(|r| match &r.body {
                    Body::Raw(d) | Body::Data(d) if r.name == name => Some(*d),
                    Body::Group(g) => find(g, name),
                    _ => None,
                })
            }
            find(&recs, "subject_save").map(sb::saved).unwrap_or_default()
        }
        None => Vec::new(),
    };
    for (k, r) in rows.iter().enumerate() {
        let name = en.get(k).map_or(r.name.as_str(), |e| e.name.as_str());
        let st = state.get(k).map_or(String::new(), |s| format!("  count {}{}", s.progress, if s.cleared() { ", cleared" } else { "" }));
        println!("{k:3} {:<8} {:<3} {:>3} {:<34} {} {}{st}", r.card.trim_start_matches("CARD_"), r.square, r.score, name, r.condition.trim_start_matches("SUBJECT_WORK_COND_"), r.params.join(","));
    }
    Ok(())
}

/// The costume tables (`Saves/costume`): each outfit's model and the accessories adjusted to it.
fn costumes(root: &Path, sysdata: Option<&Path>) -> Result<()> {
    use barrel_escha::data::costume as c;
    let table = |n: &str| -> Result<barrel_formats::table::Element> { Ok(barrel_formats::table::parse(&read(&root.join("Saves/costume").join(n))?)?) };
    let names = |n: &str| -> Option<barrel_formats::table::Element> { barrel_formats::table::parse(&read(&root.join("Saves_EN/costume").join(n)).ok()?).ok() };
    let bodies = c::bodies(&table("CostumeBody.xml")?);
    let en = names("CostumeBody.xml").map(|t| c::bodies(&t)).unwrap_or_default();
    let heads = c::accessories(&table("CostumeHead.xml")?);
    let faces = c::accessories(&table("CostumeFace.xml")?);
    let adjusts = c::adjusts(&table("AccessoryAdjust.xml")?);
    let hides = c::hides(&table("PartsInvisible.xml")?);
    for (k, b) in bodies.iter().enumerate() {
        let name = en.get(k).map_or(b.name.as_str(), |e| e.name.as_str());
        println!("{k:2} {:<22} {:<10} model {:<7} head at {} face at {}", name, b.chara.trim_start_matches("COSTUME_CHARA_"), b.base, b.head_attach, b.face_attach);
    }
    println!("{} heads ({} with a model), {} faces, {} adjustments, {} hiding rules", heads.len(), heads.iter().filter(|h| !h.file.is_empty()).count(), faces.len(), adjusts.len(), hides.len());
    if let Some(path) = sysdata {
        let state = barrel_escha::data::save::decode(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        let records = barrel_escha::data::save::state_records(&state).map_err(|e| e.to_string())?;
        fn find<'a>(rs: &'a [barrel_escha::data::save::Record<'a>], name: &str) -> Option<&'a [u8]> {
            use barrel_escha::data::save::Body;
            rs.iter().find_map(|r| match &r.body {
                Body::Raw(d) | Body::Data(d) if r.name == name => Some(*d),
                Body::Group(g) => find(g, name),
                _ => None,
            })
        }
        let head_en = names("CostumeHead.xml").map(|t| c::accessories(&t)).unwrap_or_default();
        for (k, [b, h, f]) in find(&records, "costumeData").map(c::choices).unwrap_or_default().into_iter().enumerate() {
            let body = en.get(b as usize).or(bodies.get(b as usize)).map_or("?", |x| x.name.as_str());
            let head = head_en.get(h as usize).or(heads.get(h as usize)).map_or("?", |x| x.name.as_str());
            println!("character {k}: body {b} {body}, head {h} {head}, face {f}");
        }
    }
    Ok(())
}

/// Parses every Squirrel script under ROOT/Script with Barrel's own front end.
fn nut_parse(root: &Path) -> Result<()> {
    let mut paths: Vec<_> = std::fs::read_dir(root.join("Script"))?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "nut")).collect();
    paths.sort();
    let mut bad = 0;
    for path in &paths {
        let bytes = read(path)?;
        let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
        match barrel_engine::squirrel::parse(&text) {
            Ok(s) => println!("{:<24} {} statements", path.file_name().unwrap().to_string_lossy(), s.len()),
            Err(e) => {
                bad += 1;
                println!("{:<24} {e}", path.file_name().unwrap().to_string_lossy());
            }
        }
    }
    println!("{} of {} parse", paths.len() - bad, paths.len());
    Ok(())
}

fn macros(root: &Path) -> Result<()> {
    let exe = read(&root.join("Atelier_Escha_and_Logy_EN.exe"))?;
    let pe = barrel_formats::pe::Pe::parse(&exe).map_err(|e| format!("exe: {e}"))?;
    let names = barrel_escha::macros::Names::escha(&barrel_escha::gesture::motions(&pe));
    let table = barrel_formats::ebd::OperandTable::escha();
    let mut paths: Vec<_> = std::fs::read_dir(root.join("Script"))?.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("macro_"))).collect();
    paths.sort();
    for path in paths {
        let bytes = read(&path)?;
        let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
        for m in barrel_escha::macros::read(&text) {
            let args: Vec<u32> = (0..m.params.len() as u32).map(|k| 10000 + k).collect();
            match m.words(&names, &args) {
                Ok(words) => match barrel_formats::ebd::decode(&words, &table) {
                    Ok(commands) => println!("{:<22} {:<34} {} words, {} commands", path.file_name().unwrap().to_string_lossy(), m.name, words.len(), commands.len()),
                    Err(e) => {
                        // The command words the table does not know (event files never use them).
                        let missing: Vec<String> = m
                            .words
                            .iter()
                            .filter_map(|t| match t {
                                barrel_escha::macros::Token::Name(n) if n.starts_with("ECT_") => Some(n.clone()),
                                _ => None,
                            })
                            .filter(|n| names_word(n).is_some_and(|w| table.get(w).is_none()))
                            .collect();
                        println!("{:<22} {:<34} decode: {e:?} {missing:?}", path.file_name().unwrap().to_string_lossy(), m.name)
                    }
                },
                Err(u) => println!("{:<22} {:<34} unknown {}", path.file_name().unwrap().to_string_lossy(), m.name, u.0),
            }
        }
    }
    Ok(())
}

fn names_word(name: &str) -> Option<u32> {
    barrel_escha::macros::Names::escha(&[]).value(name)
}

fn trait_kinds(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    for (k, n) in barrel_escha::data::battle_data::trait_kinds(&pe).iter().enumerate() {
        println!("{k:3} {n}");
    }
    Ok(())
}

/// A battle with a save's squad (its first three: levels, equipped stats, HP and MP now, skills)
/// and the alchemists' battle items, against a monster.
fn battle_from_save(exe: &Path, monster: &str, file: &Path) -> Result<()> {
    use barrel_escha::data::battle_data as bd;
    use barrel_escha::fight::Event;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table")?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table")?;
    let lists = bd::party_skills(&pe, bd::PARTY_SKILLS_EN).ok_or("no party skill lists")?;
    let growth = bd::party_growth(&pe, bd::PARTY_GROWTH_EN).ok_or("no party growth table")?;
    let install = exe.parent().ok_or("no install folder")?;
    let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
    let item_rows = bd::item_effects(&barrel_formats::table::parse(&read(&install.join("Saves_EN/item/item_effect.xml"))?).map_err(|e| e.to_string())?, &acts);
    let tags: Vec<&str> = item_rows.iter().map(|r| r.tag.as_str()).collect();
    let traits = bd::traits(&barrel_formats::table::parse(&read(&install.join("Saves_EN/item/item_potential.xml"))?).map_err(|e| e.to_string())?, &tags);
    let item_names: Vec<String> = barrel_formats::table::parse(&read(&install.join("Saves_EN/item/itemData.xml"))?)
        .map_err(|e| e.to_string())?
        .children
        .iter()
        .map(|r| r.attr("ItemName").unwrap_or("?").to_string())
        .collect();
    let table = bd::item_battles(&pe, bd::ITEM_BATTLE_EN, bd::ITEM_BATTLE_END_EN);
    let save_data = read(file)?;
    let body = barrel_escha::data::save::parse(&save_data).map_err(|e| format!("{}: {e}", file.display()))?.body;
    let state = barrel_escha::data::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
    let s = barrel_escha::data::save::summary(&barrel_escha::data::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?);
    let member_names = ["Escha", "Logy", "Awin", "Threia", "Reyfer", "Lucille", "Linca", "Wilbell", "Micie", "Katla", "Nio"];
    let mut names = Vec::new();
    let mut members = Vec::new();
    for &c in s.squad.iter().take(3) {
        let saved = s.party.iter().find(|m| m.character == c).ok_or_else(|| format!("character {c}: no party record"))?;
        let c = c as usize;
        let stats = growth.get(c).and_then(|l| l.get((saved.level as usize).wrapping_sub(1))).ok_or_else(|| format!("character {c}: no growth row"))?;
        let mut block = barrel_escha::duel::party_block(saved.level, stats);
        let [max_hp, max_mp, attack, defense, speed] = saved.equipped;
        (block.hp, block.attack, block.defense, block.speed) = (max_hp, attack, defense, speed);
        let mut member = barrel_escha::skirmish::Member::new(block, max_mp, &lists[c], &skills).ok_or("no party attack")?;
        member.size = barrel_escha::data::battle_data::party_size(&chara_radii_near(exe), c).unwrap_or(0.0);
        member.now = Some((saved.hp, saved.mp));
        for u in s.battle_items.get(c).into_iter().flatten() {
            let Some(row) = table.iter().find(|b| b.item == u.item as i32) else { continue };
            let name = item_names.get(u.item as usize).cloned().unwrap_or_default();
            let action = barrel_escha::battle::item_action(&name, row.target, row.area, row.wait, &u.effects, &item_rows);
            let held_traits = u.traits.iter().filter_map(|&t| usize::try_from(t).ok().and_then(|t| traits.get(t)).cloned()).collect();
            member.items.push(barrel_escha::skirmish::Held { action, power: barrel_escha::battle::item_power(u.quality), count: u.counts[0].max(0) as u32, traits: held_traits });
        }
        let items: Vec<String> = member.items.iter().map(|h| format!("{} ×{}", h.action.name, h.count)).collect();
        println!("{} level {}: HP {}/{}, MP {}/{}, attack {} defense {} speed {}{}", member_names.get(c).unwrap_or(&"?"), saved.level, saved.hp, max_hp, saved.mp, max_mp, attack, defense, speed, if items.is_empty() { String::new() } else { format!(", items {}", items.join(", ")) });
        names.push(format!("{} ({})", member_names.get(c).unwrap_or(&"?"), saved.level));
        members.push(member);
    }
    let mut rng = barrel_engine::rng::Rng::seeded(1);
    // MONSTER: a monster number, or gN for encounter group N drawn as the game does.
    let drawn: Vec<usize> = match monster.strip_prefix('g') {
        Some(g) => {
            let g: usize = g.parse().map_err(|_| format!("{monster}: group number"))?;
            let group = bd::encounter_groups(&pe, bd::ENCOUNTER_GROUPS_EN).into_iter().nth(g).ok_or("no such group")?;
            barrel_escha::battle::encounter_monsters(&group, &mut rng).into_iter().map(|m| m as usize).collect()
        }
        None => vec![monster.parse::<usize>().map_err(|_| format!("{monster}: monster number"))?],
    };
    let mut foes = Vec::new();
    for &k in &drawn {
        let m = monsters.get(k).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("monster {k}: none"))?;
        foes.push(barrel_escha::duel::Fighter { block: m.blocks[0].clone(), normal: skills.get(1).ok_or("no skill 1")?, skills: Some(&skills[..]), death_immune: m.death_immune, chara: barrel_escha::data::battle_data::MONSTER_CHARA_FIRST as i32 + k as i32 });
        names.push(m.name.clone());
    }
    println!("against {}", names[members.len()..].join(", "));
    let acts_info = bd::acts(&pe, bd::ACTS_EN);
    let kinds = bd::trait_kinds(&pe);
    let rules = barrel_escha::skirmish::ItemRules { acts: &acts_info, kinds: &kinds, effects: &item_rows };
    let radii = chara_radii_near(exe);
    let sizes: Vec<f32> = drawn.iter().map(|&k| barrel_escha::data::battle_data::monster_size(&radii, k).unwrap_or(100.0)).collect();
    println!("sizes {sizes:?}");
    let (events, winner) = barrel_escha::skirmish::skirmish(&members, &foes, &sizes, rules, &mut rng, 1000);
    for e in &events {
        match e {
            Event::Felled { who, skill, target } => println!("{} uses {skill} on {}: felled", names[*who], names[*target]),
            Event::Poisoned { who, damage, hp_left } => println!("{} takes {damage} poison damage ({hp_left} HP left)", names[*who]),
            Event::Acted { who, skill, target, damage, hp_left, .. } => println!("{} uses {skill} on {}: {damage} damage ({hp_left} HP left)", names[*who], names[*target]),
            Event::Healed { who, skill, target, amount, hp_left, .. } => println!("{} uses {skill} on {}: heals {amount} ({hp_left} HP)", names[*who], names[*target]),
        }
    }
    println!("{}", match winner { Some(0) => "the party wins".to_string(), Some(_) => "the monsters win".to_string(), None => format!("no winner after {} events", events.len()) });
    Ok(())
}
