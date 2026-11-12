//! GPU timings per stage of the frame (`barrel-view --timings`): timestamps written at the
//! stage boundaries, read back after each frame (the viewer waits for every frame anyway), and
//! the stages' medians printed at the end.

use ash::vk;

use crate::{Gpu, Result};

/// The stages, between consecutive marks.
pub const STAGES: [&str; 4] = ["shadow map", "scene", "glow + AA/upscale", "post"];
const MARKS: u32 = STAGES.len() as u32 + 1;

pub struct Timer {
    pool: vk::QueryPool,
    /// Nanoseconds per tick.
    period: f64,
    /// Each frame's stage times in milliseconds.
    frames: Vec<[f64; STAGES.len()]>,
}

impl Timer {
    pub fn new(gpu: &Gpu) -> Result<Option<Timer>> {
        let props = unsafe { gpu.instance.get_physical_device_properties(gpu.physical) };
        if props.limits.timestamp_compute_and_graphics == 0 {
            eprintln!("timings: the device has no timestamps on its graphics queue");
            return Ok(None);
        }
        let pool = unsafe { gpu.device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(MARKS), None)? };
        Ok(Some(Timer { pool, period: props.limits.timestamp_period as f64, frames: vec![] }))
    }

    /// Starts the frame's marks (mark 0, at the top of the pipe).
    ///
    /// # Safety
    /// `cb` is recording outside a render pass.
    pub unsafe fn begin(&self, d: &ash::Device, cb: vk::CommandBuffer) {
        unsafe {
            d.cmd_reset_query_pool(cb, self.pool, 0, MARKS);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.pool, 0);
        }
    }

    /// Mark `n` (1 to 4): everything recorded before it has finished.
    ///
    /// # Safety
    /// `cb` is recording, after `begin`.
    pub unsafe fn mark(&self, d: &ash::Device, cb: vk::CommandBuffer, n: u32) {
        unsafe { d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.pool, n) };
    }

    /// Reads the finished frame's marks.
    pub fn collect(&mut self, gpu: &Gpu) -> Result<()> {
        let mut ticks = [0u64; MARKS as usize];
        unsafe { gpu.device.get_query_pool_results(self.pool, 0, &mut ticks, vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)? };
        self.frames.push(std::array::from_fn(|k| ticks[k + 1].wrapping_sub(ticks[k]) as f64 * self.period / 1e6));
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
    }

    pub fn destroy(self, gpu: &Gpu) {
        unsafe { gpu.device.destroy_query_pool(self.pool, None) };
    }
}
