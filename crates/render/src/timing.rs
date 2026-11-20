//! GPU timings per stage of the frame (`barrel --timings`): timestamps written at the
//! stage boundaries, read back after each frame (the viewer waits for every frame anyway), and
//! the stages' medians printed at the end.

use ash::vk;

use crate::{Gpu, Result};

/// The stages, between consecutive marks.
pub const STAGES: [&str; 6] = ["shadow map", "occlusion", "scene", "reflect + water", "glow + AA/upscale", "post"];
const MARKS: u32 = STAGES.len() as u32 + 1;

pub struct Timer {
    pool: vk::QueryPool,
    /// Nanoseconds per tick.
    period: f64,
    /// Each frame's stage times in milliseconds.
    frames: Vec<[f64; STAGES.len()]>,
    /// The scene pass's shader runs (pixel and vertex), where the device counts them: overdraw
    /// is the pixel runs over the scene's pixels.
    stats: Option<vk::QueryPool>,
    counts: Vec<[u64; 2]>,
}

impl Timer {
    pub fn new(gpu: &Gpu) -> Result<Option<Timer>> {
        let props = unsafe { gpu.instance.get_physical_device_properties(gpu.physical) };
        if props.limits.timestamp_compute_and_graphics == 0 {
            eprintln!("timings: the device has no timestamps on its graphics queue");
            return Ok(None);
        }
        let pool = unsafe { gpu.device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(MARKS), None)? };
        let stats = if gpu.pipeline_stats {
            let flags = vk::QueryPipelineStatisticFlags::FRAGMENT_SHADER_INVOCATIONS | vk::QueryPipelineStatisticFlags::VERTEX_SHADER_INVOCATIONS;
            Some(unsafe { gpu.device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::PIPELINE_STATISTICS).pipeline_statistics(flags).query_count(1), None)? })
        } else {
            None
        };
        Ok(Some(Timer { pool, period: props.limits.timestamp_period as f64, frames: vec![], stats, counts: vec![] }))
    }

    /// Starts the frame's marks (mark 0, at the top of the pipe).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass.
    pub unsafe fn begin(&self, d: &ash::Device, cb: vk::CommandBuffer) {
        unsafe {
            d.cmd_reset_query_pool(cb, self.pool, 0, MARKS);
            if let Some(stats) = self.stats {
                d.cmd_reset_query_pool(cb, stats, 0, 1);
            }
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.pool, 0);
        }
    }

    /// Mark `n` (1 to 6): everything recorded before it has finished.
    ///
    /// # Safety
    /// `cb` is recording, after `begin`.
    pub unsafe fn mark(&self, d: &ash::Device, cb: vk::CommandBuffer, n: u32) {
        unsafe { d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.pool, n) };
    }

    /// Starts (`true`) or ends counting the scene pass's shader runs.
    ///
    /// # Safety
    /// `cb` is recording outside a render pass, after `begin`; each start is ended once.
    pub unsafe fn count(&self, d: &ash::Device, cb: vk::CommandBuffer, start: bool) {
        if let Some(stats) = self.stats {
            unsafe {
                if start {
                    d.cmd_begin_query(cb, stats, 0, vk::QueryControlFlags::empty());
                } else {
                    d.cmd_end_query(cb, stats, 0);
                }
            }
        }
    }

    /// Reads the finished frame's marks.
    pub fn collect(&mut self, gpu: &Gpu) -> Result<()> {
        let mut ticks = [0u64; MARKS as usize];
        unsafe { gpu.device.get_query_pool_results(self.pool, 0, &mut ticks, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)? };
        self.frames.push(std::array::from_fn(|k| ticks[k + 1].wrapping_sub(ticks[k]) as f64 * self.period / 1e6));
        if let Some(stats) = self.stats {
            // In the order of the flags' bits: vertex runs, then pixel runs.
            // (one query: ash counts the queries by the slice's length)
            let mut c = [[0u64; 2]; 1];
            unsafe { gpu.device.get_query_pool_results(stats, 0, &mut c, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)? };
            self.counts.push([c[0][1], c[0][0]]);
        }
        Ok(())
    }

    /// Prints each stage's median and the total's, leaving out the first frames (pipelines and
    /// caches warming up).
    pub fn report(&self) {
        let frames: Vec<_> = self.frames.iter().skip(3).collect();
        if frames.is_empty() {
            eprintln!("timings: too few frames (more than 3 needed)");
            return;
        }
        let median = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        for (k, name) in STAGES.iter().enumerate() {
            eprintln!("timings: {name:<18} {:7.3} ms", median(frames.iter().map(|f| f[k]).collect()));
        }
        eprintln!("timings: {:<18} {:7.3} ms (median of {} frames)", "total", median(frames.iter().map(|f| f.iter().sum()).collect()), frames.len());
        if self.counts.len() > 3 {
            let mut px: Vec<u64> = self.counts.iter().skip(3).map(|c| c[0]).collect();
            let mut vx: Vec<u64> = self.counts.iter().skip(3).map(|c| c[1]).collect();
            px.sort();
            vx.sort();
            eprintln!("timings: scene runs        {} pixel, {} vertex (medians)", px[px.len() / 2], vx[vx.len() / 2]);
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        unsafe {
            gpu.device.destroy_query_pool(self.pool, None);
            if let Some(stats) = self.stats {
                gpu.device.destroy_query_pool(stats, None);
            }
        }
    }
}

/// The CPU's time per stage of the frame loop (`--timings` too): named marks through the
/// frame, each stage the time since the mark before; a frame counts when it reaches `end`.
#[derive(Default)]
pub struct CpuTimer {
    marks: Vec<(&'static str, std::time::Instant)>,
    /// Each stage's times in milliseconds, in the order first seen.
    stages: Vec<(&'static str, Vec<f64>)>,
    frames: usize,
}

impl CpuTimer {
    pub fn begin(&mut self) {
        self.marks.clear();
        self.marks.push(("", std::time::Instant::now()));
    }

    /// The stage `name` ends here.
    pub fn mark(&mut self, name: &'static str) {
        if !self.marks.is_empty() {
            self.marks.push((name, std::time::Instant::now()));
        }
    }

    /// The frame's last stage ends here.
    pub fn end(&mut self, name: &'static str) {
        self.mark(name);
        if self.marks.len() < 2 {
            return;
        }
        self.frames += 1;
        for w in self.marks.windows(2) {
            let ms = (w[1].1 - w[0].1).as_secs_f64() * 1e3;
            match self.stages.iter_mut().find(|(n, _)| *n == w[1].0) {
                Some((_, v)) => v.push(ms),
                None => self.stages.push((w[1].0, vec![ms])),
            }
        }
        self.marks.clear();
    }

    /// Each stage's median and mean over the frames after the first three (a stage that only
    /// some frames reach: over those).
    pub fn report(&self) {
        if self.frames <= 3 {
            return;
        }
        let mut total = 0.0;
        for (name, v) in self.stages.iter().filter(|(n, _)| !n.is_empty()) {
            let v: Vec<f64> = v.iter().skip(3).copied().collect();
            if v.is_empty() {
                continue;
            }
            let mut s = v.clone();
            s.sort_by(f64::total_cmp);
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            total += mean * v.len() as f64 / (self.frames - 3) as f64;
            eprintln!("cpu: {name:<22} {:7.3} ms median, {mean:7.3} mean", s[s.len() / 2]);
        }
        eprintln!("cpu: {:<22} {total:7.3} ms mean per frame ({} frames)", "total", self.frames - 3);
    }
}
