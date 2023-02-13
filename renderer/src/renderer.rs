use std::collections::HashMap;

use super::memory_subsystem::{ Node, NodeKind, Page, };
use super::shaders::Program;
use super::state::{ State, StateKey, };

/// The renderer has the job of rendering any renderable objects to the screen. The renderer also stores data needed by
/// `wgpu`, such as shader programs and render pipelines.
pub struct Renderer {
	adapter: wgpu::Adapter,
	device: wgpu::Device,
	instance: wgpu::Instance,
	queue: wgpu::Queue,
	surface: wgpu::Surface,
	surface_config: wgpu::SurfaceConfiguration,
	swapchain_capabilities: wgpu::SurfaceCapabilities,
	swapchain_format: wgpu::TextureFormat,
	window: winit::window::Window,

	state_to_pipeline: HashMap<StateKey, wgpu::RenderPipeline>,

	test_buffer1: Node,
	test_buffer2: Node,
	test_page: Page,
}

impl Renderer {
	/// Creates a new renderer. Acquires a surface using `winit` and acquires a device using `wgpu`.
	pub async fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Self {
		let window = winit::window::Window::new(&event_loop).unwrap();

		let size = window.inner_size();

		let instance = wgpu::Instance::default();

		let surface = unsafe { instance.create_surface(&window) }.unwrap();

		// get the device adapter
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				compatible_surface: None,
				force_fallback_adapter: false,
				power_preference: wgpu::PowerPreference::HighPerformance,
			})
			.await
			.expect("Failed to find an appropriate adapter");

		// get the device and queue
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					features: wgpu::Features::empty(),
					label: None,
					limits: wgpu::Limits::downlevel_webgl2_defaults()
						.using_resolution(adapter.limits()),
				},
				None
			)
			.await
			.expect("Failed to get device");

		let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = wgpu::TextureFormat::Bgra8UnormSrgb;

		// configure the surface
		let surface_config = wgpu::SurfaceConfiguration {
			alpha_mode: swapchain_capabilities.alpha_modes[0],
			format: swapchain_format,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: vec![],
			width: size.width,
		};

		surface.configure(&device, &surface_config);

		let mut page = Page::new(6 * 4 + 4 * 3 * 4, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, &device);

		// create the renderer container object
		Renderer {
			test_buffer1: page.allocate_node(6 * 4, 4, NodeKind::Buffer).unwrap(),
			test_buffer2: page.allocate_node(4 * 3 * 4, 4, NodeKind::Buffer).unwrap(),
			test_page: page,

			adapter,
			device,
			instance,
			queue,
			surface,
			surface_config,
			swapchain_capabilities,
			swapchain_format,
			window,

			state_to_pipeline: HashMap::new(),
		}
	}

	pub fn tick(&mut self) {
		let frame = self.surface.get_current_texture().expect("Could not acquire next texture");

		let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let triangle: [f32; 6] = [
			0.0, 0.5,
			-0.5, -0.5,
			0.5, -0.5,
		];

		self.queue.write_buffer(
			self.test_page.get_buffer(),
			self.test_buffer1.offset,
			unsafe {
				std::slice::from_raw_parts(
					triangle.as_ptr() as *const u8,
					triangle.len() * 4 as usize,
				)
			}
		);

		let colors: [f32; 4 * 3] = [
			1.0, 0.0, 0.0, 1.0,
			0.0, 1.0, 0.0, 1.0,
			0.0, 0.0, 1.0, 1.0,
		];

		self.queue.write_buffer(
			self.test_page.get_buffer(),
			self.test_buffer2.offset,
			unsafe {
				std::slice::from_raw_parts(
					colors.as_ptr() as *const u8,
					colors.len() * 4 as usize,
				)
			}
		);

		let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: None,
		});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
						store: true,
					},
					resolve_target: None,
					view: &view,
				})],
				depth_stencil_attachment: None,
				label: None,
			});

			render_pass.set_pipeline(
				self.state_to_pipeline.values().next().unwrap()
			);

			render_pass.set_vertex_buffer(0, self.test_page.get_slice(&self.test_buffer1));
			render_pass.set_vertex_buffer(1, self.test_page.get_slice(&self.test_buffer2));

			render_pass.draw(0..3, 0..1);
		}

		self.queue.submit(Some(encoder.finish()));
		frame.present();
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface_config.width = width;
		self.surface_config.height = height;

		self.surface.configure(&self.device, &self.surface_config);
	}

	/// Creates a `wgpu` pipeline based on the current render state.
	pub fn create_pipeline(&mut self, state: &State) -> &wgpu::RenderPipeline {
		let mut program = Program::new(
			vec![state.fragment_shader, state.vertex_shader]
		);

		// create the pipeline
		let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: &program.get_bind_group_layouts(&self.device),
			label: None,
			push_constant_ranges: &[],
		});

		// create the render pipeline
		let render_pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			depth_stencil: None,
			fragment: Some(wgpu::FragmentState {
				entry_point: "main",
				module: &state.fragment_shader.module,
				targets: &[Some(self.swapchain_format.into())],
			}),
			label: None,
			layout: Some(&pipeline_layout),
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
			primitive: wgpu::PrimitiveState {
				conservative: false,
				cull_mode: Some(wgpu::Face::Back),
				front_face: wgpu::FrontFace::Ccw,
				polygon_mode: wgpu::PolygonMode::Fill,
				strip_index_format: Some(wgpu::IndexFormat::Uint16),
				topology: wgpu::PrimitiveTopology::TriangleStrip,
				unclipped_depth: false,
			},
			vertex: wgpu::VertexState {
				buffers: &[
					wgpu::VertexBufferLayout {
						array_stride: 4 * 2,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x2,
							offset: 0,
							shader_location: 0,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout {
						array_stride: 4 * 4,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x4,
							offset: 0,
							shader_location: 1,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
				],
				entry_point: "main",
				module: &state.vertex_shader.module,
			},
		});

		self.state_to_pipeline.insert(state.key(), render_pipeline);

		&self.state_to_pipeline[&state.key()]
	}

	/// Gets the wgpu device used by this renderer.
	pub fn get_device(&self) -> &wgpu::Device {
		&self.device
	}

	/// Gets the wgpu queue used by this renderer.
	pub fn get_queue(&self) -> &wgpu::Queue {
		&self.queue
	}

	/// Gets the window this renderer is rendering to.
	pub fn get_window(&self) -> &winit::window::Window {
		&self.window
	}
}
