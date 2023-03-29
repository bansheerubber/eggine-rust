use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };
use std::time::Instant;

use crate::Pass;
use crate::memory_subsystem::Memory;
use crate::shaders::ShaderTable;
use crate::state::{ State, StateKey, };

use super::{ DebugContext, WGPUContext, };

/// The boss coordinates the different components needed for rendering (memory management, passes, etc) and glues
/// together their independent logic to generate frames. The boss has executive control over all the components.
#[derive(Debug)]
pub struct Boss<'a> {
	context: Rc<WGPUContext>,
	debug: DebugContext,
	last_rendered_frame: Instant,
	/// Helper object that manages memory. TODO should we implement asynchronous memory on a per-page basis?
	memory: Arc<RwLock<Memory<'a>>>,
	passes: Vec<Box<dyn Pass>>,
	shader_table: Arc<RwLock<ShaderTable>>,
	state_to_pipeline: HashMap<StateKey, wgpu::RenderPipeline>,
	surface_config: wgpu::SurfaceConfiguration,
}

impl<'a> Boss<'a> {
	/// Creates a new renderer. Acquires a surface using `winit` and acquires a device using `wgpu`.
	pub async fn new<'q>(event_loop: &'q winit::event_loop::EventLoop<()>) -> Boss<'a> {
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
					features: adapter.features(),
					label: None,
					limits: wgpu::Limits::default()
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

		let context = Rc::new(WGPUContext {
			adapter,
			device,
			instance,
			queue,
			surface,
			swapchain_capabilities,
			swapchain_format,
			window,
		});

		// create the renderer container object
		Boss {
			memory: Arc::new(RwLock::new(
				Memory::new(context.clone())
			)),
			shader_table: Arc::new(RwLock::new(ShaderTable::new(context.clone()))),

			context,
			debug: DebugContext::default(),
			last_rendered_frame: Instant::now(),
			passes: Vec::new(),
			state_to_pipeline: HashMap::new(),
			surface_config,
		}
	}

	/// Executes render passes and presents the newly created frame. Order of operations:
	/// #1. Get handle for framebuffer
	/// #2. Complete queued buffer writes via `Memory`
	/// #3. Encode render pass commands
	/// #4. Submit command buffer to queue
	/// #5. Present frame
	pub fn tick(&mut self) {
		let frametime = Instant::now() - self.last_rendered_frame;
		self.last_rendered_frame = Instant::now();
		let deltatime = frametime.as_secs_f64();

		self.debug.begin_tick(deltatime, frametime);

		// prepare framebuffer
		let frame = self.context.surface.get_current_texture().expect("Could not acquire next texture");
		let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

		// figure out the pipelines needed for the `Pass`s
		let mut states = Vec::new();

		// steal passes for a second
		let mut passes = std::mem::take(&mut self.passes);
		{
			for pass in passes.iter() {
				let pass_states = pass.states();
				states.push(pass_states);
			}

			// create pipelines
			for pass_states in states.iter() {
				for state in pass_states.iter() {
					self.create_pipeline(state);
				}
			}

			// write data into buffers
			{
				// initialize memory command buffer
				let mut memory_encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
					label: Some("memory encoder"),
				});

				let mut memory = self.memory.write().unwrap();
				memory.complete_write_buffers(&mut memory_encoder);
				self.context.queue.submit(Some(memory_encoder.finish()));
				memory.recall();
			}

			// initialize command buffer
			let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("draw call encoder"),
			});

			// encode passes
			for pass in passes.iter_mut() {
				let states = pass.states();
				let pass_pipelines = states.iter()
					.map(|x| {
						self.get_pipeline(x).unwrap()
					})
					.collect::<Vec<&wgpu::RenderPipeline>>();

				pass.encode(deltatime, &mut encoder, &pass_pipelines, &view);
			}

			self.context.queue.submit(Some(encoder.finish()));
		}
		self.passes = passes; // give ownership of passes back to boss

		frame.present();

		self.debug.end_tick();
	}

	/// Resizes the surface to the supplied width and height.
	pub fn resize(&mut self, width: u32, height: u32) {
		self.surface_config.width = width;
		self.surface_config.height = height;

		self.context.surface.configure(&self.context.device, &self.surface_config);

		// resize passes
		for pass in self.passes.iter_mut() {
			pass.resize(&self.surface_config);
		}
	}

	/// Returns the current size of the window.
	pub fn get_window_size(&self) -> (u32, u32) {
		(self.surface_config.width, self.surface_config.height)
	}

	/// Creates a `wgpu` pipeline based on the current render state.
	pub fn create_pipeline(&mut self, state: &State) {
		// check cache before creating new pipeline
		if self.state_to_pipeline.contains_key(&state.key()) {
			return;
		}

		// create the pipeline
		let pipeline_layout = self.context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: &state.program.get_bind_group_layouts(),
			label: None,
			push_constant_ranges: &[],
		});

		// create the render pipeline
		let render_pipeline = self.context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			depth_stencil: state.depth_stencil.clone(),
			fragment: Some(wgpu::FragmentState {
				entry_point: "main",
				module: &state.program.fragment_shader.module,
				targets: &state.render_targets,
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
				strip_index_format: None,
				topology: wgpu::PrimitiveTopology::TriangleList,
				unclipped_depth: false,
			},
			vertex: wgpu::VertexState {
				buffers: state.vertex_attributes,
				entry_point: "main",
				module: &state.program.vertex_shader.module,
			},
		});

		self.state_to_pipeline.insert(state.key(), render_pipeline); // cache the pipeline
	}

	pub fn get_pipeline(&self, state: &State) -> Option<&wgpu::RenderPipeline> {
		self.state_to_pipeline.get(&state.key())
	}

	/// Sets the ordering of the `Pass`s that are handled each frame.
	pub fn set_passes(&mut self, passes: Vec<Box<dyn Pass>>) {
		self.passes = passes;
	}

	/// Gets the `WGPUContext` owned by the boss.
	pub fn get_context(&self) -> Rc<WGPUContext> {
		self.context.clone()
	}

	/// Gets the `Memory` owned by the boss.
	pub fn get_memory(&self) -> Arc<RwLock<Memory<'a>>> {
		self.memory.clone()
	}

	/// Gets the `ShaderTable` owned by the boss.
	pub fn get_shader_table(&self) -> Arc<RwLock<ShaderTable>> {
		self.shader_table.clone()
	}

	/// Gets the `wgpu::SurfaceConfiguation` owned by the boss.
	pub fn get_surface_config(&self) -> &wgpu::SurfaceConfiguration {
		&self.surface_config
	}
}
