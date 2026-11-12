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
  barrel ebd FILE.ebd          print an event's actions as words
  barrel ebd-scan DIR          read every event under DIR and count its commands
  barrel ebd-decode DIR        decode every event under DIR into commands (Escha's table)
  barrel xml FILE.xml          print a table's rows
  barrel xml-scan DIR          read every XML table under DIR and list what it holds
  barrel g1t FILE OUTDIR       write a texture pack's textures as PNG (FILE.elixir.gz#N for an entry)
  barrel g1t-scan DIR          read every texture, loose and in containers
  barrel g1m-obj FILE#N OUT.obj  write a model's submeshes as Wavefront OBJ
  barrel g1m-materials FILE#N  a model's materials: each texture's index, kind and subtype, and which submeshes use it
  barrel g1h FILE#N            list a morph file's sets and targets
  barrel g1h-scan DIR          read every morph file in the containers under DIR
  barrel g1m-scan DIR          read every model in the containers under DIR
  barrel oid-scan DIR          read every *Oid.bin bone-name list in the containers
  barrel ebm FILE.ebm          print an event's messages
  barrel ebm-scan DIR          read every message file under DIR
  barrel opl2-scan DIR         read every field placement list in the containers
  barrel opl2 FILE#N              a field's placements: each object, its instances and their tags
  barrel ktf-scan DIR          read every field terrain in the containers
  barrel g1a FILE#N            list an animation's tracks (bone, kind, values over time)
  barrel g1a-moves FILE#N [OID#N]  which bones an animation turns, and how far (names from a bone list)
  barrel g1a-scan DIR          read every animation in the containers
  barrel pak-ls FILE.PAK       list a PAK archive's files (Sophie 2, Ryza 1-3)
  barrel pak-get FILE.PAK NAME OUT  extract one file from a PAK archive
  barrel pak-scan FILE.PAK...  read every texture, model, animation, morph, event and message in archives
  barrel xml-e FILE.xml.e OUT.xml  decrypt an encrypted table (Sophie 2, Ryza)
  barrel pak-xml-e FILE.PAK    decrypt every encrypted table in an archive, compared with its plain twin
  barrel faces EXE             list each character's facial animation names (by CharaBase row)
  barrel board INSTALL DAY [--events] [--offset=N]  the request board drawn on a game day (--events: every
      request's start event seen; --offset: the save's board seed offset, global 0x1410989f0, default 0)
  barrel days INSTALL N [--events]  a new game's first N days (shops emptied at the start): board redraws, month closes, restocks
  barrel save GAMEDATA.pcsave   a save's plain header (the body is encrypted, not read yet)
  barrel vorbis-scan SeriesNNNN.fpl|DIR  read every clip's (or every .kvs in DIR) Vorbis headers (identification and setup) in a voice bank
  barrel voice-wav SeriesNNNN.fpl CLIP OUT.wav [OUT.ogg]  decode one clip natively to 16-bit WAV (and its Ogg)
  barrel stream-check FILE.kvs   checks packet-at-a-time decoding (and seeking to the loop) against a whole decode
  barrel font-text EXE FONT.g1t OUT.png TEXT   lays out TEXT in the game's font (glyph table from EXE)
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
        ["voice-scan", dir] => voice_scan(Path::new(dir)),
        ["ebd-ops", dir, word] => ebd_ops(Path::new(dir), word, None),
        ["ebd-ops", dir, word, filter] => ebd_ops(Path::new(dir), word, Some(filter)),
        ["xml", file] => xml(Path::new(file)),
        ["xml-scan", dir] => xml_scan(Path::new(dir)),
        ["g1t", file, out] => g1t(file, Path::new(out)),
        ["g1t-scan", dir] => g1t_scan(Path::new(dir)),
        ["g1m-obj", file, out] => g1m_obj(file, Path::new(out)),
        ["g1m-materials", file] => g1m_materials(file),
        ["g1h", file] => g1h(file),
        ["g1h-scan", dir] => g1h_scan(Path::new(dir)),
        ["g1m-scan", dir] => g1m_scan(Path::new(dir)),
        ["oid-scan", dir] => oid_scan(Path::new(dir)),
        ["ebm", file] => ebm(Path::new(file)),
        ["ebm-scan", dir] => ebm_scan(Path::new(dir)),
        ["opl2-scan", dir] => opl2_scan(Path::new(dir)),
        ["opl2", file] => opl2_list(file),
        ["ktf-scan", dir] => ktf_scan(Path::new(dir)),
        ["g1a", file] => g1a(file),
        ["g1a-moves", file, rest @ ..] => g1a_moves(file, rest.first().copied()),
        ["g1a-scan", dir] => g1a_scan(Path::new(dir)),
        ["pak-ls", file] => pak_ls(Path::new(file)),
        ["pak-get", file, name, out] => pak_get(Path::new(file), name, Path::new(out)),
        ["pak-scan", files @ ..] if !files.is_empty() => pak_scan(files),
        ["pak-bc6h", files @ ..] if !files.is_empty() => pak_bc6h(files),
        ["xml-e", file, out] => xml_e(Path::new(file), Path::new(out)),
        ["pak-xml-e", file] => pak_xml_e(Path::new(file)),
        ["faces", exe] => faces(Path::new(exe)),
        ["gestures", exe, numbers @ ..] => gestures(Path::new(exe), numbers),
        ["duel", exe, a, b] => duel(Path::new(exe), a, b, None),
        ["battle", exe, monster, party @ ..] if !party.is_empty() => battle(Path::new(exe), monster, party),
        ["duel", exe, a, b, "--equip", item] => duel(Path::new(exe), a, b, Some(item)),
        ["party", exe] => party(Path::new(exe)),
        ["item-effects", exe] => item_effects(Path::new(exe)),
        ["acts", exe] => acts(Path::new(exe)),
        ["trait-kinds", exe] => trait_kinds(Path::new(exe)),
        ["party-skills", exe] => party_skills(Path::new(exe)),
        ["monsters", exe] => monsters(Path::new(exe)),
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
        ["font-text", exe, g1t, out, text] => font_text(Path::new(exe), Path::new(g1t), Path::new(out), text),
        ["stream-check", kvs] => stream_check(Path::new(kvs)),
        ["voice-wav", fpl, clip, out, rest @ ..] if rest.len() <= 1 => voice_wav(Path::new(fpl), clip, Path::new(out), rest.first().map(Path::new)),
        ["save", file] => save(Path::new(file)),
        ["days", install, n] => days(Path::new(install), n, false, None),
        ["days", install, n, "--events"] => days(Path::new(install), n, true, None),
        ["days", install, n, "--from", save] => days(Path::new(install), n, false, Some(Path::new(save))),
        ["board", install, day, rest @ ..] if rest.iter().all(|a| *a == "--events" || a.starts_with("--offset=")) => {
            board(Path::new(install), day, rest.contains(&"--events"), rest.iter().find_map(|a| a.strip_prefix("--offset=")))
        }
        ["shops", install] => shops(Path::new(install), None),
        ["shops", install, lang] => shops(Path::new(install), Some(lang)),
        ["recipes", exe] => recipes(Path::new(exe)),
        ["synth", install, item, materials @ ..] if !materials.is_empty() => {
            synth(Path::new(install), item, materials)
        }
        ["play-event", file, "--stage"] => stage_event(Path::new(file), None),
        ["play-event", file, "--stage", "--fps", hz] => stage_event(Path::new(file), Some(hz)),
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
    commands.sort_by(|a, b| b.1.cmp(&a.1));
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
    kinds.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    for (k, (n, bytes)) in kinds {
        println!("{n:>7}  {:>8.1} MB  {k}", bytes as f64 / 1e6);
    }
    for f in failed.iter().take(20) {
        println!("failed: {f}");
    }
    Ok(())
}

