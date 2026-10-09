//! Direct Vulkan renderer. A single frame is in flight in this first slice.
//! SAFETY: all Vulkan calls are confined here. The device is idle before resource
//! replacement, mapping or destruction. Command buffers outlive submission; all
//! create-info slice pointers remain valid for the duration of their calls.
use crate::sim::{CELL, Game};
use ash::{Entry, vk};
use glam::{Mat4, Vec3};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}
pub struct Renderer {
    _entry: Entry,
    instance: ash::Instance,
    surface_api: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical: vk::PhysicalDevice,
    pub device: ash::Device,
    queue: vk::Queue,
    swap_api: ash::khr::swapchain::Device,
    swap: vk::SwapchainKHR,
    views: Vec<vk::ImageView>,
    frames: Vec<vk::Framebuffer>,
    depth: vk::Image,
    depth_memory: vk::DeviceMemory,
    depth_view: vk::ImageView,
    pub extent: vk::Extent2D,
    format: vk::Format,
    pass: vk::RenderPass,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    acquire: vk::Semaphore,
    finished: Vec<vk::Semaphore>,
    fence: vk::Fence,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    capacity: usize,
    gui: Option<egui_ash_renderer::Renderer>,
    pending_free: Vec<egui::TextureId>,
    pub adapter: String,
}
impl Renderer {
    pub fn new(window: &Window) -> Result<Self, String> {
        unsafe { Self::init(window).map_err(|e| format!("Vulkan initialization: {e}")) }
    }
    unsafe fn init(window: &Window) -> Result<Self, Box<dyn std::error::Error>> {
        // SAFETY: ownership and synchronization invariants documented at module top.
        unsafe {
            let entry = Entry::load()?;
            let display = window.display_handle()?.as_raw();
            let extensions = ash_window::enumerate_required_extensions(display)?;
            let name = c"Hot Vampires";
            let app = vk::ApplicationInfo::default()
                .application_name(name)
                .api_version(vk::API_VERSION_1_1);
            let available = entry.enumerate_instance_layer_properties()?;
            let validation = c"VK_LAYER_KHRONOS_validation";
            let layers = if std::env::var_os("HV_VALIDATION").is_some()
                && available
                    .iter()
                    .any(|l| std::ffi::CStr::from_ptr(l.layer_name.as_ptr()) == validation)
            {
                vec![validation.as_ptr()]
            } else {
                vec![]
            };
            let instance = entry.create_instance(
                &vk::InstanceCreateInfo::default()
                    .application_info(&app)
                    .enabled_extension_names(extensions)
                    .enabled_layer_names(&layers),
                None,
            )?;
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                display,
                window.window_handle()?.as_raw(),
                None,
            )?;
            let surface_api = ash::khr::surface::Instance::new(&entry, &instance);
            let (physical, family) = instance
                .enumerate_physical_devices()?
                .into_iter()
                .find_map(|p| {
                    instance
                        .get_physical_device_queue_family_properties(p)
                        .iter()
                        .enumerate()
                        .find(|(i, q)| {
                            q.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                && surface_api
                                    .get_physical_device_surface_support(p, *i as u32, surface)
                                    .unwrap_or(false)
                        })
                        .map(|(i, _)| (p, i as u32))
                })
                .ok_or("No Vulkan graphics/present queue")?;
            let props = instance.get_physical_device_properties(physical);
            let adapter = std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let priorities = [1.0];
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&priorities)];
            let dev_extensions = [ash::khr::swapchain::NAME.as_ptr()];
            let device = instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&dev_extensions),
                None,
            )?;
            let queue = device.get_device_queue(family, 0);
            let swap_api = ash::khr::swapchain::Device::new(&instance, &device);
            let formats = surface_api.get_physical_device_surface_formats(physical, surface)?;
            let format = formats
                .iter()
                .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
                .copied()
                .unwrap_or(formats[0]);
            let attachments = [
                vk::AttachmentDescription::default()
                    .format(format.format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
                vk::AttachmentDescription::default()
                    .format(vk::Format::D32_SFLOAT)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
            ];
            let colors = [vk::AttachmentReference::default()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
            let depth_ref = vk::AttachmentReference::default()
                .attachment(1)
                .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
            let subpasses = [vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&colors)
                .depth_stencil_attachment(&depth_ref)];
            let deps = [vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                )
                .dst_stage_mask(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                )
                .dst_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )];
            let pass = device.create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )?;
            let layout =
                device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)?;
            let vs_words = ash::util::read_spv(&mut std::io::Cursor::new(include_bytes!(
                concat!(env!("OUT_DIR"), "/vs.spv")
            )))?;
            let fs_words = ash::util::read_spv(&mut std::io::Cursor::new(include_bytes!(
                concat!(env!("OUT_DIR"), "/fs.spv")
            )))?;
            let vs = device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&vs_words),
                None,
            )?;
            let fs = device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&fs_words),
                None,
            )?;
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vs)
                    .name(c"vs"),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(fs)
                    .name(c"fs"),
            ];
            let bindings = [vk::VertexInputBindingDescription {
                binding: 0,
                stride: 24,
                input_rate: vk::VertexInputRate::VERTEX,
            }];
            let attributes = [
                vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 0,
                },
                vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 12,
                },
            ];
            let vertex = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&bindings)
                .vertex_attribute_descriptions(&attributes);
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewport = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let raster = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0);
            let multi = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS);
            let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
            let blend =
                vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic =
                vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
            let info = [vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&multi)
                .depth_stencil_state(&depth)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic)
                .layout(layout)
                .render_pass(pass)];
            let pipeline = device
                .create_graphics_pipelines(vk::PipelineCache::null(), &info, None)
                .map_err(|(_, e)| e)?[0];
            device.destroy_shader_module(vs, None);
            device.destroy_shader_module(fs, None);
            let pool = device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;
            let command = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            let acquire = device.create_semaphore(&Default::default(), None)?;
            let fence = device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?;
            let gui = egui_ash_renderer::Renderer::with_default_allocator(
                &instance,
                physical,
                device.clone(),
                pass,
                egui_ash_renderer::Options {
                    in_flight_frames: 1,
                    ..Default::default()
                },
            )?;
            let mut r = Self {
                _entry: entry,
                instance,
                surface_api,
                surface,
                physical,
                device,
                queue,
                swap_api,
                swap: vk::SwapchainKHR::null(),
                views: vec![],
                frames: vec![],
                depth: vk::Image::null(),
                depth_memory: vk::DeviceMemory::null(),
                depth_view: vk::ImageView::null(),
                extent: vk::Extent2D::default(),
                format: format.format,
                pass,
                pipeline,
                layout,
                pool,
                command,
                acquire,
                finished: vec![],
                fence,
                buffer: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                capacity: 0,
                gui: Some(gui),
                pending_free: Vec::new(),
                adapter,
            };
            r.resize(window)?;
            Ok(r)
        }
    }
    fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32, vk::Result> {
        let props = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical)
        };
        (0..props.memory_type_count)
            .find(|i| {
                bits & (1 << i) != 0
                    && props.memory_types[*i as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or(vk::Result::ERROR_FEATURE_NOT_PRESENT)
    }
    pub fn resize(&mut self, window: &Window) -> Result<(), vk::Result> {
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        unsafe {
            self.device.device_wait_idle()?;
            self.clear_swap();
            let caps = self
                .surface_api
                .get_physical_device_surface_capabilities(self.physical, self.surface)?;
            self.extent = if caps.current_extent.width != u32::MAX {
                caps.current_extent
            } else {
                vk::Extent2D {
                    width: size
                        .width
                        .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                    height: size
                        .height
                        .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
                }
            };
            let count = (caps.min_image_count + 1).min(if caps.max_image_count == 0 {
                u32::MAX
            } else {
                caps.max_image_count
            });
            let composite = [
                vk::CompositeAlphaFlagsKHR::OPAQUE,
                vk::CompositeAlphaFlagsKHR::INHERIT,
                vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
                vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
            ]
            .into_iter()
            .find(|f| caps.supported_composite_alpha.contains(*f))
            .unwrap();
            self.swap = self.swap_api.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(self.surface)
                    .min_image_count(count)
                    .image_format(self.format)
                    .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                    .image_extent(self.extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(composite)
                    .present_mode(vk::PresentModeKHR::FIFO)
                    .clipped(true),
                None,
            )?;
            let range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1);
            for image in self.swap_api.get_swapchain_images(self.swap)? {
                self.views.push(
                    self.device.create_image_view(
                        &vk::ImageViewCreateInfo::default()
                            .image(image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(self.format)
                            .subresource_range(range),
                        None,
                    )?,
                );
                self.finished
                    .push(self.device.create_semaphore(&Default::default(), None)?);
            }
            self.depth = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::D32_SFLOAT)
                    .extent(vk::Extent3D {
                        width: self.extent.width,
                        height: self.extent.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
                None,
            )?;
            let req = self.device.get_image_memory_requirements(self.depth);
            self.depth_memory = self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(self.memory_type(
                        req.memory_type_bits,
                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    )?),
                None,
            )?;
            self.device
                .bind_image_memory(self.depth, self.depth_memory, 0)?;
            self.depth_view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(self.depth)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::D32_SFLOAT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .level_count(1)
                            .layer_count(1),
                    ),
                None,
            )?;
            for &view in &self.views {
                let attachments = [view, self.depth_view];
                self.frames.push(
                    self.device.create_framebuffer(
                        &vk::FramebufferCreateInfo::default()
                            .render_pass(self.pass)
                            .attachments(&attachments)
                            .width(self.extent.width)
                            .height(self.extent.height)
                            .layers(1),
                        None,
                    )?,
                );
            }
        }
        Ok(())
    }
    unsafe fn clear_swap(&mut self) {
        unsafe {
            for f in self.frames.drain(..) {
                self.device.destroy_framebuffer(f, None);
            }
            for v in self.views.drain(..) {
                self.device.destroy_image_view(v, None);
            }
            for s in self.finished.drain(..) {
                self.device.destroy_semaphore(s, None);
            }
            self.device.destroy_image_view(self.depth_view, None);
            self.device.destroy_image(self.depth, None);
            self.device.free_memory(self.depth_memory, None);
            self.swap_api.destroy_swapchain(self.swap, None);
            self.depth_view = vk::ImageView::null();
            self.depth = vk::Image::null();
            self.depth_memory = vk::DeviceMemory::null();
            self.swap = vk::SwapchainKHR::null();
        }
    }
    pub fn draw(
        &mut self,
        vertices: &[Vertex],
        context: &egui::Context,
        output: egui::FullOutput,
    ) -> Result<bool, String> {
        unsafe {
            self.draw_inner(vertices, context, output)
                .map_err(|e| e.to_string())
        }
    }
    unsafe fn draw_inner(
        &mut self,
        vertices: &[Vertex],
        context: &egui::Context,
        output: egui::FullOutput,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            let (image, suboptimal) = match self.swap_api.acquire_next_image(
                self.swap,
                u64::MAX,
                self.acquire,
                vk::Fence::null(),
            ) {
                Ok(v) => v,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            // Serial frame also retires presentation before texture replacement/free.
            self.device.queue_wait_idle(self.queue)?;
            let bytes = bytemuck::cast_slice::<Vertex, u8>(vertices);
            if bytes.len() > self.capacity {
                self.device.destroy_buffer(self.buffer, None);
                self.device.free_memory(self.memory, None);
                self.capacity = bytes.len().next_power_of_two();
                self.buffer = self.device.create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(self.capacity as u64)
                        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )?;
                let req = self.device.get_buffer_memory_requirements(self.buffer);
                self.memory = self.device.allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(self.memory_type(
                            req.memory_type_bits,
                            vk::MemoryPropertyFlags::HOST_VISIBLE
                                | vk::MemoryPropertyFlags::HOST_COHERENT,
                        )?),
                    None,
                )?;
                self.device
                    .bind_buffer_memory(self.buffer, self.memory, 0)?;
            }
            let mapped = self.device.map_memory(
                self.memory,
                0,
                bytes.len() as u64,
                vk::MemoryMapFlags::empty(),
            )?;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.cast(), bytes.len());
            self.device.unmap_memory(self.memory);
            let gui = self.gui.as_mut().unwrap();
            gui.free_textures(&self.pending_free)?;
            self.pending_free = output.textures_delta.free.clone();
            gui.set_textures(self.queue, self.pool, &output.textures_delta.set)?;
            let primitives = context.tessellate(output.shapes, output.pixels_per_point);
            self.device
                .reset_command_buffer(self.command, vk::CommandBufferResetFlags::empty())?;
            self.device
                .begin_command_buffer(self.command, &vk::CommandBufferBeginInfo::default())?;
            let clear = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.055, 0.085, 0.13, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.extent,
            };
            self.device.cmd_begin_render_pass(
                self.command,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.pass)
                    .framebuffer(self.frames[image as usize])
                    .render_area(area)
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(
                self.command,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
            self.device.cmd_set_viewport(
                self.command,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.extent.width as f32,
                    height: self.extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(self.command, 0, &[area]);
            self.device
                .cmd_bind_vertex_buffers(self.command, 0, &[self.buffer], &[0]);
            self.device
                .cmd_draw(self.command, vertices.len() as u32, 1, 0, 0);
            gui.cmd_draw(
                self.command,
                self.extent,
                output.pixels_per_point,
                &primitives,
            )?;
            self.device.cmd_end_render_pass(self.command);
            self.device.end_command_buffer(self.command)?;
            self.device.reset_fences(&[self.fence])?;
            let waits = [self.acquire];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let commands = [self.command];
            let signals = [self.finished[image as usize]];
            self.device.queue_submit(
                self.queue,
                &[vk::SubmitInfo::default()
                    .wait_semaphores(&waits)
                    .wait_dst_stage_mask(&stages)
                    .command_buffers(&commands)
                    .signal_semaphores(&signals)],
                self.fence,
            )?;
            let swaps = [self.swap];
            let indices = [image];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signals)
                .swapchains(&swaps)
                .image_indices(&indices);
            match self.swap_api.queue_present(self.queue, &present) {
                Ok(v) => Ok(!v && !suboptimal),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(false),
                Err(e) => Err(e.into()),
            }
        }
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.gui.take();
            self.clear_swap();
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_render_pass(self.pass, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_semaphore(self.acquire, None);
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_device(None);
            self.surface_api.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

#[derive(Default)]
pub struct SceneCache {
    ground: std::collections::BTreeMap<(i32, i32), Vec<Vertex>>,
    ground_seed: Option<u64>,
    walls: Vec<(Vec<bool>, Vec<Vertex>)>,
}
impl SceneCache {
    pub fn scene(&mut self, game: &Game, yaw: f32, zoom: f32, aspect: f32) -> Vec<Vertex> {
        let alpha = game.alpha();
        let p = game.prev_player.lerp(game.player, alpha);
        let center = Vec3::new(p.x, crate::terrain::height(game.seed, p), p.y);
        let elevation = 30.0_f32.to_radians();
        let horizontal = 31.0 * elevation.cos();
        let eye = center
            + Vec3::new(
                yaw.sin() * horizontal,
                31.0 * elevation.sin(),
                yaw.cos() * horizontal,
            );
        let view = Mat4::look_at_rh(eye, center, Vec3::Y);
        let projection =
            Mat4::orthographic_rh(-zoom * aspect, zoom * aspect, -zoom, zoom, 0.1, 80.0);
        let matrix = projection * view;
        self.update_ground(game.seed, p, zoom, aspect);
        self.walls
            .resize_with(game.walls.len(), || (Vec::new(), Vec::new()));
        for (index, wall) in game.walls.iter().enumerate() {
            if self.walls[index].0 == wall.cells {
                continue;
            }
            let mut wall_mesh = Vec::new();
            for (i, &solid) in wall.cells.iter().enumerate() {
                if !solid {
                    continue;
                }
                let c = wall.coord(i);
                let mut exposed = [false; 6];
                for axis in 0..3 {
                    for d in [-1isize, 1] {
                        let t = c[axis] as isize + d;
                        if t < 0 || t >= wall.size[axis] as isize {
                            exposed[match (axis, d) {
                                (0, -1) => 2,
                                (0, 1) => 3,
                                (1, -1) => 5,
                                (1, 1) => 4,
                                (2, -1) => 0,
                                _ => 1,
                            }] = true;
                        } else {
                            let mut n = c;
                            n[axis] = t as usize;
                            let j = n[0] + wall.size[0] * (n[1] + wall.size[1] * n[2]);
                            if !wall.cells[j] {
                                exposed[match (axis, d) {
                                    (0, -1) => 2,
                                    (0, 1) => 3,
                                    (1, -1) => 5,
                                    (1, 1) => 4,
                                    (2, -1) => 0,
                                    _ => 1,
                                }] = true;
                            }
                        }
                    }
                }
                if exposed.iter().any(|v| *v) {
                    let color = if c[1] == wall.size[1] - 1 {
                        [0.26, 0.57, 0.36]
                    } else {
                        [0.66, 0.61, 0.47]
                    };
                    add_cube_faces(
                        &mut wall_mesh,
                        Mat4::IDENTITY,
                        wall.center(i),
                        Vec3::splat(CELL),
                        color,
                        exposed,
                    );
                }
            }
            self.walls[index] = (wall.cells.clone(), wall_mesh);
        }
        let mut mesh = Vec::new();
        for ground_mesh in self.ground.values() {
            mesh.extend_from_slice(ground_mesh);
        }
        for (_, wall_mesh) in &self.walls {
            mesh.extend_from_slice(wall_mesh);
        }
        let mut cube = |pos: Vec3, size: Vec3, color: [f32; 3]| {
            add_cube(&mut mesh, Mat4::IDENTITY, pos, size, color)
        };
        let mut bodies = vec![(p, game.facing, [0.12, 0.8, 0.94], 0u32, 0u32)];
        for e in &game.enemies {
            bodies.push((
                e.prev.lerp(e.pos, alpha),
                e.direction.x.atan2(e.direction.y),
                match e.kind {
                    1 => [0.98, 0.48, 0.18],
                    2 => [0.76, 0.32, 0.94],
                    3 => [0.9, 0.14, 0.38],
                    _ => [0.95, 0.28, 0.34],
                },
                e.chipped,
                e.kind,
            ));
        }
        for (pos, facing, color, chipped, kind) in bodies {
            let root = Vec3::new(pos.x, crate::terrain::height(game.seed, pos), pos.y);
            let gait = (game.world * 11.0 + pos.x).sin() * 0.1;
            cube(
                root + Vec3::new(0.0, 0.015, 0.0),
                Vec3::new(0.85, 0.02, 0.65),
                [0.11, 0.2, 0.19],
            );
            let forward = Vec3::new(facing.sin(), 0.0, facing.cos());
            let side = Vec3::new(forward.z, 0.0, -forward.x);
            for sign in [-1.0, 1.0] {
                cube(
                    root + side * 0.17 * sign + Vec3::new(0.0, 0.3 + gait * sign, 0.0),
                    Vec3::new(0.23, 0.5, 0.25),
                    [0.19, 0.23, 0.33],
                );
            }
            cube(root + Vec3::Y * 0.82, Vec3::new(0.58, 0.58, 0.35), color);
            for sign in [-1.0, 1.0] {
                if !(chipped >= 3 && sign > 0.0) {
                    cube(
                        root + side * 0.4 * sign + Vec3::Y * 0.78,
                        Vec3::new(0.18, 0.5, 0.22),
                        color,
                    );
                }
            }
            // The head is a fine cell shell; no internal faces are emitted.
            for y in 0..5 {
                for z in 0..5 {
                    for x in 0..5 {
                        if x > 0 && x < 4 && y > 0 && y < 4 && z > 0 && z < 4 {
                            continue;
                        }
                        if chipped > 0 && x == 4 && y == 4 {
                            continue;
                        }
                        let offset = Vec3::new(
                            (x as f32 - 2.0) * 0.08,
                            1.32 + (y as f32 - 2.0) * 0.08,
                            (z as f32 - 2.0) * 0.08,
                        );
                        cube(
                            root + offset,
                            Vec3::splat(0.078),
                            if kind == 0 {
                                [color[0] * 0.85, color[1] * 0.85, color[2] * 0.85]
                            } else {
                                color
                            },
                        );
                    }
                }
            }
            cube(
                root + Vec3::Y * 1.34 + forward * 0.23,
                Vec3::new(0.28, 0.08, 0.07),
                [0.98, 0.95, 0.63],
            );
        }
        for &pos in &game.loot {
            cube(
                Vec3::new(pos.x, crate::terrain::height(game.seed, pos) + 0.22, pos.y),
                Vec3::splat(0.25),
                [0.98, 0.8, 0.12],
            );
        }
        for s in &game.shots {
            let pos = s.prev.lerp(s.pos, alpha);
            cube(
                Vec3::new(pos.x, crate::terrain::height(game.seed, pos) + 0.85, pos.y),
                if s.hostile {
                    Vec3::splat(0.26)
                } else {
                    Vec3::new(0.12, 0.12, 0.32)
                },
                if s.hostile {
                    [1.0, 0.12, 0.65]
                } else {
                    [0.65, 1.0, 0.94]
                },
            );
        }
        for d in &game.debris {
            cube(d.prev.lerp(d.pos, alpha), Vec3::splat(0.08), d.color);
        }
        if game.level("satellite") > 0 {
            let o = game.orbit();
            cube(
                Vec3::new(o.x, crate::terrain::height(game.seed, o) + 0.8, o.y),
                Vec3::splat(0.45),
                [0.3, 0.92, 0.97],
            );
        }
        for v in &mut mesh {
            let mut p = matrix.transform_point3(Vec3::from_array(v.position));
            p.y = -p.y;
            v.position = p.to_array();
            if game.phase == crate::sim::Phase::Combat {
                let activity = if game.config.min_scale < 1.0 {
                    (game.scale - game.config.min_scale) / (1.0 - game.config.min_scale)
                } else {
                    1.0
                };
                let saturation = 0.08 + 0.92 * activity.clamp(0.0, 1.0);
                let luminance = v.color[0] * 0.2126 + v.color[1] * 0.7152 + v.color[2] * 0.0722;
                v.color = v
                    .color
                    .map(|channel| luminance + (channel - luminance) * saturation);
            }
        }
        mesh
    }
    fn update_ground(&mut self, seed: u64, p: glam::Vec2, zoom: f32, aspect: f32) {
        use crate::terrain::{CHUNK_SIZE, MESH_STEP};
        if self.ground_seed != Some(seed) {
            self.ground.clear();
            self.ground_seed = Some(seed);
        }
        let center = (
            (p.x / CHUNK_SIZE as f32).floor() as i32,
            (p.y / CHUNK_SIZE as f32).floor() as i32,
        );
        // At 30-degree elevation the ground footprint extends twice the vertical zoom.
        // Overscan also covers yaw, slopes, and travel within the center chunk.
        let radius = ((zoom * (aspect.max(1.0) + 2.0) + 16.0) / CHUNK_SIZE as f32).ceil() as i32;
        self.ground
            .retain(|&(x, z), _| (x - center.0).abs() <= radius && (z - center.1).abs() <= radius);
        for z in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                self.ground.entry((x, z)).or_insert_with(|| {
                    let mut mesh = Vec::new();
                    for dz in (0..CHUNK_SIZE).step_by(MESH_STEP as usize) {
                        for dx in (0..CHUNK_SIZE).step_by(MESH_STEP as usize) {
                            let point = |ox: i32, oz: i32| {
                                let p = glam::Vec2::new(
                                    (x * CHUNK_SIZE + dx + ox) as f32,
                                    (z * CHUNK_SIZE + dz + oz) as f32,
                                );
                                Vec3::new(p.x, crate::terrain::vertex_height(seed, p), p.y)
                            };
                            let a = point(0, 0);
                            let b = point(MESH_STEP, 0);
                            let c = point(0, MESH_STEP);
                            let d = point(MESH_STEP, MESH_STEP);
                            for tri in [[a, c, b], [b, c, d]] {
                                let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
                                let light = 0.72
                                    + 0.28
                                        * normal
                                            .dot(Vec3::new(-0.3, 0.9, 0.2).normalize())
                                            .max(0.0);
                                for pos in tri {
                                    let variation = crate::terrain::noise(
                                        seed ^ 0x99887766,
                                        glam::Vec2::new(pos.x, pos.z) / 36.0,
                                    ) * 0.5
                                        + 0.5;
                                    let green = Vec3::new(0.18, 0.38, 0.24);
                                    let dry = Vec3::new(0.4, 0.43, 0.27);
                                    let color = green.lerp(dry, variation * 0.7) * light;
                                    mesh.push(Vertex {
                                        position: pos.to_array(),
                                        color: color.to_array(),
                                    });
                                }
                            }
                        }
                    }
                    mesh
                });
            }
        }
    }
    #[cfg(test)]
    pub fn ground_chunks(&self) -> usize {
        self.ground.len()
    }
}
fn add_cube(mesh: &mut Vec<Vertex>, m: Mat4, pos: Vec3, size: Vec3, color: [f32; 3]) {
    add_cube_faces(mesh, m, pos, size, color, [true; 6]);
}
fn add_cube_faces(
    mesh: &mut Vec<Vertex>,
    m: Mat4,
    pos: Vec3,
    size: Vec3,
    color: [f32; 3],
    exposed: [bool; 6],
) {
    let h = size * 0.5;
    let corners = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    for (f, (face, light)) in [
        ([0, 1, 2, 3], 0.66),
        ([5, 4, 7, 6], 0.83),
        ([4, 0, 3, 7], 0.7),
        ([1, 5, 6, 2], 0.9),
        ([3, 2, 6, 7], 1.0),
        ([4, 5, 1, 0], 0.5),
    ]
    .into_iter()
    .enumerate()
    {
        if !exposed[f] {
            continue;
        }
        for i in [0, 1, 2, 0, 2, 3] {
            let p = m.transform_point3(pos + corners[face[i]]);
            mesh.push(Vertex {
                position: p.to_array(),
                color: color.map(|c| c * light),
            });
        }
    }
}
