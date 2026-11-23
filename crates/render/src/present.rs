//! Presenting to a window: a Vulkan surface for a winit window (Wayland, X11) and a swapchain
//! the display paces: MAILBOX where offered, with the loop held to the display's refresh, else
//! FIFO (vsync). The engine's frame loop draws into the image `acquire` hands out and gives it
//! back with `present`.

use ash::vk;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::{Gpu, Result};

/// The instance extensions a surface on this display needs.
pub fn instance_extensions(display: RawDisplayHandle) -> Result<Vec<*const std::ffi::c_char>> {
    let platform = match display {
        RawDisplayHandle::Wayland(_) => ash::khr::wayland_surface::NAME.as_ptr(),
        RawDisplayHandle::Xlib(_) => ash::khr::xlib_surface::NAME.as_ptr(),
        RawDisplayHandle::Xcb(_) => ash::khr::xcb_surface::NAME.as_ptr(),
        _ => return Err("only Wayland and X11 windows are supported".into()),
    };
    Ok(vec![ash::khr::surface::NAME.as_ptr(), platform])
}

pub struct Presenter {
    surface_fn: ash::khr::surface::Instance,
    swapchain_fn: ash::khr::swapchain::Device,
    surface: vk::SurfaceKHR,
    pub swapchain: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    /// The display's refresh interval, which `present` holds a MAILBOX swapchain's loop to.
    interval: Option<std::time::Duration>,
    /// Vertical sync (the default) or not, and a frame rate limit's interval (`set_pacing`).
    vsync: bool,
    limit: Option<std::time::Duration>,
    /// The present mode of the swapchain.
    pub mode: vk::PresentModeKHR,
    /// The swapchain's images can be copied from (the blurred screen reads them, `screen_blur`).
    pub readable: bool,
    /// When the next present may go.
    next: std::cell::Cell<std::time::Instant>,
}

impl Presenter {
    /// A surface for the window and a swapchain of `width` × `height` (the window's size);
    /// `refresh_hz` the display's refresh rate (none known: FIFO).
    pub fn new(gpu: &Gpu, display: RawDisplayHandle, window: RawWindowHandle, width: u32, height: u32, refresh_hz: Option<f64>) -> Result<Presenter> {
        let surface = unsafe {
            match (display, window) {
                (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => ash::khr::wayland_surface::Instance::new(&gpu._entry, &gpu.instance)
                    .create_wayland_surface(
                        &vk::WaylandSurfaceCreateInfoKHR::default().display(d.display.as_ptr()).surface(w.surface.as_ptr()),
                        None,
                    )?,
                (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w)) => ash::khr::xlib_surface::Instance::new(&gpu._entry, &gpu.instance)
                    .create_xlib_surface(
                        &vk::XlibSurfaceCreateInfoKHR::default()
                            .dpy(d.display.map_or(std::ptr::null_mut(), |p| p.as_ptr()) as *mut _)
                            .window(w.window),
                        None,
                    )?,
                (RawDisplayHandle::Xcb(d), RawWindowHandle::Xcb(w)) => ash::khr::xcb_surface::Instance::new(&gpu._entry, &gpu.instance)
                    .create_xcb_surface(
                        &vk::XcbSurfaceCreateInfoKHR::default()
                            .connection(d.connection.map_or(std::ptr::null_mut(), |p| p.as_ptr()))
                            .window(w.window.get()),
                        None,
                    )?,
                _ => return Err("unsupported window system".into()),
            }
        };
        let surface_fn = ash::khr::surface::Instance::new(&gpu._entry, &gpu.instance);
        if !unsafe { surface_fn.get_physical_device_surface_support(gpu.physical, gpu.queue_family, surface)? } {
            return Err("the graphics queue cannot present to this window".into());
        }
        let swapchain_fn = ash::khr::swapchain::Device::new(&gpu.instance, &gpu.device);
        let mut p = Presenter {
            surface_fn,
            swapchain_fn,
            surface,
            swapchain: vk::SwapchainKHR::null(),
            format: vk::Format::UNDEFINED,
            extent: vk::Extent2D { width, height },
            images: Vec::new(),
            views: Vec::new(),
            interval: refresh_hz.filter(|hz| *hz >= 20.0).map(|hz| std::time::Duration::from_secs_f64(1.0 / hz)),
            vsync: true,
            limit: None,
            mode: vk::PresentModeKHR::FIFO,
            readable: false,
            next: std::cell::Cell::new(std::time::Instant::now()),
        };
        p.rebuild(gpu, width, height)?;
        Ok(p)
    }

