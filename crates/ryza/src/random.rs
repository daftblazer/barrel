//! The game's random numbers. Every draw of Ryza DX comes from one Mersenne Twister (MT19937,
//! the C++ `std::mt19937`), seeded from the clock when the game starts, through a 15-bit
//! draw and a few wrappers: an integer between two bounds, a float between two bounds and the
//! like. Barrel computes the same numbers, so that a run started from the game's generator
//! state draws what the game draws (`docs/engine.md`, "Ryza DX: the random generator").
//!
//! The wrappers are the game's own formulas, float steps included (`0x1406ae6b0` and its
//! neighbours): a result differs by one in the last place if the order of two operations does.

/// The generator's words and the draws it has made since it was seeded.
const WORDS: usize = 624;
/// The length of the window the game's generator keeps its words in (twice the words: the
/// next block is made into the second half and the first is refilled when the index runs out).
const WINDOW: u32 = 2 * WORDS as u32;
/// `1 / 32767`, the double the float draws scale a 15-bit draw by.
const SCALE: f64 = f64::from_bits(0x3f00_0020_0040_0080);
/// The float added to the upper bound of a float draw (a hair over `1e-4`).
const REACH: f32 = f32::from_bits(0x38d1_b717);

/// A Mersenne Twister, MT19937: the output sequence of the C++ `std::mt19937`.
#[derive(Clone, Debug)]
pub struct Engine {
    words: [u32; WORDS],
    /// The next word to temper (`WORDS` when the block is spent).
    next: usize,
}

impl Engine {
    /// The generator after `seed` (`std::mt19937(seed)`).
    pub fn seeded(seed: u32) -> Engine {
        let mut words = [0u32; WORDS];
        words[0] = seed;
        for i in 1..WORDS {
            words[i] = 1_812_433_253u32.wrapping_mul(words[i - 1] ^ (words[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Engine { words, next: WORDS }
    }

    /// The next 32-bit output.
    pub fn next_u32(&mut self) -> u32 {
        if self.next == WORDS {
            self.twist();
        }
        let mut y = self.words[self.next];
        self.next += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    fn twist(&mut self) {
        for i in 0..WORDS {
            let y = (self.words[i] & 0x8000_0000) | (self.words[(i + 1) % WORDS] & 0x7fff_ffff);
            let mag = if y & 1 == 1 { 0x9908_b0df } else { 0 };
            self.words[i] = self.words[(i + 397) % WORDS] ^ (y >> 1) ^ mag;
        }
        self.next = 0;
    }
}

/// What a draw was asked for, and what it gave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `irand(a, b)`: an integer from `a` to `b`, both included (`0x1406ae5e0`).
    Int(i32, i32),
    /// `frand(a, b)`: a float from `a` to `b` (`0x1406ae340`).
    Float(f32, f32),
    /// `one_to(n)`: an integer from 1 to `n` (`0x1406ae640`, hooked as `r640`).
    OneTo(i32),
    /// `below(n)`: an integer from 0 to `n - 1` (`0x1406ae680`, hooked as `r680`).
    Below(i32),
}

/// One logged draw: the call, the generator's index before it (the game's own window index)
/// and the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    pub call: Call,
    pub index: u32,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
}

impl std::fmt::Display for Draw {
    /// The way the game's hook log writes a call: integers in hex, floats in decimal
    /// (`irand 1 a`, `frand 12 22`), then the index and the value.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.call, self.value) {
            (Call::Int(a, b), Value::Int(v)) => write!(f, "irand {a:x} {b:x} @{:x} -> {v}", self.index),
            (Call::Float(a, b), Value::Float(v)) => write!(f, "frand {a} {b} @{:x} -> {v}", self.index),
            (Call::OneTo(n), Value::Int(v)) => write!(f, "r640 {n:x} @{:x} -> {v}", self.index),
            (Call::Below(n), Value::Int(v)) => write!(f, "r680 {n:x} @{:x} -> {v}", self.index),
            _ => Err(std::fmt::Error),
        }
    }
}

/// The game's random numbers.
#[derive(Clone, Debug)]
pub struct Random {
    engine: Engine,
    /// Draws made since the generator was seeded, to name the game's window index.
    drawn: u64,
    log: Option<Vec<Draw>>,
}

impl Random {
    /// The generator as the game has it after seeding with `seed` (it reseeds from the clock
    /// once at start-up) and `drawn` draws.
    pub fn after(seed: u32, drawn: u64) -> Random {
        let mut r = Random { engine: Engine::seeded(seed), drawn: 0, log: None };
        for _ in 0..drawn {
            r.engine.next_u32();
        }
        r.drawn = drawn;
        r
    }

