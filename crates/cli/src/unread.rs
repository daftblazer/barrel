//! `barrel-tool unread FILE`: the byte ranges of a data file that Barrel's parser never uses.
//!
//! The parsers are black boxes here. A block of the file is "used" when inverting its bytes
//! changes what the parser returns (a hash of its `Debug` text; an error, or a panic, is its own
//! result). A block that changes nothing is unused; a block that does is split in halves and
//! tested again, down to a granularity, breadth first so a time budget leaves coarse answers.

use std::collections::VecDeque;
use std::fmt::{Debug, Write};
use std::hash::Hasher;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What the search found; every range is `(start, end)` in file bytes.
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    /// Ranges whose inversion changed nothing, merged where adjacent.
    pub unused: Vec<(usize, usize)>,
    /// Ranges that changed the result but were not split further when the budget ran out.
    pub unrefined: Vec<(usize, usize)>,
}

/// Search `data` for unused ranges, with `fingerprint` standing for the parser.
pub fn find_unused(data: &[u8], gran: usize, budget: Duration, fingerprint: &dyn Fn(&[u8]) -> u64) -> Report {
    let gran = gran.max(1);
    let start = Instant::now();
    let baseline = fingerprint(data);
    let mut buf = data.to_vec();
    let mut queue = VecDeque::new();
    seed(&mut queue, 0, data.len(), gran);
    let (mut unused, mut unrefined) = (Vec::new(), Vec::new());
    while let Some((a, b)) = queue.pop_front() {
        if a == b {
            continue;
        }
        if start.elapsed() >= budget {
            unrefined.push((a, b));
            continue;
        }
        buf[a..b].iter_mut().for_each(|x| *x ^= 0xff);
        let same = fingerprint(&buf) == baseline;
        buf[a..b].copy_from_slice(&data[a..b]);
        if same {
            unused.push((a, b));
        } else if b - a > gran {
            seed(&mut queue, a, b, gran);
        }
    }
    Report { unused: merge(unused), unrefined: merge(unrefined) }
}

/// Queue a range as its two halves: the whole file's inversion is never worth testing.
fn seed(queue: &mut VecDeque<(usize, usize)>, a: usize, b: usize, gran: usize) {
    if b - a > gran {
        let mid = a + ((b - a) / 2).div_ceil(gran) * gran;
        queue.extend([(a, mid), (mid, b)]);
    } else if b > a {
        queue.push_back((a, b));
    }
}

