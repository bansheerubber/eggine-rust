use carton::Carton;
use std::cell::RefCell;
use std::rc::Rc;

use crate::Pass;
use crate::boss::{ Boss, WGPUContext, };
use crate::shaders::Program;
use crate::state::{ ComputeState, RenderState, };
use crate::testing::indirect_pass::DepthPyramidTexture;

/// Visualizes various steps in a depth pyramid.
#[derive(Debug)]
pub struct DepthVisualizer<'a> {
	/// Stores the bind group.
	bind_group: Option<wgpu::BindGroup>,
	/// Stores `wgpu` created objects required for basic rendering operations.
	context: Rc<WGPUContext>,
	/// Reference to the depth pyramid we're visualizing.
	depth_pyramid: Option<Rc<RefCell<Vec<DepthPyramidTexture<'a>>>>>,
	/// The texture in thet depth pyramid we're currently visualizing.
	depth_pyramid_index: usize,
	/// Whether the pass can render.
	enabled: bool,
	/// The custom `PipelineLayout` we need for push constants.
	pipeline_layout: wgpu::PipelineLayout,
	/// Reference to the memory subsystem, used to allocate/write data.
	/// Program used during rendering.
	program: Rc<Program>,
	/// Used to increment `depth_pyramid_index`
	timer: f64,
}

impl<'a> DepthVisualizer<'a> {
	pub fn new<'q>(boss: &mut Boss<'q>, carton: &mut Carton) -> Box<DepthVisualizer<'q>> {
		let context = boss.get_context().clone();

		// create the visualizer program
		let program = {
			// define shader names
			let fragment_shader = "data/depth-visualizer.frag.spv".to_string();
			let vertex_shader = "data/depth-visualizer.vert.spv".to_string();

			// lock shader table
			let shader_table = boss.get_shader_table();
			let mut shader_table = shader_table.write().unwrap();

			// load from carton
			let fragment_shader = shader_table.load_shader_from_carton(&fragment_shader, carton).unwrap();
			let vertex_shader = shader_table.load_shader_from_carton(&vertex_shader, carton).unwrap();

			// create the program
			shader_table.create_render_program("depth-visualizer-shader", fragment_shader, vertex_shader)
		};

		let bind_group_layout
			= context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[
					wgpu::BindGroupLayoutEntry {
						binding: 0,
						count: None,
						ty: wgpu::BindingType::Texture {
							multisampled: false,
							sample_type: wgpu::TextureSampleType::Float { filterable: false, },
							view_dimension: wgpu::TextureViewDimension::D2,
						},
						visibility: wgpu::ShaderStages::FRAGMENT,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 1,
						count: None,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
					},
				],
				label: Some("depth-visualizer-bind-group-layout"),
			});

		let pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			bind_group_layouts: &[&bind_group_layout],
			label: Some("depth-visualizer-pipeline-layout"),
			push_constant_ranges: &[wgpu::PushConstantRange {
				range: 0..16,
				stages: wgpu::ShaderStages::FRAGMENT,
			}],
		});

		Box::new(
			DepthVisualizer {
				bind_group: None,
				context,
				depth_pyramid: None,
				depth_pyramid_index: 0,
				enabled: true,
				pipeline_layout,
				program,
				timer: 0.0,
			}
		)
	}

	pub fn set_depth_pyramid(&mut self, depth_pyramid: Option<Rc<RefCell<Vec<DepthPyramidTexture<'a>>>>>) {
		self.depth_pyramid = depth_pyramid;
		self.depth_pyramid_index = 0;
	}
}

/// Pass implementation. Indirectly render all shapes we have ownership over.
impl Pass for DepthVisualizer<'_> {
	fn render_states<'a>(&'a self) -> Vec<RenderState<'a>> {
		vec![
			RenderState { // state for the composite stage
				depth_stencil: None,
				label: "depth-visualizer".to_string(),
				layout: Some(&self.pipeline_layout),
				program: &self.program,
				render_targets: vec![Some(wgpu::ColorTargetState {
					blend: None,
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				vertex_attributes: &[],
			},
		]
	}

	fn compute_states<'a>(&'a self) -> Vec<ComputeState<'a>> {
		Vec::new()
	}

	/// Encode all draw commands.
	fn encode(
		&mut self,
		deltatime: f64,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		compute_pipelines: &Vec<&wgpu::ComputePipeline>,
		view: &wgpu::TextureView
	) {
		if self.depth_pyramid.is_none() || self.depth_pyramid.as_ref().unwrap().borrow().len() == 0 {
			self.bind_group = None;
			return;
		}

		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("depth-visualizer-encoder"),
		});

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				color_attachments: &[
					Some(wgpu::RenderPassColorAttachment {
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Load,
							store: true,
						},
						resolve_target: None,
						view: &view,
					}),
				],
				depth_stencil_attachment: None,
				label: Some("depth-visualizer-pass"),
			});

			render_pass.set_pipeline(&render_pipelines[0]);

			let clipping = glam::Vec2::new(1.0, 1000.0);
			render_pass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, bytemuck::cast_slice(&[clipping]));

			// bind uniforms
			render_pass.set_bind_group(0, &self.bind_group.as_ref().unwrap(), &[]);

			render_pass.draw(0..3, 0..1);
		}

		self.context.queue.submit(Some(encoder.finish()));

		self.timer += deltatime;
		if self.timer > 1.0 { // increment `depth_pyramid_index` every second
			self.depth_pyramid_index += 1;
			let length = self.depth_pyramid.as_ref().unwrap().borrow().len();
			if self.depth_pyramid_index >= length  {
				self.depth_pyramid_index = 0;
			}

			self.timer = 0.0;

			self.create_bind_groups(render_pipelines, compute_pipelines);

			println!("Depth pyramid step {}", self.depth_pyramid_index);
		}
	}

	/// Handle a window resize.
	fn resize(&mut self, _config: &wgpu::SurfaceConfiguration) {

	}

	/// Gets the memory usage of the pass' render textures.
	fn get_render_texture_usage(&self) -> u64 {
		0
	}

	/// Recreate bind groups.
	fn create_bind_groups(
		&mut self,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		_compute_pipelines: &Vec<&wgpu::ComputePipeline>
	) {
		if self.depth_pyramid.is_none() || self.depth_pyramid.as_ref().unwrap().borrow().len() == 0 {
			self.bind_group = None;
			return;
		}

		let depth_test_sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let index = self.depth_pyramid_index;
		let depth_pyramid = self.depth_pyramid.as_ref().unwrap().borrow();

		let bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&depth_pyramid[index].view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&depth_test_sampler),
				},
			],
			label: None,
			layout: &render_pipelines[0].get_bind_group_layout(0),
		});

		self.bind_group = Some(bind_group);
	}

	fn enable(&mut self) {
		self.enabled = true;
	}

	fn disable(&mut self) {
		self.enabled = false;
	}

	fn is_enabled(&self) -> bool {
		self.enabled
	}
}
