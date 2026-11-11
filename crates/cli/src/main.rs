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
  barrel ebd-decode DIR        decode every event under DIR into commands (Escha's table)";

fn main() -> ExitCode {
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
