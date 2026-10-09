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
    cut: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneParams {
    matrix: [f32; 16],
    player: [f32; 4],
    cut: [f32; 4],
    look: [f32; 4],
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
    shadow_pipeline: vk::Pipeline,
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
    timestamps: vk::QueryPool,
    timestamp_period: f32,
    timestamp_bits: u32,
    timestamp_written: bool,
    pub gpu_ms: Option<f64>,
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
            let timestamp_bits = instance.get_physical_device_queue_family_properties(physical)
                [family as usize]
                .timestamp_valid_bits;
            let timestamps =
                if timestamp_bits > 0 && std::env::args().any(|a| a == "--render-profile") {
                    device.create_query_pool(
                        &vk::QueryPoolCreateInfo::default()
                            .query_type(vk::QueryType::TIMESTAMP)
                            .query_count(2),
                        None,
                    )?
                } else {
                    vk::QueryPool::null()
                };
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
            let push_ranges = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .offset(0)
                .size(std::mem::size_of::<SceneParams>() as u32)];
            let layout = device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push_ranges),
                None,
            )?;
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
                stride: std::mem::size_of::<Vertex>() as u32,
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
                vk::VertexInputAttributeDescription {
                    location: 2,
                    binding: 0,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: 24,
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
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
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
            let shadow_depth = depth.depth_write_enable(false);
            let shadow_info = [info[0].depth_stencil_state(&shadow_depth)];
            let shadow_pipeline = device
                .create_graphics_pipelines(vk::PipelineCache::null(), &shadow_info, None)
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
                shadow_pipeline,
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
                timestamps,
                timestamp_period: props.limits.timestamp_period,
                timestamp_bits,
                timestamp_written: false,
                gpu_ms: None,
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
        params: &SceneParams,
        context: &egui::Context,
        output: egui::FullOutput,
    ) -> Result<bool, String> {
        unsafe {
            self.draw_inner(vertices, params, context, output)
                .map_err(|e| e.to_string())
        }
    }
    unsafe fn draw_inner(
        &mut self,
        vertices: &[Vertex],
        params: &SceneParams,
        context: &egui::Context,
        output: egui::FullOutput,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            if self.timestamp_written {
                let mut values = [0u64; 2];
                self.device.get_query_pool_results(
                    self.timestamps,
                    0,
                    &mut values,
                    vk::QueryResultFlags::TYPE_64,
                )?;
                let mask = if self.timestamp_bits < 64 {
                    (1u64 << self.timestamp_bits) - 1
                } else {
                    u64::MAX
                };
                self.gpu_ms = Some(
                    (values[1].wrapping_sub(values[0]) & mask) as f64
                        * self.timestamp_period as f64
                        / 1_000_000.0,
                );
            }
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
            if self.timestamps != vk::QueryPool::null() {
                self.device
                    .cmd_reset_query_pool(self.command, self.timestamps, 0, 2);
                self.device.cmd_write_timestamp(
                    self.command,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    self.timestamps,
                    0,
                );
            }
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
            self.device.cmd_push_constants(
                self.command,
                self.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::bytes_of(params),
            );
            let opaque = params.look[1] as u32;
            debug_assert!(opaque as usize <= vertices.len());
            self.device.cmd_draw(self.command, opaque, 1, 0, 0);
            if opaque < vertices.len() as u32 {
                self.device.cmd_bind_pipeline(
                    self.command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.shadow_pipeline,
                );
                self.device
                    .cmd_draw(self.command, vertices.len() as u32 - opaque, 1, opaque, 0);
            }
            gui.cmd_draw(
                self.command,
                self.extent,
                output.pixels_per_point,
                &primitives,
            )?;
            self.device.cmd_end_render_pass(self.command);
            if self.timestamps != vk::QueryPool::null() {
                self.device.cmd_write_timestamp(
                    self.command,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    self.timestamps,
                    1,
                );
                self.timestamp_written = true;
            }
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
            self.device.destroy_pipeline(self.shadow_pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_render_pass(self.pass, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_semaphore(self.acquire, None);
            self.device.destroy_fence(self.fence, None);
            if self.timestamps != vk::QueryPool::null() {
                self.device.destroy_query_pool(self.timestamps, None);
            }
            self.device.destroy_device(None);
            self.surface_api.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

#[derive(Default)]
pub struct SceneCache {
    pub params: SceneParams,
    mesh_capacity: usize,
    shadow_capacity: usize,
    ground: std::collections::BTreeMap<(i32, i32), (u64, Vec<Vertex>)>,
    pub camera: crate::camera::Camera,
    pub mesh_rebuilds: u64,
    pub mesh_us: u64,
    pub mesh_peak_us: u64,
    pub scene_peak_us: u64,
    pub scene_last_us: u64,
    shadows: Vec<Option<(Vec3, u64, Vec<Vertex>)>>,
    pub wall_mesh_rebuilds: u64,
    pub wall_mesh_translations: u64,
    ground_seed: Option<u64>,
    walls: Vec<(Vec<bool>, Vec3, Vec<Vertex>, [usize; 3], u8, Vec<u8>)>,
}
impl SceneCache {
    pub fn synchronized(&self, game: &Game) -> bool {
        (0..crate::terrain::CHUNKS()).all(|z| {
            (0..crate::terrain::CHUNKS()).all(|x| {
                self.ground
                    .get(&(
                        x as i32 + (game.terrain.origin.x / 8.0) as i32,
                        z as i32 + (game.terrain.origin.y / 8.0) as i32,
                    ))
                    .is_some_and(|(stamp, _)| *stamp == game.terrain.stamp(x, z))
            })
        })
    }
    #[cfg(test)]
    pub fn scene(&mut self, game: &Game, yaw: f32, zoom: f32, aspect: f32) -> Vec<Vertex> {
        self.scene_frame(game, yaw, zoom, aspect, 1.0 / 60.0)
    }
    pub fn scene_frame(
        &mut self,
        game: &Game,
        yaw: f32,
        zoom: f32,
        aspect: f32,
        real_dt: f32,
    ) -> Vec<Vertex> {
        let scene_start = std::time::Instant::now();
        let alpha = game.alpha();
        let p = game.prev_player.lerp(game.player, alpha);
        let (center, eye) = self.camera.update(game, p, yaw, real_dt);
        let view = Mat4::look_at_rh(eye, center, Vec3::Y);
        let projection =
            Mat4::orthographic_rh(-zoom * aspect, zoom * aspect, -zoom, zoom, 0.05, 128.0);
        let matrix = projection * view;
        self.update_ground(game);
        self.shadows.resize_with(game.walls.len(), || None);
        self.walls.resize_with(game.walls.len(), || {
            (
                Vec::new(),
                Vec3::splat(f32::INFINITY),
                Vec::new(),
                [0; 3],
                0,
                Vec::new(),
            )
        });
        for (index, wall) in game.walls.iter().enumerate() {
            if self.walls[index].0 == wall.cells
                && self.walls[index].3 == wall.size
                && self.walls[index].4 == wall.material
                && self.walls[index].5 == wall.voxel_materials
            {
                let delta = wall.origin - self.walls[index].1;
                if delta != Vec3::ZERO {
                    for vertex in &mut self.walls[index].2 {
                        vertex.position = (Vec3::from_array(vertex.position) + delta).to_array();
                    }
                    self.walls[index].1 = wall.origin;
                    self.wall_mesh_translations += 1;
                }
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
                    let material = wall
                        .voxel_materials
                        .get(i)
                        .copied()
                        .filter(|&m| m != 0)
                        .unwrap_or(wall.material);
                    let color = match material {
                        7 => {
                            if (c[0] / 2 + c[1] / 2 + c[2] / 2) % 3 == 0 {
                                [0.51, 0.49, 0.43]
                            } else {
                                [0.63, 0.59, 0.50]
                            }
                        }
                        5 => {
                            if (c[0] + c[2]) % 2 == 0 {
                                [0.36, 0.52, 0.16]
                            } else {
                                [0.48, 0.60, 0.22]
                            }
                        }
                        6 => {
                            if (c[0] + c[1] + c[2]) % 3 == 0 {
                                [0.24, 0.43, 0.19]
                            } else {
                                [0.34, 0.53, 0.22]
                            }
                        }
                        1 => [0.34, 0.22, 0.12],
                        2 => {
                            if (c[0] + c[1] + c[2]) % 3 == 0 {
                                [0.20, 0.38, 0.16]
                            } else {
                                [0.29, 0.48, 0.20]
                            }
                        }
                        3 => {
                            if (c[0] + c[1] + c[2]) % 4 == 0 {
                                [0.42, 0.45, 0.48]
                            } else {
                                [0.52, 0.54, 0.56]
                            }
                        }
                        _ => {
                            if c[1] == wall.size[1] - 1 {
                                [0.26, 0.57, 0.36]
                            } else {
                                [0.66, 0.61, 0.47]
                            }
                        }
                    };
                    let first = wall_mesh.len();
                    add_cube_faces(
                        &mut wall_mesh,
                        Mat4::IDENTITY,
                        wall.center(i),
                        Vec3::splat(CELL),
                        color,
                        exposed,
                    );
                    shade_voxel_corners(&mut wall_mesh[first..], wall, c, exposed);
                    let tile = match material {
                        1 => 4,
                        2 | 5 | 6 => 5,
                        3 => 3,
                        7 => 6,
                        _ => 7,
                    };
                    texture_faces(
                        &mut wall_mesh[first..],
                        wall.origin,
                        Vec3::splat(CELL),
                        tile,
                    );
                }
            }
            self.walls[index] = (
                wall.cells.clone(),
                wall.origin,
                wall_mesh,
                wall.size,
                wall.material,
                wall.voxel_materials.clone(),
            );
            self.wall_mesh_rebuilds += 1;
            self.shadows[index] = None;
        }
        let mut mesh = Vec::with_capacity(self.mesh_capacity);
        for (&(x, z), (_, ground_mesh)) in &self.ground {
            let min = Vec3::new(x as f32 * 8.0 - 32.0, -8.0, z as f32 * 8.0 - 32.0);
            if visible_bounds(matrix, min, min + Vec3::new(8.0, 16.0, 8.0)) {
                mesh.extend_from_slice(ground_mesh);
            }
        }
        for (index, (_, _, wall_mesh, _, _, _)) in self.walls.iter().enumerate() {
            let wall = &game.walls[index];
            if visible_bounds(
                matrix,
                wall.origin,
                wall.origin + Vec3::from_array(wall.size.map(|n| n as f32 * CELL)),
            ) {
                mesh.extend_from_slice(wall_mesh);
            }
        }
        for v in &mut mesh {
            v.cut[2] = 1.0;
        }
        // Soft vegetation does not require a separate shadow patch.
        // Its face lighting and cached voxel occlusion keep individual tufts readable.
        let mut shadow_mesh = Vec::with_capacity(self.shadow_capacity);
        for (index, wall) in game.walls.iter().enumerate() {
            if wall.is_groundcover() {
                continue;
            }
            if wall.cells.iter().all(|&c| !c) {
                continue;
            }
            let extent = Vec3::from_array(wall.size.map(|n| n as f32 * CELL));
            let ground = game.terrain.height(glam::Vec2::new(
                wall.origin.x + extent.x * 0.5,
                wall.origin.z + extent.z * 0.5,
            ));
            let height = (wall.origin.y + extent.y - ground).max(0.0);
            let center = wall.origin + extent * 0.5 + Vec3::new(height * 0.3, 0.0, -height * 0.2);
            let radii = glam::Vec2::new(
                (extent.x * 0.5 + height * 0.15).clamp(0.3, 2.2),
                (extent.z * 0.5 + height * 0.1).clamp(0.3, 2.2),
            );
            if !visible_bounds(
                matrix,
                center - Vec3::new(radii.x, 16.0, radii.y),
                center + Vec3::new(radii.x, 1.0, radii.y),
            ) {
                continue;
            }
            let stale = self.shadows[index]
                .as_ref()
                .is_none_or(|(origin, stamp, _)| {
                    *origin != wall.origin || *stamp != self.mesh_rebuilds
                });
            if stale {
                let mut shadow = Vec::new();
                soft_shadow(&mut shadow, &game.terrain, center, radii, 0.27);
                self.shadows[index] = Some((wall.origin, self.mesh_rebuilds, shadow));
            }
            shadow_mesh.extend_from_slice(&self.shadows[index].as_ref().unwrap().2);
        }
        let mut actor_details = Vec::new();
        let mut cube = |pos: Vec3, size: Vec3, color: [f32; 3]| {
            add_cube(&mut mesh, Mat4::IDENTITY, pos, size, color)
        };
        let mut bodies = vec![(
            p,
            game.facing,
            [0.12, 0.8, 0.94],
            0u32,
            0u32,
            game.player_body.visual_y,
            game.player_body.grounded,
        )];
        for e in &game.enemies {
            let center = Vec3::new(e.pos.x, e.body.visual_y + 0.8, e.pos.y);
            if !visible_bounds(matrix, center - Vec3::ONE, center + Vec3::ONE) {
                continue;
            }
            // The cutaway belongs to the player; it must not expose covered enemies.
            if self.camera.fade > 0.01 && game.obstruction(game.player, e.pos).is_some() {
                continue;
            }
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
                e.body.visual_y,
                e.body.grounded,
            ));
        }
        for (pos, facing, color, chipped, kind, y, grounded) in bodies {
            let root = Vec3::new(pos.x, y, pos.y);
            soft_shadow(
                &mut shadow_mesh,
                &game.terrain,
                root + Vec3::new(0.35, 0.0, -0.2),
                glam::Vec2::new(0.62, 0.43),
                0.35,
            );
            let forward = Vec3::new(facing.sin(), 0.0, facing.cos());
            let side = Vec3::new(forward.z, 0.0, -forward.x);
            for sign in [-1.0, 1.0] {
                let foot = root + side * 0.17 * sign;
                let sole = if grounded {
                    crate::controller::ground(
                        &game.terrain,
                        &game.walls,
                        glam::Vec2::new(foot.x, foot.z),
                        0.12,
                        root.y + game.physics.step_height,
                    )
                    .min(root.y + 0.4)
                } else {
                    root.y
                };
                let length = (root.y + 0.55 - sole).clamp(0.15, 0.9);
                cube(
                    Vec3::new(foot.x, sole + length * 0.5 + 0.02, foot.z),
                    Vec3::new(0.23, length, 0.25),
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
                        let occupied = |nx: i32, ny: i32, nz: i32| {
                            (0..5).contains(&nx)
                                && (0..5).contains(&ny)
                                && (0..5).contains(&nz)
                                && !(nx > 0 && nx < 4 && ny > 0 && ny < 4 && nz > 0 && nz < 4)
                                && !(chipped > 0 && nx == 4 && ny == 4)
                        };
                        let faces = if chipped == 0 {
                            [z == 0, z == 4, x == 0, x == 4, y == 4, y == 0]
                        } else {
                            [
                                (0, 0, -1),
                                (0, 0, 1),
                                (-1, 0, 0),
                                (1, 0, 0),
                                (0, 1, 0),
                                (0, -1, 0),
                            ]
                            .map(|(dx, dy, dz)| !occupied(x + dx, y + dy, z + dz))
                        };
                        add_cube_faces(
                            &mut actor_details,
                            Mat4::IDENTITY,
                            root + offset,
                            Vec3::splat(0.078),
                            if kind == 0 {
                                [color[0] * 0.85, color[1] * 0.85, color[2] * 0.85]
                            } else {
                                color
                            },
                            faces,
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
                Vec3::new(pos.x, game.terrain.height(pos) + 0.22, pos.y),
                Vec3::splat(0.25),
                [0.98, 0.8, 0.12],
            );
        }
        for s in &game.shots {
            let pos = s.prev.lerp(s.pos, alpha);
            cube(
                Vec3::new(pos.x, s.prev_y + (s.y - s.prev_y) * alpha, pos.y),
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
        for g in &game.grenades {
            cube(
                g.prev.lerp(g.pos, alpha),
                Vec3::splat(0.24),
                [1.0, 0.7, 0.1],
            );
        }
        for d in &game.debris {
            cube(d.prev.lerp(d.pos, alpha), Vec3::splat(0.08), d.color);
        }
        if game.level("satellite") > 0 {
            let o = game.orbit();
            cube(
                Vec3::new(o.x, game.player_body.visual_y + 0.8, o.y),
                Vec3::splat(0.45),
                [0.3, 0.92, 0.97],
            );
        }
        let player_clip =
            matrix.transform_point3(Vec3::new(p.x, game.player_body.visual_y + 0.8, p.y));
        let rx = 2.2 / (zoom * aspect);
        mesh.extend(actor_details);
        let ry = 2.0 / zoom;
        let saturation = if game.phase == crate::sim::Phase::Combat && game.config.min_scale < 1.0 {
            0.08 + 0.92
                * ((game.scale - game.config.min_scale) / (1.0 - game.config.min_scale))
                    .clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.params = SceneParams {
            matrix: matrix.to_cols_array(),
            player: [player_clip.x, player_clip.y, player_clip.z, 0.0],
            cut: [rx, ry, self.camera.fade, 0.0],
            look: [saturation, 0.0, 0.0, 0.0],
        };
        if self.camera.fade > 0.05 {
            // Thin cyan silhouette only; geometry remains opaque outside the local ellipse.
            let mut outline = Vec::new();
            let root = Vec3::new(p.x, game.player_body.visual_y, p.y);
            let outline_matrix = Mat4::from_translation(root) * Mat4::from_rotation_y(yaw);
            for sign in [-1.0, 1.0] {
                add_cube(
                    &mut outline,
                    outline_matrix,
                    Vec3::new(0.36 * sign, 0.8, 0.0),
                    Vec3::new(0.05, 1.5, 0.05),
                    [0.1, 0.9, 1.0],
                );
            }
            for y in [0.06, 1.53] {
                add_cube(
                    &mut outline,
                    outline_matrix,
                    Vec3::Y * y,
                    Vec3::new(0.72, 0.05, 0.1),
                    [0.1, 0.9, 1.0],
                );
            }
            for v in &mut outline {
                let mut q = matrix.transform_point3(Vec3::from_array(v.position));
                q.y = -q.y;
                q.z = 0.0001;
                v.position = q.to_array();
                v.cut = [0.0, 0.0, -2.0, 0.0];
                v.color = v.color.map(|c| c * self.camera.fade);
            }
            mesh.extend(outline);
        }
        self.shadow_capacity = shadow_mesh.capacity();
        self.params.look[1] = mesh.len() as f32;
        mesh.extend(shadow_mesh);
        self.mesh_capacity = mesh.capacity();
        self.scene_last_us = scene_start.elapsed().as_micros() as u64;
        self.scene_peak_us = self.scene_peak_us.max(self.scene_last_us);
        mesh
    }
    fn update_ground(&mut self, game: &Game) {
        use crate::terrain::{BOTTOM, CHUNK_CELLS, CHUNKS, XZ, Y};
        if self.ground_seed != Some(game.seed) {
            self.ground.clear();
            self.ground_seed = Some(game.seed);
        }
        let start = std::time::Instant::now();
        let t = &game.terrain;
        let offset = (t.origin / 8.0).as_ivec2();
        self.ground.retain(|&(x, z), _| {
            x >= offset.x
                && z >= offset.y
                && x < offset.x + CHUNKS() as i32
                && z < offset.y + CHUNKS() as i32
        });
        for cz in 0..CHUNKS() {
            for cx in 0..CHUNKS() {
                let key = (cx as i32 + offset.x, cz as i32 + offset.y);
                let stamp = t.stamp(cx, cz);
                if self.ground.get(&key).is_some_and(|(s, _)| *s == stamp) {
                    continue;
                }
                let mut mesh = Vec::new();
                for z in cz * CHUNK_CELLS..(cz + 1) * CHUNK_CELLS {
                    for x in cx * CHUNK_CELLS..(cx + 1) * CHUNK_CELLS {
                        let p = t.world_center(x, z);
                        let top = t.tops[x + crate::terrain::COLS() * z] as isize;
                        // Merge the exposed vertical voxel strip; surfaces still coincide with every voxel.
                        let h = t.top(x, z);
                        let edge = crate::terrain::Terrain::protected(x, z);
                        let color = if edge || top == 0 {
                            [0.35, 0.4, 0.46]
                        } else if h < crate::terrain::height(game.seed, p) - Y() * 0.5 {
                            [0.48, 0.35, 0.23]
                        } else {
                            [0.26, 0.4, 0.25]
                        };
                        // Merge equal-height top runs without changing the voxel surface.
                        let same_surface = |nx: usize| {
                            t.tops[nx + crate::terrain::COLS() * z] as isize == top
                                && (t.top(nx, z)
                                    < crate::terrain::height(game.seed, t.world_center(nx, z))
                                        - Y() * 0.5)
                                    == (h < crate::terrain::height(game.seed, p) - Y() * 0.5)
                        };
                        if x == cx * CHUNK_CELLS || !same_surface(x - 1) {
                            let mut end = x + 1;
                            while end < (cx + 1) * CHUNK_CELLS && same_surface(end) {
                                end += 1;
                            }
                            let width = (end - x) as f32 * XZ();
                            let first = mesh.len();
                            add_cube_faces(
                                &mut mesh,
                                Mat4::IDENTITY,
                                Vec3::new(p.x + (width - XZ()) * 0.5, h - Y() * 0.5, p.y),
                                Vec3::new(width, Y(), XZ()),
                                color,
                                [false, false, false, false, true, false],
                            );
                            texture_faces(
                                &mut mesh[first..],
                                Vec3::ZERO,
                                Vec3::new(XZ(), Y(), XZ()),
                                if edge || top == 0 {
                                    3
                                } else if color == [0.26, 0.4, 0.25] {
                                    2
                                } else {
                                    1
                                },
                            );
                        }
                        for (dx, dz, face) in [(0, -1, 0), (0, 1, 1), (-1, 0, 2), (1, 0, 3)] {
                            let nx = x as isize + dx;
                            let nz = z as isize + dz;
                            let neighbor = if nx < 0
                                || nz < 0
                                || nx >= crate::terrain::COLS() as isize
                                || nz >= crate::terrain::COLS() as isize
                            {
                                ((t.height(p + glam::Vec2::new(dx as f32, dz as f32) * XZ())
                                    - BOTTOM)
                                    / Y())
                                .round() as isize
                                    - 1
                            } else {
                                t.tops[nx as usize + crate::terrain::COLS() * nz as usize] as isize
                            };
                            if neighbor >= top {
                                continue;
                            }
                            let low = BOTTOM + (neighbor + 1) as f32 * Y();
                            let mut exposed = [false; 6];
                            exposed[face] = true;
                            let first = mesh.len();
                            add_cube_faces(
                                &mut mesh,
                                Mat4::IDENTITY,
                                Vec3::new(p.x, (low + h) * 0.5, p.y),
                                Vec3::new(XZ(), h - low, XZ()),
                                if edge {
                                    [0.34, 0.38, 0.43]
                                } else {
                                    [0.48, 0.35, 0.23]
                                },
                                exposed,
                            );
                            texture_faces(
                                &mut mesh[first..],
                                Vec3::ZERO,
                                Vec3::new(XZ(), Y(), XZ()),
                                if edge { 3 } else { 1 },
                            );
                        }
                    }
                }
                self.ground.insert(key, (stamp, mesh));
                self.mesh_rebuilds += 1;
            }
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.mesh_us += elapsed;
        self.mesh_peak_us = self.mesh_peak_us.max(elapsed);
    }
    #[cfg(test)]
    pub fn ground_chunks(&self) -> usize {
        self.ground.len()
    }
}
fn add_cube(mesh: &mut Vec<Vertex>, m: Mat4, pos: Vec3, size: Vec3, color: [f32; 3]) {
    let first = mesh.len();
    add_cube_faces(mesh, m, pos, size, color, [true; 6]);
    if size.max_element() <= 0.1 {
        for v in &mut mesh[first..] {
            v.cut[3] = 0.0;
        }
    }
}

fn texture_faces(vertices: &mut [Vertex], origin: Vec3, scale: Vec3, tile: u8) {
    for face in vertices.chunks_exact_mut(6) {
        let a = Vec3::from_array(face[0].position);
        let b = Vec3::from_array(face[1].position);
        let c = Vec3::from_array(face[2].position);
        let normal = (b - a).cross(c - a).abs();
        let axes = if normal.y >= normal.x && normal.y >= normal.z {
            [0, 2]
        } else if normal.x >= normal.z {
            [2, 1]
        } else {
            [0, 1]
        };
        for v in face {
            let p = (Vec3::from_array(v.position) - origin) / scale;
            v.cut[0] = p[axes[0]];
            v.cut[1] = p[axes[1]];
            v.cut[3] = tile as f32;
        }
    }
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
    for (f, (face, _)) in [
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
        // Warm directional sunlight and cool hemispheric skylight, baked once for scenery.
        let normal = m
            .transform_vector3([-Vec3::Z, Vec3::Z, -Vec3::X, Vec3::X, Vec3::Y, -Vec3::Y][f])
            .normalize();
        let sun = normal
            .dot(Vec3::new(-0.45, 0.83, 0.32).normalize())
            .max(0.0);
        let sky = 0.82 + 0.18 * normal.y;
        let light = Vec3::new(0.48, 0.53, 0.62) * sky + Vec3::new(0.65, 0.59, 0.46) * sun;
        for i in [0, 1, 2, 0, 2, 3] {
            let p = m.transform_point3(pos + corners[face[i]]);
            mesh.push(Vertex {
                position: p.to_array(),
                color: (Vec3::from_array(color) * light).to_array(),
                cut: [
                    if i == 1 || i == 2 { 1.0 } else { 0.0 },
                    if i >= 2 { 1.0 } else { 0.0 },
                    0.0,
                    8.0,
                ],
            });
        }
    }
}

fn visible_bounds(matrix: Mat4, min: Vec3, max: Vec3) -> bool {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                let p = matrix.transform_point3(Vec3::new(x, y, z));
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    hi.x >= -1.02 && lo.x <= 1.02 && hi.y >= -1.02 && lo.y <= 1.02 && hi.z >= 0.0 && lo.z <= 1.0
}

fn soft_shadow(
    mesh: &mut Vec<Vertex>,
    terrain: &crate::terrain::Terrain,
    center: Vec3,
    radii: glam::Vec2,
    opacity: f32,
) {
    let cell = crate::terrain::XZ();
    let min = ((glam::Vec2::new(center.x, center.z) - radii) / cell)
        .floor()
        .as_ivec2();
    let max = ((glam::Vec2::new(center.x, center.z) + radii) / cell)
        .ceil()
        .as_ivec2();
    for z in min.y..max.y {
        for x in min.x..max.x {
            let p = glam::Vec2::new(x as f32, z as f32) * cell;
            let uv =
                (p + glam::Vec2::splat(cell * 0.5) - glam::Vec2::new(center.x, center.z)) / radii;
            if uv.length_squared() > 1.3 {
                continue;
            }
            let height = terrain.height(p + glam::Vec2::splat(cell * 0.5)) + 0.008;
            for corner in [[0., 0.], [1., 0.], [1., 1.], [0., 0.], [1., 1.], [0., 1.]] {
                let q = p + glam::Vec2::from_array(corner) * cell;
                let uv = (q - glam::Vec2::new(center.x, center.z)) / radii;
                mesh.push(Vertex {
                    position: [q.x, height, q.y],
                    color: [0.08, 0.12, 0.18],
                    cut: [uv.x, uv.y, -1.0, opacity],
                });
            }
        }
    }
}

fn shade_voxel_corners(
    vertices: &mut [Vertex],
    wall: &crate::sim::Wall,
    cell: [usize; 3],
    exposed: [bool; 6],
) {
    let occupied = |p: [i32; 3]| {
        if (0..3).any(|a| p[a] < 0 || p[a] >= wall.size[a] as i32) {
            return false;
        }
        wall.cells[p[0] as usize + wall.size[0] * (p[1] as usize + wall.size[1] * p[2] as usize)]
    };
    let mut start = 0;
    for (face, normal) in [
        [0, 0, -1],
        [0, 0, 1],
        [-1, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
    ]
    .into_iter()
    .enumerate()
    {
        if !exposed[face] {
            continue;
        }
        let axis = normal.iter().position(|&n| n != 0).unwrap();
        let tangents = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        for vertex in &mut vertices[start..start + 6] {
            let mut outside = cell.map(|n| n as i32);
            outside[axis] += normal[axis];
            let mut a = outside;
            let mut b = outside;
            let center = wall
                .center(cell[0] + wall.size[0] * (cell[1] + wall.size[1] * cell[2]))
                .to_array();
            a[tangents[0]] += if vertex.position[tangents[0]] > center[tangents[0]] {
                1
            } else {
                -1
            };
            b[tangents[1]] += if vertex.position[tangents[1]] > center[tangents[1]] {
                1
            } else {
                -1
            };
            let mut corner = a;
            corner[tangents[1]] = b[tangents[1]];
            let sa = occupied(a);
            let sb = occupied(b);
            let occlusion = if sa && sb {
                3
            } else {
                sa as u32 + sb as u32 + occupied(corner) as u32
            };
            vertex.color = vertex.color.map(|c| c * (1.0 - occlusion as f32 * 0.13));
        }
        start += 6;
    }
}

#[cfg(test)]
mod lighting_tests {
    use super::*;

    #[test]
    fn merged_ground_faces_repeat_per_voxel_and_small_particles_have_no_texture() {
        let mut mesh = Vec::new();
        add_cube_faces(
            &mut mesh,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(2.0, 0.125, 0.25),
            [0.3; 3],
            [false, false, false, false, true, false],
        );
        texture_faces(&mut mesh, Vec3::ZERO, Vec3::new(0.25, 0.125, 0.25), 2);
        let min = mesh.iter().map(|v| v.cut[0]).fold(f32::INFINITY, f32::min);
        let max = mesh
            .iter()
            .map(|v| v.cut[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(max - min, 8.0);
        assert!(mesh.iter().all(|v| v.cut[3] == 2.0));
        mesh.clear();
        add_cube(
            &mut mesh,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::splat(0.08),
            [1.0; 3],
        );
        assert!(mesh.iter().all(|v| v.cut[3] == 0.0));
    }

    #[test]
    fn rotating_only_changes_gpu_params_and_keeps_cached_world_vertices() {
        let mut g = Game::new(42, Default::default());
        g.walls = vec![crate::sim::Wall::new(Vec3::new(3.0, 0.0, 3.0), [3, 5, 3])];
        let mut cache = SceneCache::default();
        cache.scene(&g, 0.0, 10.0, 1.6);
        let original: Vec<_> = cache.walls[0]
            .2
            .iter()
            .map(|v| (v.position, v.cut))
            .collect();
        let matrix = cache.params.matrix;
        let rebuilds = cache.wall_mesh_rebuilds;
        cache.scene(&g, 1.2, 10.0, 1.6);
        assert_eq!(cache.wall_mesh_rebuilds, rebuilds);
        assert_ne!(cache.params.matrix, matrix);
        assert_eq!(
            cache.walls[0]
                .2
                .iter()
                .map(|v| (v.position, v.cut))
                .collect::<Vec<_>>(),
            original
        );
    }

    #[test]
    fn visibility_keeps_partial_and_camera_enclosing_bounds() {
        assert!(visible_bounds(
            Mat4::IDENTITY,
            Vec3::new(0.9, -0.2, 0.2),
            Vec3::new(1.5, 0.2, 0.8)
        ));
        assert!(visible_bounds(
            Mat4::IDENTITY,
            Vec3::splat(-10.0),
            Vec3::splat(10.0)
        ));
        assert!(!visible_bounds(
            Mat4::IDENTITY,
            Vec3::new(1.1, 0.0, 0.1),
            Vec3::new(2.0, 1.0, 0.8)
        ));
        assert!(!visible_bounds(
            Mat4::IDENTITY,
            Vec3::new(-0.5, -0.5, 1.1),
            Vec3::new(0.5, 0.5, 2.0)
        ));
    }

    #[test]
    fn voxel_ao_darkens_creases_and_disappears_when_neighbor_is_removed() {
        let mut wall = crate::sim::Wall::new(Vec3::ZERO, [2, 2, 1]);
        wall.cells = vec![true, false, false, true];
        let mut face = Vec::new();
        add_cube_faces(
            &mut face,
            Mat4::IDENTITY,
            wall.center(0),
            Vec3::splat(CELL),
            [1.0; 3],
            [false, false, false, true, false, false],
        );
        let original = face.clone();
        shade_voxel_corners(
            &mut face,
            &wall,
            [0, 0, 0],
            [false, false, false, true, false, false],
        );
        assert!(
            face.iter()
                .zip(&original)
                .any(|(a, b)| a.color[0] < b.color[0])
        );
        wall.cells[3] = false;
        face.clone_from(&original);
        shade_voxel_corners(
            &mut face,
            &wall,
            [0, 0, 0],
            [false, false, false, true, false, false],
        );
        assert!(face.iter().zip(&original).all(|(a, b)| a.color == b.color));
    }

    #[test]
    fn shadow_mesh_follows_excavated_terrain_and_preserves_receiver_depth() {
        let mut game = Game::new(42, Default::default());
        let center = Vec3::ZERO;
        game.terrain.excavate(Vec3::ZERO, 0.6, 1.0);
        let mut shadow = Vec::new();
        soft_shadow(
            &mut shadow,
            &game.terrain,
            center,
            glam::Vec2::splat(1.0),
            0.3,
        );
        assert!(!shadow.is_empty());
        for tile in shadow.chunks_exact(6) {
            let p = glam::Vec2::new(tile[0].position[0], tile[0].position[2])
                + glam::Vec2::splat(crate::terrain::XZ() * 0.5);
            assert!((tile[0].position[1] - game.terrain.height(p) - 0.008).abs() < 0.0001);
            assert!(tile.iter().all(|v| v.cut[2] == -1.0));
        }
    }
}