fn ebd_decode(dir: &Path) -> Result<()> {
    let table = OperandTable::escha();
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
    let table = OperandTable::escha();
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

fn describe_position(p: &barrel_game::camera::Position) -> String {
    use barrel_game::camera::Anchor;
    let [x, y, z] = p.offset;
    match p.anchor {
        Anchor::World => format!("({x}, {y}, {z})"),
        Anchor::Saved(n) => format!("saved point {n} + ({x}, {y}, {z})"),
        Anchor::Thing { kind, id, point } => format!("thing {kind}/{id} point {point} + ({x}, {y}, {z})"),
    }
}

fn describe_move(m: &barrel_game::camera::Move) -> String {
    use barrel_game::camera::Move;
    let to = describe_position(m.to());
    match *m {
        Move::Straight { seconds, ease, .. } => format!("moves to {to} in {seconds:.2} s (ease {ease})"),
        Move::Speed { speed, accel, .. } if accel == [0.0; 3] => format!("goes to {to} at {speed}/s"),
        Move::Speed { speed, accel, .. } => format!("goes to {to} at {speed}/s, accelerating {accel:?}"),
        Move::Jump { first, height, .. } => format!("jumps to {to}, {height} high ({first})"),
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

fn open_pak(file: &Path) -> Result<(std::fs::File, barrel_formats::pak::Pak)> {
    use barrel_formats::pak::Pak;
    use std::io::Read;
    let mut f = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut header = [0u8; 16];
    f.read_exact(&mut header).map_err(|e| format!("{}: {e}", file.display()))?;
    let size = Pak::table_size(&header).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut table = header.to_vec();
    table.resize(size, 0);
    f.read_exact(&mut table[16..]).map_err(|e| format!("{}: {e}", file.display()))?;
    let (pak, _) = Pak::parse_any(&table).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok((f, pak))
}

fn pak_ls(file: &Path) -> Result<()> {
    let (_, pak) = open_pak(file)?;
    for e in &pak.entries {
        println!("{:>10} {}", e.size, e.name);
    }
    println!("{} files", pak.entries.len());
    Ok(())
}

fn pak_read(
    f: &mut std::fs::File,
    pak: &barrel_formats::pak::Pak,
    entry: &barrel_formats::pak::Entry,
) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut data = vec![0u8; entry.size as usize];
    f.seek(SeekFrom::Start(entry.offset))?;
    f.read_exact(&mut data)?;
    pak.unscramble(entry, 0, &mut data);
    Ok(data)
}

fn pak_get(file: &Path, name: &str, out: &Path) -> Result<()> {
    let (mut f, pak) = open_pak(file)?;
    let entry = pak.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or("no such file in the archive")?;
    let data = pak_read(&mut f, &pak, entry).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::write(out, &data).map_err(|e| format!("{}: {e}", out.display()))?;
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
        let (mut fh, pak) = open_pak(Path::new(file))?;
        for entry in &pak.entries {
            if !entry.name.ends_with(".g1t") || entry.size > 256 << 20 {
                continue;
            }
            let data = pak_read(&mut fh, &pak, entry).map_err(|e| format!("{file}: {e}"))?;
            let Ok(g) = barrel_formats::g1t::G1t::parse(&data) else { continue };
            for t in g.textures.iter().filter(|t| t.format == barrel_formats::g1t::Format::Bc6h) {
                for level in &t.levels {
                    for block in level.chunks_exact(16) {
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
        let (mut fh, pak) = open_pak(Path::new(file))?;
        for entry in &pak.entries {
            let name = entry.name.as_str();
            let interesting = [".g1t", ".g1m", ".g1a", ".g1h", ".elixir.gz", ".ebd", ".ebm"].iter().any(|x| name.ends_with(x));
            if !interesting || entry.size > 512 << 20 {
                continue;
            }
            let data = pak_read(&mut fh, &pak, entry).map_err(|e| format!("{file}: {e}"))?;
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
    let (mut f, pak) = open_pak(file)?;
    let by_name: BTreeMap<&str, &barrel_formats::pak::Entry> =
        pak.entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let (mut same, mut differ, mut alone, mut failed) = (0, 0, 0, 0);
    for e in pak.entries.iter().filter(|e| e.name.ends_with(".xml.e")) {
        let data = pak_read(&mut f, &pak, e).map_err(|err| format!("{}: {err}", file.display()))?;
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
                let t = pak_read(&mut f, &pak, twin).map_err(|err| format!("{}: {err}", file.display()))?;
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
    use barrel_game::quest::{requests, Kind};
    let mut total = 0;
    for (file, kind) in [("QuestAlc", Kind::Synthesis), ("QuestBtl", Kind::Monster), ("QuestGat", Kind::Gathering)] {
        let path = install.join(format!("Saves/quest/{file}.xml"));
        let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        let rs = requests(&table, kind);
        println!("{file}: {} requests", rs.len());
        for r in rs.iter().take(5) {
            let prizes: Vec<String> = r.prizes.iter().map(|(t, n)| format!("{t} x{n}")).collect();
            let date = |n: i32| {
                let d = barrel_game::calendar::Date::of(n.max(0) as u32);
                format!("Y{} {}/{}", d.year, d.month, d.day)
            };
            println!("  {:<32} {:>6.2} pt  days {}-{} ({} to {})  {}", r.target, r.points, r.from_day, r.to_day, date(r.from_day), date(r.to_day), prizes.join(", "));
        }
        total += rs.len();
    }
    println!("{total} requests (the first five of each shown; points and counts as the tables give them)");
    Ok(())
}

/// The request board as it would be redrawn on `day` (see `barrel_game::quest::draw_board`):
/// no assignment cleared, the seed offset given (0 by default), the targets' states
/// taken as known, battle candidates sorted by their monster's first stat block level.
fn board(install: &Path, day: &str, events: bool, offset: Option<&str>) -> Result<()> {
    let offset: i32 = offset.map_or(Ok(0), |v| v.parse().map_err(|_| format!("{v}: not a number")))?;
    use barrel_game::quest::{draw_board, requests, Kind};
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
    let monsters = barrel_formats::battle_data::monsters(&pe, barrel_formats::battle_data::MONSTERS_EN).ok_or("no monster table")?;
    let (tags, _) = pe.enum_names("MONSTER_ROCKRAT_00").ok_or("no MONSTER_* enum")?;
    let level = |r: &barrel_game::quest::Request| {
        // The array holds several enums; the monsters' numbers count from its first MONSTER_ tag.
        let first = tags.iter().position(|t| t.starts_with("MONSTER_")).unwrap_or(0);
        tags.iter().position(|t| *t == r.target).and_then(|i| i.checked_sub(first)).and_then(|i| monsters.get(i)).and_then(|m| m.blocks.first()).map_or(0, |b| b.level)
    };
    let starts: std::collections::HashSet<String> = all.iter().filter_map(|r| r.from_event.clone()).collect();
    let seen = |e: &str| events && starts.contains(e);
    let slots = draw_board(&all, day, offset, seen, |_| false, |_| true, level);
    let d = barrel_game::calendar::Date::of(day.max(0) as u32);
    println!("day {day} (Y{} {}/{}){}", d.year, d.month, d.day, if barrel_game::calendar::board_day(day.max(0) as u32) { ", a redraw day" } else { ", not a redraw day (drawn as if it were)" });
    for (k, s) in slots.iter().enumerate() {
        let what = ["synthesis", "synthesis", "synthesis", "battle", "battle", "battle", "battle", "gathering", "gathering", "gathering"][k];
        match s {
            Some(i) => println!("  {k} {what:<9} {} ({:.0}%, {:.2} pt)", all[*i].target, all[*i].priority, all[*i].points),
            None => println!("  {k} {what:<9} -"),
        }
    }
    Ok(())
}

/// A new game's first `n` days on `barrel_game::days::World` (fixed generator state, everyone in
/// the list in slot order, no assignment cleared, the item states taken as high enough).
fn days(install: &Path, n: &str, events: bool, from: Option<&Path>) -> Result<()> {
    use barrel_game::days::{Reason, Tables, World};
    use barrel_game::quest::{requests, Kind};
    let n: u32 = n.parse().map_err(|_| format!("{n}: not a number of days"))?;
    let mut all = Vec::new();
    for (file, kind) in [("QuestAlc", Kind::Synthesis), ("QuestBtl", Kind::Monster), ("QuestGat", Kind::Gathering)] {
        let path = install.join(format!("Saves/quest/{file}.xml"));
        all.extend(requests(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?, kind));
    }
    let path = install.join("Saves/rank/RankData.xml");
    let ranks = barrel_game::rank::ranks(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?);
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
    let seen = |e: &str| events && starts.contains(e);
    let path = install.join("Saves/shop/ShopData.xml");
    let wares = barrel_game::shop::wares(&barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?);
    let t = Tables { requests: &all, wares: &wares, ranks: &ranks, recipe_cost: &recipe_cost, seen: &seen, cleared: &|_| false, known: &|_| true, level: &|_| 0 };
    let rng = barrel_game::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    let mut w = if let Some(file) = from {
        // A save's calendar (the main generator is not saved: a fixed one stands in).
        let data = read(file)?;
        let body = &barrel_formats::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?.body;
        let state = barrel_formats::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
        let records = barrel_formats::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?;
        let summary = barrel_formats::save::summary(&records);
        let (items, _) = pe.enum_names("ITEM_MIX_CRAFT").ok_or("no ITEM_* enum")?;
        let (names, _) = pe.enum_names("MONSTER_ROCKRAT_00").ok_or("no MONSTER_* enum")?;
        let monsters = &names[names.iter().position(|t| t.starts_with("MONSTER_")).unwrap_or(0)..];
        let board = barrel_game::quest::board_from_save(&summary.board, &all, &items, monsters);
        let w = World::from_save(&summary, rng, &wares, board);
        let posted: Vec<&str> = w.board.iter().map(|s| s.map_or("-", |i| all[i].target.as_str())).collect();
        println!("board as saved: {}", posted.join(", "));
        let d = barrel_game::calendar::Date::of(w.date);
        println!("from {}: day {} (Y{} {}/{}), rank {} ({} points), squad {:?}", file.display(), w.date, d.year, d.month, d.day, w.rank.rank, w.rank.points, w.joined);
        w
    } else {
        let mut w = World::new_game(rng, 0, &wares);
        // A buyer who empties every shop on the first day.
        for s in &mut w.stock {
            s.count = 0;
        }
        w.joined = (0..barrel_game::days::MEMBERS).collect();
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
        let d = barrel_game::calendar::Date::of(w.date);
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

/// A save's header.
fn save(file: &Path) -> Result<()> {
    let data = read(file)?;
    // SYSDATA (the settings) is coded whole; a GAMEDATA file after its plain header.
    let body = if file.file_name().is_some_and(|n| n.to_string_lossy().starts_with("SYSDATA")) {
        &data[..]
    } else {
        let s = barrel_formats::save::parse(&data).map_err(|e| format!("{}: {e}", file.display()))?;
        let t = s.saved_at;
        println!("saved {:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second);
        println!("summary {:?}", s.summary);
        s.body
    };
    let state = barrel_formats::save::decode(body).map_err(|e| format!("{}: {e}", file.display()))?;
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
    fn show(r: &barrel_formats::save::Record, depth: usize) {
        use barrel_formats::save::Body;
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
    let records = barrel_formats::save::state_records(&state).map_err(|e| format!("{}: {e}", file.display()))?;
    for r in &records {
        show(r, 0);
    }
    let s = barrel_formats::save::summary(&records);
    let date = s.day.map(|d| {
        let c = barrel_game::calendar::Date::of(d);
        format!("{d} (year {}, {}/{})", c.year, c.month, c.day)
    });
    println!("money {:?}, alchemy (level, exp) {:?}, day {}, sizes {:?}", s.money, s.alchemy, date.unwrap_or_else(|| "?".into()), s.limits);
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

/// Writes `text` in the English font, white on dark gray, to a PNG (a check of the glyph
/// table against the atlas).
/// Every narration card (`u55`): its layout and the texture pieces it shows.
fn telops(exe: &Path) -> Result<()> {
    let bytes = std::fs::read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&bytes)?;
    for card in 0..=50 {
        let Some(id) = barrel_formats::layout::telop_layout(&pe, card) else { continue };
        let pieces = barrel_formats::layout::pieces(&pe, id);
        let list: Vec<String> = pieces.iter().map(|p| format!("{} {:?} at {:?}", p.texture, p.src, p.at)).collect();
        println!("card {card:2} layout {id}: {}", list.join("; "));
    }
    Ok(())
}

fn font_text(exe: &Path, g1t: &Path, out: &Path, text: &str) -> Result<()> {
    use barrel_formats::{font, g1t as tex};
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let glyphs = font::glyphs(&pe, font::FONT_EN).ok_or("no font table at the English executable's address")?;
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
    for px in img.chunks_exact_mut(4) {
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
    let wares = barrel_game::shop::wares(&table);
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
fn monsters(exe: &Path) -> Result<()> {
    use barrel_formats::battle_data as bd;
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let monsters = bd::monsters(&pe, bd::MONSTERS_EN).ok_or("no monster table at the English executable's address")?;
    let skills = bd::skills(&pe, bd::SKILLS_EN).ok_or("no skill table at the English executable's address")?;
    for (n, m) in monsters.iter().enumerate() {
        println!("{n:3} {}{}", m.name, if m.boss { " (boss)" } else { "" });
        for b in &m.blocks {
            let names: Vec<String> = b.skills.iter().map(|(s, _)| skills.get(*s as usize).map_or_else(|| s.to_string(), |k| k.name.clone())).collect();
            println!("      lv {:2}  {}/{}/{}/{}  {}", b.level, b.hp, b.attack, b.defense, b.speed, names.join(", "));
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
    use barrel_formats::battle_data as bd;
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
    use barrel_formats::battle_data as bd;
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
/// `barrel_game::duel` for what it leaves out.
fn duel(exe: &Path, a: &str, b: &str, equip: Option<&str>) -> Result<()> {
    use barrel_formats::battle_data as bd;
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
            return Ok((name, barrel_game::duel::party_block(l as i32, stats), attack));
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
        let row = table.children.iter().find(|r| r.attr("ItemName") == Some(name)).ok_or_else(|| format!("no item named {name}"))?;
        let n = |k: &str| row.attr(k).and_then(|v| v.parse().ok()).unwrap_or(0);
        let e = barrel_game::duel::Equipment {
            hp: n("HP"), mp: n("MP"), attack: n("ATK"), defense: n("DEF"), speed: n("SPD"), quality,
        };
        ba = barrel_game::duel::equip(&ba, &[e]);
        println!("{na} equips {name} (quality {quality}): attack {}, defence {}, speed {}", ba.attack, ba.defense, ba.speed);
    }
    let mut rng = barrel_game::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    // Monsters pick by their rules from the skill table; party members use the normal attack.
    let table = |s: &str| (!s.starts_with('p')).then_some(&skills[..]);
    let fa = barrel_game::duel::Fighter { block: ba, normal: aa, skills: table(a) };
    let fb = barrel_game::duel::Fighter { block: bb, normal: ab, skills: table(b) };
    let hits = barrel_game::duel::duel(&fa, &fb, &mut rng, 1000);
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
    println!("{} hits (partial battle: no traits or start rules, states only poison and stat buffs; see barrel_game::duel)", hits.len());
    Ok(())
}

/// Each row of `Saves/item/item_effect.xml` with its battle effects (act names from EXE).
fn item_effects(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let (acts, _) = pe.enum_names("ACT_DAMAGE_HP").ok_or("no ACT_* enum")?;
    let path = exe.parent().ok_or("no install folder")?.join("Saves_EN/item/item_effect.xml");
    let root = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows = barrel_formats::battle_data::item_effects(&root, &acts);
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

/// Party members against a monster: see `barrel_game::skirmish`.
fn battle(exe: &Path, monster: &str, party: &[&str]) -> Result<()> {
    use barrel_formats::battle_data as bd;
    use barrel_game::fight::Event;
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
        let block = barrel_game::duel::party_block(l as i32, stats);
        let mut member = barrel_game::skirmish::Member::new(block, stats.mp, &lists[m], &skills).ok_or("no party attack")?;
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
                let at = barrel_formats::recipe::find(&data, ESCHA_FIRST_RECIPE).ok_or("no recipe table found")?;
                let recipes = barrel_formats::recipe::parse(&data, at).map_err(|e| format!("recipes: {e}"))?;
                let r = recipes.iter().find(|r| i32::from(r.item) == b.item).ok_or_else(|| format!("{item}: no recipe makes item {}", b.item))?;
                (0..4).filter_map(|k| barrel_formats::recipe::effect_at(&r.lines[k], values.get(k).copied().unwrap_or(0))).map(|e| e as i16).collect()
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
            let mut action = barrel_game::battle::item_action(&name, 2, 600, &ids, &item_rows);
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
            member.items.push(barrel_game::skirmish::Held { action, power: barrel_game::battle::item_power(quality), count, traits });
        }
        members.push(member);
        names.push(format!("{} ({l})", member_names[m]));
    }
    let n: usize = monster.parse().map_err(|_| format!("{monster}: not a monster number"))?;
    let m = monsters.get(n).filter(|m| !m.blocks.is_empty()).ok_or_else(|| format!("{monster}: no such monster"))?;
    names.push(m.name.clone());
    let foe = barrel_game::duel::Fighter { block: m.blocks[0].clone(), normal: skills.get(1).ok_or("no skill 1")?, skills: Some(&skills[..]) };
    let mut rng = barrel_game::rng::Rng::from_state([123456789, 362436069, 521288629, 88675123]);
    let acts = bd::acts(&pe, bd::ACTS_EN);
    let kinds = bd::trait_kinds(&pe);
    let rules = barrel_game::skirmish::ItemRules { acts: &acts, kinds: &kinds, effects: &item_rows };
    let (events, winner) = barrel_game::skirmish::skirmish(&members, &foe, rules, &mut rng, 1000);
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

fn faces(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    let t = barrel_game::face::Tables::read(&pe);
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
    use barrel_formats::recipe::{self, Ingredient};
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

fn synth(install: &Path, item: &str, materials: &[&str]) -> Result<()> {
    use barrel_formats::recipe;
    use barrel_game::synthesis::{Material, Synthesis};
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
    let wanted = barrel_game::synthesis::order(recipe);
    let mut s = Synthesis::new();
    for (i, m) in materials.iter().enumerate() {
        let m = number(m)?;
        let row = items.get(m).ok_or_else(|| format!("no item {m}"))?;
        let cats: Vec<u16> = (0..4)
            .filter_map(|k| row.attr(&format!("category_{k:02}")))
            .filter_map(|tag| categories.iter().position(|c| c == tag))
            .map(|n| n as u16)
            .collect();
        match wanted.get(i) {
            Some(&w) if barrel_game::synthesis::fits(w, m as u16, &cats) => {}
            Some(w) => println!("  ! material {} does not fit {w:?} (the game takes them in order without Freestyle)", i + 1),
            None => println!("  ! material {} is one more than the recipe takes ({})", i + 1, wanted.len()),
        }
        s.add(&Material::from_item(row));
        println!("  + {} ({m}): attributes {:?}, effect value {}", name(&items, m, "ItemName"), s.attributes, s.effect_value());
    }
    if materials.len() < wanted.len() {
        println!("  ! the recipe takes {} materials", wanted.len());
    }
    for (k, (element, e)) in ["Fire", "Water", "Wind", "Earth"].iter().zip(s.effects(&recipe.lines)).enumerate() {
        let what = e.map_or("-".to_string(), |e| name(&effects, e as usize, "ItemEffectName"));
        println!("  {element:5} {:>3}: {what}", s.attributes[k] as i32);
    }
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

fn g1a(spec: &str) -> Result<()> {
    let bytes = file_or_entry(spec)?;
    let a = barrel_formats::g1a::parse(&bytes).map_err(|e| format!("{spec}: {e}"))?;
    println!("{:.3} s, {} tracks", a.duration, a.tracks.len());
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
                println!("       at {:.1?} turned {:.1?} scaled {:.2?}", i.position, i.rotation, i.scale);
            }
        }
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

fn ktf_scan(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    files_ending(dir, ".elixir.gz", &mut files)?;
    files.sort();
    let mut terrains = 0usize;
    let mut layers: BTreeMap<usize, usize> = BTreeMap::new();
    let mut failed = Vec::new();
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
                }
                Err(err) => failed.push(format!("{}#{i}: {err}", path.display())),
            }
        }
    }
    println!("{terrains} terrains, {} failed; layer counts {layers:?}", failed.len());
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
    use barrel_game::event::Player;
    use barrel_game::stage::Stage;
    let event = Ebd::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
    let table = OperandTable::escha();
    let messages = match message_file(file, None) {
        Some(m) if m.is_file() => barrel_formats::ebm::parse(&read(&m)?).unwrap_or_default(),
        _ => Vec::new(),
    };
    let steps = Player::escha(&event, &messages, &table)?.schedule();
    // Story events number their characters by Chara.xml row.
    let measures = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| read(&root.join("Saves/chara/Chara.xml")).ok())
        .and_then(|b| barrel_formats::table::parse(&b).ok())
        .map(|t| barrel_game::stage::Measures::table(&t))
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
        .map(|t| barrel_game::curve::Curve::table(&t))
        .unwrap_or_default();
    println!("({} curves)", curves.len());
    // As a renderer drawing HZ frames a second drives it: the clock decides the ticks; the
    // end state must not depend on HZ (docs/engine.md, "Timing").
    if let Some(hz) = fps {
        use barrel_game::clock::Clock;
        let end = steps.last().map_or(0.0, |t| t.time as f64) + 1.0;
        let mut clock = Clock::default();
        let mut play = barrel_game::stage::Playback::new(Stage { measures, curves, ..Stage::default() }, steps);
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
    use barrel_game::event::{Player, Step};
    let event = Ebd::parse(&read(file)?).map_err(|e| format!("{}: {e}", file.display()))?;
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
        .map(|data| barrel_formats::pe::Pe::parse(&data).map(|pe| barrel_game::gesture::motions(&pe)).unwrap_or_default())
        .unwrap_or_default();
    let fields: Vec<String> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .and_then(|root| ["Atelier_Escha_and_Logy_EN.exe", "Atelier_Escha_and_Logy.exe"].iter().map(|n| root.join(n)).find(|p| p.is_file()))
        .and_then(|exe| read(&exe).ok())
        .map(|data| barrel_formats::pe::Pe::parse(&data).map(|pe| barrel_game::event::field_maps(&pe)).unwrap_or_default())
        .unwrap_or_default();
    // The music list, from the English executable (where its address is known).
    let tracks: Vec<barrel_formats::bgm::Track> = file
        .ancestors()
        .find(|p| p.join("Saves").is_dir())
        .map(|root| root.join("Atelier_Escha_and_Logy_EN.exe"))
        .and_then(|exe| read(&exe).ok())
        .and_then(|data| barrel_formats::pe::Pe::parse(&data).ok().and_then(|pe| barrel_formats::bgm::tracks(&pe, barrel_formats::bgm::TRACKS_EN)))
        .unwrap_or_default();
    let table = OperandTable::escha();
    let player = Player::escha(&event, &messages, &table)?;
    let steps: Vec<(Option<f32>, Step)> = if timed {
        // BARREL_READ: hold each line its unattended reading time, as the viewer does.
        let player = if std::env::var("BARREL_READ").is_ok() { player.reading_time(barrel_game::event::reading_time) } else { player };
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
            Step::CharacterWait { action, character, seconds } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("{action:3} {who} waits {seconds:.2} s");
            }
            Step::Place { action, character, position } => {
                let who = names.get(character as usize).map_or("?", String::as_str);
                println!("{action:3} {who} at ({}, {}, {})", position[0], position[1], position[2]);
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
            Step::Fade { action, preset, seconds } => println!("{action:3} --- fade (preset {preset}) over {seconds} s"),
            Step::ScreenEffect { action, kind, param, seconds } => {
                const KINDS: [&str; 17] = [
                    "fade", "blur", "old sepia", "cross-fade", "battle start", "negative", "glow", "refraction", "?", "hue", "gamma", "colour",
                    "blur", "sepia", "monotone", "texture filter", "wave",
                ];
                let name = KINDS.get(kind as usize).copied().unwrap_or("?");
                println!("{action:3} --- screen effect {kind} ({name}), {param}, over {seconds} s");
            }
            Step::FieldCamera { action, change } => {
                use barrel_game::field_camera::Change as C;
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
                use barrel_game::camera::Change;
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

/// Message gestures (`barrel_game::gesture`): each entry's two forms as motion names.
fn gestures(exe: &Path, numbers: &[&str]) -> Result<()> {
    let data = read(exe)?;
    let table = barrel_game::gesture::read(&data).ok_or("gesture table not found")?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{e}"))?;
    let names = barrel_game::gesture::motions(&pe);
    let name = |m: u32| names.get(m as usize).cloned().unwrap_or_else(|| m.to_string());
    let picked: Vec<usize> = if numbers.is_empty() { (0..table.len()).collect() } else { numbers.iter().filter_map(|n| n.parse().ok()).collect() };
    for n in picked {
        let Some(entry) = table.get(n) else { continue };
        for (k, form) in entry.iter().enumerate() {
            let Some(f) = form else { continue };
            let steps: Vec<String> = f.steps.iter().map(|s| match s {
                barrel_game::gesture::Step::Motion(m) => name(*m),
                barrel_game::gesture::Step::Repeat(r) => format!("x{r}"),
            }).collect();
            println!("{n:3}.{k}: {} {} end {}", name(f.first), steps.join(" "), f.end);
        }
    }
    Ok(())
}

/// The delivery table (`barrel_game::delivery`): each assignment card's items and bonuses.
fn deliveries(install: &Path) -> Result<()> {
    let path = install.join("Saves/delivery/DeliveryData.xml");
    let table = barrel_formats::table::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let all = barrel_game::delivery::deliveries(&table);
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
    for (k, a) in barrel_formats::battle_data::acts(&pe, barrel_formats::battle_data::ACTS_EN).iter().enumerate() {
        let flags: Vec<String> = a.flags.iter().enumerate().filter(|(_, f)| **f).map(|(i, _)| i.to_string()).collect();
        println!("{k:3} {:32} flags {:20} limit {} last {}", names.get(k).map_or("?".to_string(), |n| n.to_string()), flags.join(","), a.limit, a.last);
    }
    Ok(())
}

fn trait_kinds(exe: &Path) -> Result<()> {
    let data = read(exe)?;
    let pe = barrel_formats::pe::Pe::parse(&data).map_err(|e| format!("{}: {e}", exe.display()))?;
    for (k, n) in barrel_formats::battle_data::trait_kinds(&pe).iter().enumerate() {
        println!("{k:3} {n}");
    }
    Ok(())
}
