use carton::Carton;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::Pass;
use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::Memory;
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
	/// Reference to the memory subsystem, used to allocate/write data.
	/// Program used during rendering.
	program: Rc<Program>,
	/// Since the render operations do not change frame-to-frame, pre-record the operations to a render bundle for
	/// improved performance.
	render_bundle: Option<wgpu::RenderBundle>,
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

		Box::new(
			DepthVisualizer {
				bind_group: None,
				context,
				depth_pyramid: None,
				depth_pyramid_index: 0,
				program,
				render_bundle: None,
				timer: 0.0,
			}
		)
	}

	/// Creates a render bundle with commands that are shared between multiple render passes.
	fn create_bundle(&mut self, pipeline: &wgpu::RenderPipeline) {
		let mut encoder = self.context.device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
			color_formats: &[
				Some(wgpu::TextureFormat::Bgra8UnormSrgb)
			],
			depth_stencil: None,
			label: None,
			multiview: None,
			sample_count: 1,
		});

		encoder.set_pipeline(pipeline);

		// bind uniforms
		encoder.set_bind_group(0, &self.bind_group.as_ref().unwrap(), &[]);

		encoder.draw(0..3, 0..1);

		self.render_bundle = Some(encoder.finish(&wgpu::RenderBundleDescriptor {
			label: None,
		}));
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
				label: "composite-prepass".to_string(),
				layout: None,
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
		encoder: &mut wgpu::CommandEncoder,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		compute_pipelines: &Vec<&wgpu::ComputePipeline>,
		view: &wgpu::TextureView
	) {
		if self.depth_pyramid.is_none() {
			self.bind_group = None;
			return;
		}

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
				label: Some("composite-pass"),
			});

			// create the render bundle if necessary
			if self.render_bundle.is_none() {
				self.create_bundle(render_pipelines[0]);
			}

			render_pass.execute_bundles([self.render_bundle.as_ref().unwrap()]);
		}

		self.timer += deltatime;
		if self.timer > 1.0 { // increment `depth_pyramid_index` every second
			self.depth_pyramid_index += 1;
			let length = self.depth_pyramid.as_ref().unwrap().borrow().len();
			if self.depth_pyramid_index >= length  {
				self.depth_pyramid_index = 0;
			}

			self.timer = 0.0;

			self.create_bind_groups(render_pipelines, compute_pipelines);
			self.render_bundle = None;

			println!("Depth pyramid step {}", self.depth_pyramid_index);
		}
	}

	/// Handle a window resize.
	fn resize(&mut self, config: &wgpu::SurfaceConfiguration) {
		self.render_bundle = None; // invalidate the render bundle
	}

	/// Recreate bind groups.
	fn create_bind_groups(
		&mut self,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		compute_pipelines: &Vec<&wgpu::ComputePipeline>
	) {
		if self.depth_pyramid.is_none() {
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
}
