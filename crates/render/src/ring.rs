//! Per-frame buffers for frames in flight: one copy (slot) per frame the GPU may still be
//! drawing, kept mapped. The CPU writes the frame being prepared into slot `Gpu::slot` while
//! the GPU reads the frame before from the other; the window loop waits for a slot's last frame
//! before preparing the next frame in it (`docs/renderer.md`, "Frames in flight").

use ash::vk;

use crate::{Gpu, Result};

/// Frames prepared and drawn at once: the CPU one frame ahead of the GPU.
pub const SLOTS: usize = 2;

pub struct Ring {
    pub buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    ptr: *mut u8,
    /// One slot's size, as asked for.
    pub size: u64,
    /// The distance between slots (the size rounded up to the device's offset alignment).
    pub stride: u64,
}

impl Ring {
    pub fn new(gpu: &Gpu, size: u64, usage: vk::BufferUsageFlags) -> Result<Ring> {
        let size = size.max(16);
        let stride = size.div_ceil(gpu.offset_alignment) * gpu.offset_alignment;
        let (buffer, memory) = gpu.buffer(stride * SLOTS as u64, usage, true)?;
        let ptr = unsafe { gpu.device.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())? as *mut u8 };
        Ok(Ring { buffer, memory, ptr, size, stride })
    }

    /// Slot `slot`'s offset in the buffer.
    pub fn offset(&self, slot: usize) -> u64 {
        slot as u64 * self.stride
    }

    /// Writes `bytes` at `at` in slot `slot` (cut to the slot's size).
    pub fn write_slot(&self, slot: usize, at: u64, bytes: &[u8]) {
        let n = (bytes.len() as u64).min(self.size.saturating_sub(at)) as usize;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.add((self.offset(slot) + at) as usize), n) };
    }

    /// Writes `bytes` at `at` in the slot of the frame being prepared.
    pub fn write(&self, gpu: &Gpu, at: u64, bytes: &[u8]) {
        self.write_slot(gpu.slot(), at, bytes);
    }

    /// Writes `bytes` at `at` in every slot (a starting value).
    pub fn write_all(&self, at: u64, bytes: &[u8]) {
        for slot in 0..SLOTS {
            self.write_slot(slot, at, bytes);
        }
    }

    /// Slot `slot` as a descriptor's buffer range.
    pub fn info(&self, slot: usize) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo::default().buffer(self.buffer).offset(self.offset(slot)).range(self.size)
    }

    pub fn destroy(self, gpu: &Gpu) {
        unsafe {
            gpu.device.unmap_memory(self.memory);
            gpu.device.destroy_buffer(self.buffer, None);
            gpu.device.free_memory(self.memory, None);
        }
    }
}

/// The window loop's frames in flight: per slot the semaphore its swapchain image is acquired
/// with, the fence its submission signals and its command buffer; per swapchain image the
/// semaphore its present waits for (an image's is free again once it is acquired anew).
pub struct Frames {
    acquired: [vk::Semaphore; SLOTS],
    fences: [vk::Fence; SLOTS],
    buffers: [Option<vk::CommandBuffer>; SLOTS],
    drawn: Vec<vk::Semaphore>,
}

impl Frames {
    pub fn new(gpu: &Gpu) -> Result<Frames> {
        let d = &gpu.device;
        let mut acquired = [vk::Semaphore::null(); SLOTS];
        let mut fences = [vk::Fence::null(); SLOTS];
        for k in 0..SLOTS {
            unsafe {
                acquired[k] = d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
                fences[k] = d.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)?;
            }
        }
        Ok(Frames { acquired, fences, buffers: [None; SLOTS], drawn: Vec::new() })
    }

    /// Waits for the last frame prepared in the slot of the frame now being prepared (two
    /// frames back), so its per-frame data and command buffer may be reused.
    pub fn wait_slot(&mut self, gpu: &Gpu, pool: vk::CommandPool) -> Result<()> {
        let k = gpu.slot();
        unsafe {
            gpu.device.wait_for_fences(&[self.fences[k]], true, u64::MAX)?;
            if let Some(cb) = self.buffers[k].take() {
                gpu.device.free_command_buffers(pool, &[cb]);
            }
        }
        Ok(())
    }

    /// The semaphore the frame being prepared acquires its image with.
    pub fn acquired(&self, gpu: &Gpu) -> vk::Semaphore {
        self.acquired[gpu.slot()]
    }

    /// Submits the frame being prepared (`cb`, drawing into swapchain image `image`) and returns
    /// the semaphore its present waits for. `images` is the swapchain's image count.
    pub fn submit(&mut self, gpu: &Gpu, cb: vk::CommandBuffer, image: usize, images: usize) -> Result<vk::Semaphore> {
        let d = &gpu.device;
        while self.drawn.len() < images.max(image + 1) {
            self.drawn.push(unsafe { d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? });
        }
        let k = gpu.slot();
        let (waits, stages, cbs, signals) = ([self.acquired[k]], [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT], [cb], [self.drawn[image]]);
        unsafe {
            d.reset_fences(&[self.fences[k]])?;
            d.queue_submit(
                gpu.queue,
                &[vk::SubmitInfo::default().wait_semaphores(&waits).wait_dst_stage_mask(&stages).command_buffers(&cbs).signal_semaphores(&signals)],
                self.fences[k],
            )?;
        }
        self.buffers[k] = Some(cb);
        Ok(self.drawn[image])
    }

    /// Submits the frame being prepared offscreen (no swapchain image to wait for or present).
    pub fn submit_offscreen(&mut self, gpu: &Gpu, cb: vk::CommandBuffer) -> Result<()> {
        let k = gpu.slot();
        let cbs = [cb];
        unsafe {
            gpu.device.reset_fences(&[self.fences[k]])?;
            gpu.device.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&cbs)], self.fences[k])?;
        }
        self.buffers[k] = Some(cb);
        Ok(())
    }

    /// Waits for the frame just submitted (one frame in flight: `--timings` reads its
    /// timestamps right after).
    pub fn wait_submitted(&mut self, gpu: &Gpu) -> Result<()> {
        unsafe { gpu.device.wait_for_fences(&[self.fences[gpu.slot()]], true, u64::MAX)? };
        Ok(())
    }

    /// Waits for every frame in flight, then frees all (the device must be idle or about to be).
    pub fn destroy(self, gpu: &Gpu, pool: vk::CommandPool) -> Result<()> {
        let d = &gpu.device;
        unsafe {
            d.wait_for_fences(&self.fences, true, u64::MAX)?;
            for cb in self.buffers.into_iter().flatten() {
                d.free_command_buffers(pool, &[cb]);
            }
            for s in self.acquired.into_iter().chain(self.drawn) {
                d.destroy_semaphore(s, None);
            }
            for f in self.fences {
                d.destroy_fence(f, None);
            }
        }
        Ok(())
    }
}
