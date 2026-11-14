//! Which voice clips Escha & Logy DX's story lines play (the voice packs themselves:
//! `barrel_formats::voice`).

/// The first voice clip of each event's lines in the English executable: an i32 per `EVENT_*`
/// name (1,165, in the enum's order) at this address, -1 for an event without voices; read by
/// `0x140213420 (event)`. A line's clip is its event's first clip plus the line's index.
pub const FIRST_CLIPS_EN: u64 = 0x1408_2d710;
pub const EVENTS: usize = 1165;

/// The events' first voice clips (`None` for an unvoiced event), from the executable.
pub fn first_clips(pe: &barrel_formats::pe::Pe, at: u64) -> Option<Vec<Option<u32>>> {
    let o = pe.offset(at)?;
    let d = pe.data.get(o..o + 4 * EVENTS)?;
    Some(d.chunks_exact(4).map(|c| u32::try_from(i32::from_le_bytes(c.try_into().unwrap())).ok()).collect())
}

/// Where an event's lines most likely start among the clips, guessed without the executable
/// ([`first_clips`] has them; this cross-checks it, docs/formats.md, "Story lines' voices"): a story line's clip is its scene's first clip plus the line's index, and a clip's
/// size follows the line's length, so the first clip is the place where the run of `sizes`
/// (by clip number, 0 for none) correlates best with the lines' `lengths`. Returns the clip
/// and the correlation (Pearson, -1 to 1); `None` for fewer than 3 lines.
pub fn story_start(sizes: &[u32], lengths: &[usize]) -> Option<(usize, f32)> {
    let n = lengths.len();
    if n < 3 || sizes.len() < n {
        return None;
    }
    let lens: Vec<f64> = lengths.iter().map(|&l| l as f64).collect();
    let mean_l = lens.iter().sum::<f64>() / n as f64;
    let dl: Vec<f64> = lens.iter().map(|l| l - mean_l).collect();
    let var_l: f64 = dl.iter().map(|d| d * d).sum();
    if var_l == 0.0 {
        return None;
    }
    let mut best: Option<(usize, f32)> = None;
    for start in 0..=sizes.len() - n {
        let win = &sizes[start..start + n];
        if win.contains(&0) {
            continue;
        }
        let mean_s = win.iter().map(|&s| s as f64).sum::<f64>() / n as f64;
        let (mut cov, mut var_s) = (0.0, 0.0);
        for (d, &s) in dl.iter().zip(win) {
            let ds = s as f64 - mean_s;
            cov += d * ds;
            var_s += ds * ds;
        }
        if var_s == 0.0 {
            continue;
        }
        let r = (cov / (var_l * var_s).sqrt()) as f32;
        if best.is_none_or(|(_, b)| r > b) {
            best = Some((start, r));
        }
    }
    best
}

#[cfg(test)]
mod story_tests {
    #[test]
    fn finds_the_run_that_follows_the_lengths() {
        let mut sizes = vec![500u32; 40];
        let lengths = [10usize, 40, 20, 80, 15];
        for (k, &l) in lengths.iter().enumerate() {
            sizes[17 + k] = 100 * l as u32 + 7;
        }
        let (start, r) = super::story_start(&sizes, &lengths).unwrap();
        assert_eq!(start, 17);
        assert!(r > 0.99);
    }
}
