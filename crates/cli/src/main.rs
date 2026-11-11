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
  barrel g1h FILE#N            list a morph file's sets and targets
  barrel g1h-scan DIR          read every morph file in the containers under DIR
  barrel g1m-scan DIR          read every model in the containers under DIR
  barrel oid-scan DIR          read every *Oid.bin bone-name list in the containers
  barrel ebm FILE.ebm          print an event's messages
  barrel ebm-scan DIR          read every message file under DIR
  barrel opl2-scan DIR         read every field placement list in the containers
  barrel ktf-scan DIR          read every field terrain in the containers
  barrel g1a-scan DIR          read every animation in the containers
  barrel pak-ls FILE.PAK       list a PAK archive's files (Sophie 2, Ryza 1-3)
  barrel pak-get FILE.PAK NAME OUT  extract one file from a PAK archive
  barrel xml-e FILE.xml.e OUT.xml  decrypt an encrypted table (Sophie 2, Ryza)
  barrel pak-xml-e FILE.PAK    decrypt every encrypted table in an archive, compared with its plain twin
  barrel recipes EXE           list the synthesis recipes compiled into Escha & Logy DX's executable
  barrel synth INSTALL ITEM MATERIAL...  simulate a synthesis (item numbers; names from Saves_EN)
  barrel play-event FILE.ebd [LANG]  run an event and print what happens (LANG: EN, SC, TC; default JP)";

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
        ["xml", file] => xml(Path::new(file)),
        ["xml-scan", dir] => xml_scan(Path::new(dir)),
        ["g1t", file, out] => g1t(file, Path::new(out)),
        ["g1t-scan", dir] => g1t_scan(Path::new(dir)),
        ["g1m-obj", file, out] => g1m_obj(file, Path::new(out)),
        ["g1h", file] => g1h(file),
        ["g1h-scan", dir] => g1h_scan(Path::new(dir)),
        ["g1m-scan", dir] => g1m_scan(Path::new(dir)),
        ["oid-scan", dir] => oid_scan(Path::new(dir)),
        ["ebm", file] => ebm(Path::new(file)),
        ["ebm-scan", dir] => ebm_scan(Path::new(dir)),
        ["opl2-scan", dir] => opl2_scan(Path::new(dir)),
        ["ktf-scan", dir] => ktf_scan(Path::new(dir)),
        ["g1a-scan", dir] => g1a_scan(Path::new(dir)),
        ["pak-ls", file] => pak_ls(Path::new(file)),
        ["pak-get", file, name, out] => pak_get(Path::new(file), name, Path::new(out)),
        ["xml-e", file, out] => xml_e(Path::new(file), Path::new(out)),
        ["pak-xml-e", file] => pak_xml_e(Path::new(file)),
        ["recipes", exe] => recipes(Path::new(exe)),
        ["synth", install, item, materials @ ..] if !materials.is_empty() => {
            synth(Path::new(install), item, materials)
        }
        ["play-event", file] => play_event(Path::new(file), None),
        ["play-event", file, lang] => play_event(Path::new(file), Some(lang)),
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
        println!("{} {}x{} {:?} ({:#04x}), {} levels", name.display(), t.width, t.height, t.format, t.format_code, t.levels.len());
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
        println!("[{} speaker {}] {}", m.kind, m.speaker, m.repaired_text().replace("<CR>", " / "));
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

fn play_event(file: &Path, lang: Option<&str>) -> Result<()> {
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
    let table = OperandTable::escha();
    for step in Player::escha(&event, &messages, &table)? {
        match step {
            Step::Message { index, speaker, text } => {
                let who = usize::try_from(speaker).ok().and_then(|i| names.get(i)).map_or("", String::as_str);
                println!("  [{index:3}] {who:>10}: {}", text.replace("<CR>", " "));
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
