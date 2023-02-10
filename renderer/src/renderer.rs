use std::collections::HashMap;

use super::shaders::Program;
use super::shaders::ShaderTable;
use super::state::State;

/// The renderer has the job of rendering any renderable objects to the screen. The renderer also stores data needed by
/// `wgpu`, such as shader programs and render pipelines.
pub struct Renderer {
	adapter: wgpu::Adapter,
	device: wgpu::Device,
	instance: wgpu::Instance,
	queue: wgpu::Queue,
	surface: wgpu::Surface,
	swapchain_capabilities: wgpu::SurfaceCapabilities,
	swapchain_format: wgpu::TextureFormat,

	state_to_pipeline: HashMap<State, wgpu::RenderPipeline>,
	shader_table: ShaderTable,
}

impl Renderer {
	/// Creates a new renderer. Acquires a surface using `winit` and acquires a device using `wgpu`.
	pub async fn new() -> Self {
		let event_loop = winit::event_loop::EventLoop::new();
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
    let swapchain_format = swapchain_capabilities.formats[0];

		// configure the surface
		let config = wgpu::SurfaceConfiguration {
			alpha_mode: swapchain_capabilities.alpha_modes[0],
			format: swapchain_format,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: vec![],
			width: size.width,
		};

		surface.configure(&device, &config);

		// create the renderer container object
		let mut renderer = Renderer {
			adapter,
			device,
			instance,
			queue,
			surface,
			swapchain_capabilities,
			swapchain_format,

			state_to_pipeline: HashMap::new(),
			shader_table: ShaderTable::new(),
		};

		// create thet initial render pipeline
		renderer.create_pipeline(&State {});

		return renderer;
	}

	pub fn tick() {

	}

	/// Creates a `wgpu` pipeline based on the current render state.
	fn create_pipeline(&mut self, state: &State) -> &wgpu::RenderPipeline {
		// load the shaders
		if let Err(_) = self.shader_table.load_shader_from_file("texture.vert.spv", &self.device) {
			panic!("Could not load vert shader");
		};

		if let Err(_) = self.shader_table.load_shader_from_file("texture.frag.spv", &self.device) {
			panic!("Could not load frag shader");
		};

		let mut program = Program::new(
			vec![self.shader_table.get_shader("texture.frag.spv"), self.shader_table.get_shader("texture.vert.spv")]
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
				module: &self.shader_table.get_shader("texture.frag.spv").module,
				targets: &[Some(self.swapchain_format.into())],
			}),
			label: None,
			layout: Some(&pipeline_layout),
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
			primitive: wgpu::PrimitiveState::default(),
			vertex: wgpu::VertexState {
				buffers: &[],
				entry_point: "main",
				module: &self.shader_table.get_shader("texture.vert.spv").module,
			},
		});

		self.state_to_pipeline.insert(state.clone(), render_pipeline);

		&self.state_to_pipeline[state]
	}
}