fn merge(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (a, b) in ranges {
        match out.last_mut() {
            Some(last) if last.1 >= a => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Feeds formatted text straight into a hasher, so large parse results are never built as strings.
struct HashWriter(std::collections::hash_map::DefaultHasher);

impl Write for HashWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

fn hash_debug(v: &dyn Debug) -> u64 {
    let mut w = HashWriter(Default::default());
    let _ = write!(w, "{v:?}");
    w.0.finish()
}

/// Hash a parse result; a panic counts as its own (always different) result.
fn guarded(f: impl FnOnce() -> u64) -> u64 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(u64::MAX)
}

type Fingerprint = Box<dyn Fn(&[u8]) -> u64>;

/// A fingerprint from a parse expression over `d`; results may borrow from `d`.
macro_rules! parser {
    (|$d:ident| $e:expr) => {
        Box::new(|$d: &[u8]| guarded(|| hash_debug(&$e))) as Fingerprint
    };
    ($f:path) => {
        parser!(|d| $f(d))
    };
}

/// The parser for a file, by its first four bytes and then by its name.
fn parser_for(name: &str, data: &[u8]) -> Option<(&'static str, Fingerprint)> {
    use barrel_formats as f;
    let magic = data.get(..4).unwrap_or(&[]);
    Some(match magic {
        b"_M1G" => ("g1m", parser!(|d| f::g1m::G1m::parse(d))),
        b"GT1G" => ("g1t", parser!(|d| f::g1t::G1t::parse(d))),
        b"_A1G" => ("g1a", parser!(f::g1a::parse)),
        b"_H1G" => ("g1h", parser!(f::g1h::parse)),
        b"OC1G" => ("g1co", parser!(f::g1co::parse)),
        b"KFTK" => ("ktf", parser!(f::ktf::parse)),
        b"_MHK" => ("khm", parser!(f::khm::parse)),
        b"SWGQ" => ("swg", parser!(f::swg::parse)),
        b"PSSG" => ("pssg", parser!(|d| f::pssg::Pssg::parse(d))),
        // A bank's header; the sample data it points into is not in the file.
        b"_HBW" => ("kwb", parser!(|d| f::kwb::sounds(d, 1 << 24))),
        _ if magic == b"CRAE" || name.ends_with(".elixir.gz") => (
            "elixir",
            // The entries' bytes are not parsed, only listed.
            parser!(|d| f::elixir::Elixir::parse(d).map(|e| (e.version, e.flags, e.entries.clone(), e.compressed))),
        ),
        _ if name.ends_with(".xml.e") => ("xml_e", parser!(f::xml_e::decrypt)),
        _ if name.ends_with(".xml") => ("table", parser!(f::table::parse)),
        _ if name.ends_with(".ebd") => ("ebd", parser!(|d| f::ebd::Ebd::parse(d))),
        _ if name.ends_with(".oid") => ("oid", parser!(f::oid::parse)),
        _ if name.ends_with(".opl2") => ("opl2", parser!(f::opl2::parse)),
        _ => return None,
    })
}

pub fn unread(path: &Path, rest: &[&str]) -> Result<()> {
    let (mut gran, mut budget) = (4usize, 60.0f64);
    let mut it = rest.iter();
    while let Some(&flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag}: needs a number"))?;
        match flag {
            "--gran" => gran = value.parse().map_err(|_| format!("{value}: not a number"))?,
            "--budget" => budget = value.parse().map_err(|_| format!("{value}: not a number"))?,
            _ => return Err(format!("{flag}: unknown option").into()),
        }
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let (kind, fingerprint) = parser_for(name, &data).ok_or_else(|| format!("{}: no parser for this file", path.display()))?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let report = find_unused(&data, gran, Duration::from_secs_f64(budget), &*fingerprint);
    std::panic::set_hook(hook);
    for &(a, b) in &report.unused {
        let hex: Vec<String> = data[a..b.min(a + 16)].iter().map(|x| format!("{x:02x}")).collect();
        println!("{a:#x}..{b:#x} ({}) {}", b - a, hex.join(" "));
    }
    let total: usize = report.unused.iter().map(|r| r.1 - r.0).sum();
    let percent = if data.is_empty() { 0.0 } else { total as f64 * 100.0 / data.len() as f64 };
    println!("{}: {} bytes, {total} unused in {} ranges ({percent:.1}%), parser {kind}", path.display(), data.len(), report.unused.len());
    if !report.unrefined.is_empty() {
        let bytes: usize = report.unrefined.iter().map(|r| r.1 - r.0).sum();
        println!("{}: {bytes} bytes used (not refined) in {} ranges; raise --budget", path.display(), report.unrefined.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake format: the parser reads bytes 0..4 (a count) and 8..12 only.
    fn fake(d: &[u8]) -> u64 {
        hash_debug(&(d.get(..4).map(<[u8]>::to_vec), d.get(8..12).map(<[u8]>::to_vec)))
    }

    #[test]
    fn unread_field_is_reported_and_read_field_is_not() {
        let data: Vec<u8> = (0..32).collect();
        let r = find_unused(&data, 4, Duration::from_secs(10), &fake);
        assert_eq!(r.unused, vec![(4, 8), (12, 32)]);
        assert!(r.unrefined.is_empty());
    }

    #[test]
    fn adjacent_ranges_merge() {
        assert_eq!(merge(vec![(8, 12), (0, 4), (4, 8), (20, 24)]), vec![(0, 12), (20, 24)]);
    }

    #[test]
    fn granularity_limits_the_split() {
        let data = vec![0u8; 16];
        let one = |d: &[u8]| u64::from(d[5]);
        let r = find_unused(&data, 4, Duration::from_secs(10), &one);
        assert_eq!(r.unused, vec![(0, 4), (8, 16)]);
        let r = find_unused(&data, 1, Duration::from_secs(10), &one);
        assert_eq!(r.unused, vec![(0, 5), (6, 16)]);
    }

    #[test]
    fn zero_budget_terminates_with_everything_unrefined() {
        let data = vec![0u8; 1 << 12];
        let r = find_unused(&data, 4, Duration::ZERO, &fake);
        assert!(r.unused.is_empty());
        assert_eq!(r.unrefined, vec![(0, 1 << 12)]);
    }

    #[test]
    fn a_panic_counts_as_changed() {
        let data = vec![0u8; 8];
        let f = |d: &[u8]| guarded(|| if d[2] != 0 { panic!("bad") } else { 0 });
        std::panic::set_hook(Box::new(|_| {}));
        let r = find_unused(&data, 4, Duration::from_secs(10), &f);
        let _ = std::panic::take_hook();
        assert_eq!(r.unused, vec![(4, 8)]);
    }
}
