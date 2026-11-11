//! Presenting to a window: a Vulkan surface for a winit window (Wayland, X11) and a FIFO
//! swapchain (vsync, so the display paces the frames). The engine's frame loop draws into
//! the image `acquire` hands out and gives it back with `present`.

use ash::vk;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::{Gpu, Result};

/// The instance extensions a surface on this display needs.
pub fn instance_extensions(display: RawDisplayHandle) -> Result<Vec<*const i8>> {
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
}

impl Presenter {
    /// A surface for the window and a swapchain of `width` × `height` (the window's size).
    pub fn new(gpu: &Gpu, display: RawDisplayHandle, window: RawWindowHandle, width: u32, height: u32) -> Result<Presenter> {
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
        let swapchain_fn = ash::khr::swapchain::Device::new(&gpu.instance, &gpu.device);
        let swapchain = unsafe {
            swapchain_fn.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(surface)
                    .min_image_count(count)
                    .image_format(chosen.format)
                    .image_color_space(chosen.color_space)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    // FIFO: vsync, always available; the display paces the loop.
                    .present_mode(vk::PresentModeKHR::FIFO)
                    .clipped(true),
                None,
            )?
        };
        let images = unsafe { swapchain_fn.get_swapchain_images(swapchain)? };
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
        Ok(Presenter { surface_fn, swapchain_fn, surface, swapchain, format: chosen.format, extent, images, views })
    }

    /// The next image to draw into; `ready` is signalled when it may be written.
    pub fn acquire(&self, ready: vk::Semaphore) -> Result<u32> {
        let (index, _) = unsafe { self.swapchain_fn.acquire_next_image(self.swapchain, u64::MAX, ready, vk::Fence::null())? };
        Ok(index)
    }

    /// Shows image `index` once `done` is signalled.
    pub fn present(&self, gpu: &Gpu, index: u32, done: vk::Semaphore) -> Result<()> {
        let (swapchains, indices, waits) = ([self.swapchain], [index], [done]);
        unsafe {
            self.swapchain_fn.queue_present(
                gpu.queue,
                &vk::PresentInfoKHR::default().wait_semaphores(&waits).swapchains(&swapchains).image_indices(&indices),
            )?
        };
        Ok(())
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
