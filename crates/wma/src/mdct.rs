//! The inverse MDCT's middle half through a DCT-IV, computed with a complex FFT of half its
//! length (pre-rotation, radix-2 FFT, post-rotation).

#[derive(Clone, Copy, Default)]
struct C {
    re: f32,
    im: f32,
}

impl C {
    fn mul(self, o: C) -> C {
        C { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
}

/// An `n`-point inverse MDCT half (n coefficients in, n samples out).
pub struct Imdct {
    n: usize,
    pre: Vec<C>,
    post: Vec<C>,
    twiddles: Vec<C>,
    bitrev: Vec<usize>,
}

impl Imdct {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 4);
        let m = n / 2;
        let pi = std::f64::consts::PI;
        let rot = |a: f64| C { re: a.cos() as f32, im: a.sin() as f32 };
        let bits = m.trailing_zeros();
        Imdct {
            n,
            pre: (0..m).map(|k| rot(-pi * (4 * k + 1) as f64 / (4 * n) as f64)).collect(),
            post: (0..m).map(|k| rot(-pi * k as f64 / n as f64)).collect(),
            twiddles: (0..m / 2).map(|k| rot(-2.0 * pi * k as f64 / m as f64)).collect(),
            bitrev: (0..m).map(|i| if bits == 0 { 0 } else { i.reverse_bits() >> (usize::BITS - bits) }).collect(),
        }
    }

    /// DCT-IV: `y[m] = Σ x[k] cos(π/N (m + ½)(k + ½))`.
    pub fn dct4(&self, x: &[f32], y: &mut [f32]) {
        let (n, m) = (self.n, self.n / 2);
        let mut v: Vec<C> = vec![C::default(); m];
        for k in 0..m {
            v[self.bitrev[k]] = C { re: x[2 * k], im: x[n - 1 - 2 * k] }.mul(self.pre[k]);
        }
        let mut len = 2;
        while len <= m {
            let step = m / len;
            for s in (0..m).step_by(len) {
                for j in 0..len / 2 {
                    let w = self.twiddles[j * step];
                    let (a, b) = (v[s + j], v[s + j + len / 2].mul(w));
                    v[s + j] = C { re: a.re + b.re, im: a.im + b.im };
                    v[s + j + len / 2] = C { re: a.re - b.re, im: a.im - b.im };
                }
            }
            len *= 2;
        }
        for k in 0..m {
            let w = v[k].mul(self.post[k]);
            y[2 * k] = w.re;
            y[n - 1 - 2 * k] = -w.im;
        }
    }

    /// The inverse MDCT's middle half with FFmpeg's sign and `scale`:
    /// `y[m] = −scale · Σ x[k] cos(π/N (m + N + ½)(k + ½))`. With cos(a + π(k + ½)) =
    /// −(−1)^k sin a and the DST-IV / DCT-IV mirror, that is `scale · (−1)^m · DCT-IV(z)` for
    /// `z[k] = (−1)^(N−1−k) x[N−1−k]`, the sign folded in.
    pub fn half(&self, x: &[f32], y: &mut [f32], scale: f32) {
        let n = self.n;
        let z: Vec<f32> = (0..n).map(|k| if (n - 1 - k) % 2 == 0 { x[n - 1 - k] } else { -x[n - 1 - k] }).collect();
        self.dct4(&z, y);
        for (m, v) in y.iter_mut().enumerate() {
            *v *= if m % 2 == 0 { scale } else { -scale };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_dct4(x: &[f32]) -> Vec<f64> {
        let n = x.len();
        (0..n).map(|m| x.iter().enumerate().map(|(k, &v)| v as f64 * (std::f64::consts::PI / n as f64 * (m as f64 + 0.5) * (k as f64 + 0.5)).cos()).sum()).collect()
    }

    fn naive_half(x: &[f32]) -> Vec<f64> {
        let n = x.len();
        (0..n).map(|m| -x.iter().enumerate().map(|(k, &v)| v as f64 * (std::f64::consts::PI / n as f64 * (m as f64 + n as f64 + 0.5) * (k as f64 + 0.5)).cos()).sum::<f64>()).collect()
    }

    #[test]
    fn matches_the_direct_formulas() {
        for n in [4usize, 8, 64, 256, 2048] {
            let x: Vec<f32> = (0..n).map(|i| ((i * 7919 % 1000) as f32 / 500.0 - 1.0) * if i % 3 == 0 { 3.0 } else { 1.0 }).collect();
            let t = Imdct::new(n);
            let mut y = vec![0f32; n];
            t.dct4(&x, &mut y);
            let want = naive_dct4(&x);
            let peak = want.iter().fold(0f64, |a, &b| a.max(b.abs()));
            assert!(y.iter().zip(&want).all(|(a, b)| ((*a as f64) - b).abs() < 1e-5 * peak.max(1.0)), "dct4 {n}");
            t.half(&x, &mut y, 1.0);
            let want = naive_half(&x);
            assert!(y.iter().zip(&want).all(|(a, b)| ((*a as f64) - b).abs() < 1e-5 * peak.max(1.0)), "half {n}: {:?} vs {:?}", &y[..4], &want[..4]);
        }
    }
}