    /// (Re)makes the swapchain at the window's size (`width` × `height` when the surface
    /// leaves it to us), replacing the old one; after a resize, or when it went stale.
    pub fn rebuild(&mut self, gpu: &Gpu, width: u32, height: u32) -> Result<()> {
        let (surface_fn, surface) = (&self.surface_fn, self.surface);
        let caps = unsafe { surface_fn.get_physical_device_surface_capabilities(gpu.physical, surface)? };
        let formats = unsafe { surface_fn.get_physical_device_surface_formats(gpu.physical, surface)? };
        // The same 8-bit sRGB-encoded output as the offscreen frames.
        let chosen = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM)
            .or(formats.first())
            .copied()
            .ok_or("the surface offers no formats")?;
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        let mut count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let modes = unsafe { surface_fn.get_physical_device_surface_present_modes(gpu.physical, surface)? };
        let mode = choose_mode(self.vsync, self.interval.is_some(), &modes);
        let old = self.swapchain;
        let swapchain = unsafe {
            self.swapchain_fn.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(surface)
                    .min_image_count(count)
                    .image_format(chosen.format)
                    .image_color_space(chosen.color_space)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST | (caps.supported_usage_flags & vk::ImageUsageFlags::TRANSFER_SRC))
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(mode)
                    .clipped(true)
                    .old_swapchain(old),
                None,
            )?
        };
        unsafe {
            gpu.device.device_wait_idle()?;
            for v in self.views.drain(..) {
                gpu.device.destroy_image_view(v, None);
            }
            if old != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(old, None);
            }
        }
        let images = unsafe { self.swapchain_fn.get_swapchain_images(swapchain)? };
        let views = images
            .iter()
            .map(|&image| unsafe {
                gpu.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(chosen.format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        self.readable = caps.supported_usage_flags.contains(vk::ImageUsageFlags::TRANSFER_SRC);
        (self.swapchain, self.format, self.extent, self.images, self.views, self.mode) = (swapchain, chosen.format, extent, images, views, mode);
        Ok(())
    }

    /// Sets vertical sync and the frame rate limit. A change of vertical sync remakes the
    /// swapchain in another present mode.
    pub fn set_pacing(&mut self, gpu: &Gpu, pacing: crate::settings::Pacing) -> Result<()> {
        self.limit = pacing.interval();
        if pacing.vsync != self.vsync {
            self.vsync = pacing.vsync;
            let (width, height) = (self.extent.width, self.extent.height);
            self.rebuild(gpu, width, height)?;
        }
        Ok(())
    }

    /// The next image to draw into (`ready` is signalled when it may be written), or `None`
    /// when the swapchain no longer fits the window and must be rebuilt.
    pub fn acquire(&self, ready: vk::Semaphore) -> Result<Option<u32>> {
        match unsafe { self.swapchain_fn.acquire_next_image(self.swapchain, u64::MAX, ready, vk::Fence::null()) } {
            Ok((index, _)) => Ok(Some(index)),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Shows image `index` once `done` is signalled; true when the swapchain should be
    /// rebuilt (stale or no longer a perfect fit).
    pub fn present(&self, gpu: &Gpu, index: u32, done: vk::Semaphore) -> Result<bool> {
        // A MAILBOX loop with vertical sync goes no faster than the display's refresh (FIFO's own
        // pace when it keeps up); the frame rate limit holds any mode to its interval.
        let held = (self.mode == vk::PresentModeKHR::MAILBOX && self.vsync).then_some(self.interval).flatten();
        if let Some(interval) = held.max(self.limit) {
            let now = std::time::Instant::now();
            let at = self.next.get();
            if at > now {
                std::thread::sleep(at - now);
            }
            self.next.set(at.max(now) + interval);
        }
        let (swapchains, indices, waits) = ([self.swapchain], [index], [done]);
        match unsafe {
            self.swapchain_fn.queue_present(
                gpu.queue,
                &vk::PresentInfoKHR::default().wait_semaphores(&waits).swapchains(&swapchains).image_indices(&indices),
            )
        } {
            Ok(suboptimal) => Ok(suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(true),
            Err(e) => Err(e.into()),
        }
    }

    pub fn destroy(self, gpu: &Gpu) {
        unsafe {
            for v in &self.views {
                gpu.device.destroy_image_view(*v, None);
            }
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            self.surface_fn.destroy_surface(self.surface, None);
        }
    }
}

/// The present mode for vertical sync on or off, given the modes the surface offers and whether
/// the display's refresh is known. With vsync: MAILBOX where offered and the refresh is known (a
/// frame that misses a refresh shows at the next one, where FIFO holds it to a whole multiple of
/// the refresh: a 35 ms frame at 60 Hz shown every 50 ms; `present` keeps the loop to the refresh
/// as FIFO did), else FIFO. Without: the newest frame at once, MAILBOX, else IMMEDIATE, else FIFO
/// (always offered).
fn choose_mode(vsync: bool, refresh_known: bool, offered: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    use vk::PresentModeKHR as M;
    let order: &[M] = match (vsync, refresh_known) {
        (true, true) => &[M::MAILBOX],
        (true, false) => &[],
        (false, _) => &[M::MAILBOX, M::IMMEDIATE],
    };
    order.iter().copied().find(|m| offered.contains(m)).unwrap_or(M::FIFO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vk::PresentModeKHR as M;

    #[test]
    fn present_mode_follows_vsync_and_the_offer() {
        let all = [M::FIFO, M::MAILBOX, M::IMMEDIATE];
        assert_eq!(choose_mode(true, true, &all), M::MAILBOX);
        assert_eq!(choose_mode(true, false, &all), M::FIFO);
        assert_eq!(choose_mode(true, true, &[M::FIFO, M::IMMEDIATE]), M::FIFO);
        assert_eq!(choose_mode(false, true, &all), M::MAILBOX);
        assert_eq!(choose_mode(false, false, &[M::FIFO, M::IMMEDIATE]), M::IMMEDIATE);
        assert_eq!(choose_mode(false, true, &[M::FIFO]), M::FIFO);
    }
}
