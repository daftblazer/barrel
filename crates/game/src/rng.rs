//! The random numbers battle and the rest of the game draw, as Escha & Logy DX's executable
//! makes them: one Xorshift128 generator (Marsaglia's, shifts 11, 8 and 19) shared by
//! everything (`docs/tables.md`, "Random numbers").

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    state: [u32; 4],
}

impl Rng {
    /// A generator in the given state `[x, y, z, w]` (for lockstep checks, the state read
    /// from the original game).
    pub fn from_state(state: [u32; 4]) -> Rng {
        Rng { state }
    }

    pub fn state(&self) -> [u32; 4] {
        self.state
    }

    /// The next 32-bit number.
    pub fn next_u32(&mut self) -> u32 {
        let [x, y, z, w] = self.state;
        let t = x ^ (x << 11);
        let next = w ^ (w >> 19) ^ t ^ (t >> 8);
        self.state = [y, z, w, next];
        next
    }

    /// A number from `a` to `b`, both included, in either order; draws nothing when they are
    /// equal. The executable's way: the low 31 bits of the next number, modulo the span.
    pub fn range(&mut self, a: i32, b: i32) -> i32 {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        if lo == hi {
            return lo;
        }
        let span = hi.wrapping_sub(lo).wrapping_add(1);
        let r = (self.next_u32() & 0x7fff_ffff) as i32;
        lo.wrapping_add(r % span)
    }
}

impl Rng {
    /// A number from 0 to `n - 1` (`n` > 0; the executable divides by it unchecked), the
    /// executable's way: the low 31 bits of the next number modulo `n`. Always draws.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0 && n <= i32::MAX as u32, "below: n out of range");
        (self.next_u32() & 0x7fff_ffff) % n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u32; 4] = [123456789, 362436069, 521288629, 88675123];

    #[test]
    fn matches_marsaglias_xorshift128() {
        let mut r = Rng::from_state(SEED);
        assert_eq!(r.next_u32(), 3701687786);
        assert_eq!(r.next_u32(), 458299110);
    }

    #[test]
    fn range_is_inclusive_and_takes_either_order() {
        let mut a = Rng::from_state(SEED);
        let mut b = Rng::from_state(SEED);
        assert_eq!(a.range(54, 60), 54 + (3701687786u32 & 0x7fff_ffff) as i32 % 7);
        assert_eq!(b.range(60, 54), Rng::from_state(SEED).range(54, 60));
        let before = a.state();
        assert_eq!(a.range(50, 50), 50);
        assert_eq!(a.state(), before);
    }

    #[test]
    fn below_draws_once() {
        let mut r = Rng::from_state(SEED);
        assert_eq!(r.below(3), (3701687786u32 & 0x7fff_ffff) % 3);
        assert_eq!(r.next_u32(), 458299110);
    }
}
