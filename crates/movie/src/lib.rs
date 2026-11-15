//! Movie playback for the games' `.wmv` files (ASF, `barrel_formats::asf`): a native VC-1
//! Advanced profile video decoder ([`vc1`], [`stream`]) and WMA 9 Pro sound (the LGPL crate
//! `barrel_wma`), decoded on the CPU and played by [`player::Movie`].

pub mod bitplane;
pub mod bits;
pub mod player;
pub mod stream;
pub mod vc1;
pub mod vc1_inter;
pub mod vc1_intra;
pub mod vc1_small;
pub mod vc1_loop;
pub mod vc1_mc;
pub mod vc1_mv;
pub mod vc1_tables;

#[cfg(test)]
mod player_tests {
    /// Plays `BARREL_MOVIE` for `BARREL_SECONDS` (3) on the wall clock, as a window would: how
    /// many pictures were shown, dropped as late, and the sound's length.
    #[test]
    #[ignore]
    fn plays_in_real_time() {
        let Ok(path) = std::env::var("BARREL_MOVIE") else { return };
        let seconds: f64 = std::env::var("BARREL_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
        let t0 = std::time::Instant::now();
        let mut m = crate::player::Movie::open(std::fs::read(path).unwrap()).unwrap();
        let opened = t0.elapsed().as_secs_f64();
        let audio = m.audio.as_ref().map(|a| (a.rate, a.channels.len(), a.channels[0].len()));
        let start = std::time::Instant::now();
        let (mut shown, mut last) = (0usize, -1.0f64);
        while start.elapsed().as_secs_f64() < seconds {
            let clock = start.elapsed().as_secs_f64();
            if let Some(f) = m.frame_at(clock, false) {
                if f.time != last {
                    shown += 1;
                    last = f.time;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        eprintln!("opened in {opened:.2} s (sound {audio:?}), duration {:.2} s; in {seconds} s: {shown} pictures shown, {} dropped, last at {last:.3}, error {:?}", m.duration, m.dropped, m.error);
        assert!(m.error.is_none());
    }
}