    /// The generator from a dump of the game's: the window index (a u32), then the window of
    /// 1,248 words and a trailing word. The game fills the second half of its window with the
    /// next block when the index reaches 624 and reads on through it; `None` for a dump too
    /// short or an index out of the window.
    pub fn from_dump(bytes: &[u8]) -> Option<Random> {
        let word = |i: usize| bytes.get(4 * i..4 * i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let index = word(0)?;
        if index >= WINDOW {
            return None;
        }
        // Up to index 624 the current block is the first half (at 624 its twist is due); then
        // the second half holds it, entered at `index - 624`.
        let (from, next) = if index <= WORDS as u32 { (1, index as usize) } else { (1 + WORDS, index as usize - WORDS) };
        let mut words = [0u32; WORDS];
        for (i, w) in words.iter_mut().enumerate() {
            *w = word(from + i)?;
        }
        // Draws since the seeding are not in the dump; the window index is what the game shows.
        Some(Random { engine: Engine { words, next }, drawn: u64::from((index + WORDS as u32) % WINDOW), log: None })
    }

    /// Starts recording the draws.
    pub fn record(&mut self) {
        self.log = Some(Vec::new());
    }

    /// The draws recorded so far.
    pub fn draws(&self) -> &[Draw] {
        self.log.as_deref().unwrap_or(&[])
    }

    /// The game's window index now (`0x141b74600`'s first word): 624 after the seeding.
    pub fn index(&self) -> u32 {
        ((u64::from(WINDOW) / 2 + self.drawn) % u64::from(WINDOW)) as u32
    }

    /// A draw of 15 bits (`0x1406ae6b0`: the engine's output through the range 0 to 0x7fff,
    /// which takes the low 15 bits and never redraws).
    pub fn rand15(&mut self) -> u32 {
        self.drawn += 1;
        self.engine.next_u32() & 0x7fff
    }

    /// The 15-bit draw as a float in 0 to 1 (a double product rounded to float).
    fn unit(&mut self) -> f32 {
        (f64::from(self.rand15()) * SCALE) as f32
    }

    fn note(&mut self, index: u32, call: Call, value: Value) {
        if let Some(log) = &mut self.log {
            log.push(Draw { call, index, value });
        }
    }

    /// An integer from `a` to `b`, in either order, both included (`0x1406ae5e0`); equal
    /// bounds give that number without a draw.
    pub fn irand(&mut self, a: i32, b: i32) -> i32 {
        let (low, high) = (a.min(b), a.max(b));
        let index = self.index();
        let value = if low == high { high } else { low.wrapping_add((self.rand15() as i32) % high.wrapping_sub(low).wrapping_add(1)) };
        if low != high {
            self.note(index, Call::Int(a, b), Value::Int(value));
        }
        value
    }

    /// A float from `a` to `b`, in either order (`0x1406ae340`): `low + draw / 32767 * (high +
    /// 1e-4 - low)` in float; equal bounds give that number without a draw.
    pub fn frand(&mut self, a: f32, b: f32) -> f32 {
        let (low, high) = if a > b { (b, a) } else { (a, b) };
        if low == high {
            return low;
        }
        let index = self.index();
        let u = self.unit();
        let value = u * ((high + REACH) - low) + low;
        self.note(index, Call::Float(a, b), Value::Float(value));
        value
    }

    /// A draw from 1 to `n` (`0x1406ae640`; 0 for `n` 0, without a draw).
    pub fn one_to(&mut self, n: i32) -> i32 {
        if n == 0 {
            return 0;
        }
        let index = self.index();
        let value = (self.rand15() as i32) % n + 1;
        self.note(index, Call::OneTo(n), Value::Int(value));
        value
    }

    /// A draw from 0 to `n - 1` (`0x1406ae680`; 0 for `n` 0, without a draw).
    pub fn below(&mut self, n: i32) -> i32 {
        if n == 0 {
            return 0;
        }
        let index = self.index();
        let value = (self.rand15() as i32) % n;
        self.note(index, Call::Below(n), Value::Int(value));
        value
    }

    /// A chance of `percent` in 100 (`0x1406ae500`): whether a draw mod 100 is below it; 0 or less is never and 100 or
    /// more always, with no draw.
    pub fn chance(&mut self, percent: i32) -> bool {
        match percent {
            ..=0 => false,
            100.. => true,
            _ => (self.rand15() as i32) % 100 < percent,
        }
    }

    /// A float from 0 to 1 (`0x1406ae410`).
    pub fn fraction(&mut self) -> f32 {
        self.unit()
    }

    /// A float from 0 to `a` (`0x1406ae3d0`).
    pub fn scaled(&mut self, a: f32) -> f32 {
        self.unit() * a
    }

    /// A float from `-a` to `a` (`0x1406ae2e0`): `draw * 2a - (a + 1e-4)`.
    pub fn around(&mut self, a: f32) -> f32 {
        let u = self.unit();
        u * (a + a) - (a + REACH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator after the reseed of the gathering capture: seed 1767225600, 7306 draws
    /// (the game's index 0x1ba at frame 2040).
    fn at_2040() -> Random {
        Random::after(1_767_225_600, 7306)
    }

    #[test]
    fn mersenne_twister_matches_the_reference_outputs() {
        // The C++ reference: `std::mt19937 g(5489)`'s first and 10,000th outputs.
        let mut e = Engine::seeded(5489);
        assert_eq!(e.next_u32(), 3_499_211_612);
        for _ in 1..9_999 {
            e.next_u32();
        }
        assert_eq!(e.next_u32(), 4_123_659_995);
    }

    #[test]
    fn scale_and_reach_are_the_games_constants() {
        assert_eq!(SCALE, 1.0 / 32767.0);
        assert_eq!(REACH, 1e-4);
    }

    #[test]
    fn the_index_follows_the_games_window() {
        let r = at_2040();
        assert_eq!(r.index(), 0x1ba);
        assert_eq!(Random::after(1, 0).index(), 624);
        assert_eq!(Random::after(1, 624).index(), 1248 % 1248);
        assert_eq!(Random::after(1, 7306 + 1248).index(), 0x1ba);
    }

    /// The draws after the 2040 state, by hand from the dumped words (the C++ outputs 1038260854,
    /// 3278098524, 626949479, ... masked to 15 bits).
    #[test]
    fn rand15_from_the_2040_state() {
        let mut r = at_2040();
        let got: Vec<u32> = (0..8).map(|_| r.rand15()).collect();
        assert_eq!(got, [6774, 20572, 32103, 27911, 6539, 31786, 4851, 27631]);
        assert_eq!(r.index(), 0x1ba + 8);
    }

    #[test]
    fn the_wrappers_use_the_game_formulas() {
        let mut r = at_2040();
        assert_eq!(r.irand(1, 2), 6774 % 2 + 1);
        assert_eq!(r.irand(99, 0), 20572 % 100);
        let f = r.frand(12.0, 22.0);
        assert_eq!(f, ((32103.0f64 / 32767.0) as f32) * ((22.0f32 + 1e-4) - 12.0) + 12.0);
        assert_eq!(r.irand(7, 7), 7, "equal bounds do not draw");
        assert_eq!(r.index(), 0x1ba + 3);
        assert_eq!(r.one_to(10), 27911 % 10 + 1);
        assert_eq!(r.below(4), 6539 % 4);
        assert_eq!(r.below(0), 0);
        let scaled = r.scaled(8.0);
        assert_eq!(scaled, ((31786.0f64 / 32767.0) as f32) * 8.0);
        let around = r.around(2.0);
        assert_eq!(around, ((4851.0f64 / 32767.0) as f32) * 4.0 - (2.0 + 1e-4));
        assert!(!r.chance(0) && r.chance(100), "no draw at 0 or 100");
        assert_eq!(r.index(), 0x1ba + 7);
        assert_eq!(r.chance(50), 27631 % 100 < 50);
    }

    #[test]
    fn a_dump_resumes_the_sequence() {
        // A dump of the game's window at index 0x1ba: its first half is the current block.
        let mut bytes = vec![0u8; 4 * 1250];
        bytes[..4].copy_from_slice(&0x1bau32.to_le_bytes());
        for (i, w) in at_2040().engine.words.iter().enumerate() {
            bytes[4 + 4 * i..8 + 4 * i].copy_from_slice(&w.to_le_bytes());
        }
        let mut a = Random::from_dump(&bytes).unwrap();
        let mut b = at_2040();
        assert_eq!(a.index(), b.index());
        assert_eq!((a.rand15(), a.rand15()), (b.rand15(), b.rand15()));
        // Past index 624 the block is the second half, entered where the index says.
        bytes[..4].copy_from_slice(&(624u32 + 5).to_le_bytes());
        bytes[4 + 4 * 624..8 + 4 * 624].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(Random::from_dump(&bytes).unwrap().index(), 629);
        assert!(Random::from_dump(&bytes[..100]).is_none());
    }

    #[test]
    fn the_log_names_each_draw() {
        let mut r = at_2040();
        r.record();
        r.irand(1, 2);
        r.irand(0, 99);
        r.frand(12.0, 22.0);
        let lines: Vec<String> = r.draws().iter().map(|d| d.to_string()).collect();
        assert_eq!(lines[0], "irand 1 2 @1ba -> 1");
        assert_eq!(lines[1], "irand 0 63 @1bb -> 72");
        assert!(lines[2].starts_with("frand 12 22 @1bc -> 21.7"), "{}", lines[2]);
    }
}
