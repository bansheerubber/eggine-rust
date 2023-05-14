use carton::Carton;
use glam::Vec4Swizzles;
use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::{ Pass, shapes, };
use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::{ Memory, Node, NodeKind, PageError, textures, };
use crate::state::{ ComputeState, RenderState, };
use crate::testing::indirect_pass::{
	AllocatedMemory,
	BindGroups,
	GlobalUniform,
	ObjectUniform,
	Programs,
	RenderTextures,
};

/// Renders `Shape`s using deferred shading w/ indirect draw calls.
#[derive(Debug)]
pub struct IndirectPass<'a> {
	/// The pages and nodes allocated on the GPU.
	pub(crate) allocated_memory: AllocatedMemory<'a>,
	/// The batches of shapes used during rendering.
	pub(crate) batching_parameters: HashMap<shapes::BatchParametersKey, shapes::BatchParameters>,
	/// Stores the bind groups used for shaders.
	pub(crate) bind_groups: Option<BindGroups>,
	/// The blueprints used by the shapes rendered by this pass implementation.
	pub(crate) blueprints: Vec<Rc<shapes::blueprint::Blueprint>>,
	/// Since the compositor step's render operations do not change frame-to-frame, pre-record the operations to a render
	/// bundle for improved performance.
	pub(crate) compositor_render_bundle: Option<wgpu::RenderBundle>,
	/// Stores `wgpu` created objects required for basic rendering operations.
	pub(crate) context: Rc<WGPUContext>,
	/// Whether the pass can render.
	pub(crate) enabled: bool,
	/// Used for the `vertex_offset` for meshes in an indirect indexed draw call.
	pub(crate) highest_vertex_offset: i32,
	/// The amount of bytes written to the indices page.
	pub(crate) indices_page_written: u64,
	/// The total number of indices written into the index buffer. Used to calculate the `first_index` for meshes in an
	/// indirect indexed draw call.
	pub(crate) indices_written: u32,
	/// Reference to the memory subsystem, used to allocate/write data.
	pub(crate) memory: Arc<RwLock<Memory<'a>>>,
	/// Program related information.
	pub(crate) programs: Programs,
	/// The render textures used in the initial passes in the deferred shading pipeline.
	pub(crate) render_textures: RenderTextures,
	/// The amount of bytes written to the vertices page.
	pub(crate) vertices_page_written: u64,

	pub(crate) x_angle: f32,
	pub(crate) y_angle: f32,
}

impl<'a> IndirectPass<'a> {
	pub fn new<'q>(boss: &mut Boss<'q>, carton: &mut Carton) -> Box<IndirectPass<'q>> {
		let memory = boss.get_memory();
		let mut memory = memory.write().unwrap();

		let context = boss.get_context().clone();

		// allocate memory
		let allocated_memory = {
			// create indirect command buffer page
			let indirect_command_buffer = memory.new_page(
				8_000_000,
				wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
				"indirect-command-buffer",
				false
			);

			// create node that fills entire indirect command buffer page
			let indirect_command_buffer_node = memory.get_page_mut(indirect_command_buffer)
				.unwrap()
				.allocate_node(8_000_000, 1, NodeKind::Buffer)
				.unwrap();

			// create the uniforms page
			let uniforms_page_uuid = memory.new_page(
				5_000,
				wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
				"uniforms-buffer",
				false
			);
			let uniform_page = memory.get_page_mut(uniforms_page_uuid).unwrap();
			let global_uniform_node = uniform_page.allocate_node(
				std::mem::size_of::<GlobalUniform>() as u64, 4, NodeKind::Buffer
			).unwrap();

			// create the storage buffer for object uniforms
			let object_storage_size = 5_000_000;
			let object_storage_page_uuid = memory.new_page(
				object_storage_size,
				wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
				"shape-uniform-buffer-object",
				false
			);

			let object_page = memory.get_page_mut(object_storage_page_uuid).unwrap();
			let object_storage_node = object_page.allocate_node(
				object_storage_size, 4, NodeKind::Buffer
			).unwrap();

			let max_objects_per_batch = object_storage_size / std::mem::size_of::<ObjectUniform>() as u64;

			// create the storage buffer for object bone matrices
			let bone_storage_size = 60_000_000;
			let bone_storage_page_uuid = memory.new_page(
				bone_storage_size,
				wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
				"bone-uniform-buffer-object",
				false
			);

			let bone_page = memory.get_page_mut(bone_storage_page_uuid).unwrap();
			let bone_storage_node = bone_page.allocate_node(
				bone_storage_size, 4, NodeKind::Buffer
			).unwrap();

			// create the storage buffer for the mesh primitive table
			let mesh_primitive_table_size = 1_000_000;
			let mesh_primitive_table_page_uuid = memory.new_page(
				mesh_primitive_table_size,
				wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
				"bone-uniform-buffer-object",
				false
			);

			let mesh_primitive_table_page = memory.get_page_mut(mesh_primitive_table_page_uuid).unwrap();
			let mesh_primitive_table_node = mesh_primitive_table_page.allocate_node(
				mesh_primitive_table_size, 4, NodeKind::Buffer
			).unwrap();

			// create vertex attribute pages
			let positions_page = memory.new_page(
				36_000_000,
				wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				"position-attributes",
				false
			);

			let normals_page = memory.new_page(
				36_000_000,
				wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				"normal-attributes",
				false
			);

			let uvs_page = memory.new_page(
				24_000_000,
				wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				"uv-attributes",
				false
			);

			let bone_indices = memory.new_page(
				48_000_000,
				wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				"bone-indices-attributes",
				false
			);

			let bone_weights = memory.new_page(
				48_000_000,
				wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				"bone-weights-attributes",
				false
			);

			let indices_page = memory.new_page(
				24_000_000,
				wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
				"indices-buffer",
				false
			);

			// test buffer for reasons
			let test_page_uuid = memory.new_page(
				48_000_000,
				wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
				"test-buffer",
				false
			);
			let test_page = memory.get_page_mut(test_page_uuid).unwrap();
			let test_node = test_page.allocate_node(
				std::mem::size_of::<GlobalUniform>() as u64, 4, NodeKind::Buffer
			).unwrap();

			AllocatedMemory {
				bone_storage_page: bone_storage_page_uuid,
				bone_storage_node,
				bone_indices,
				bone_weights,
				depth_pyramid: Rc::new(RefCell::new(Vec::new())),
				global_uniform_node,
				indices_page,
				indirect_command_buffer,
				indirect_command_buffer_map: HashMap::new(),
				indirect_command_buffer_node,
				max_objects_per_batch,
				mesh_primitive_table_page: mesh_primitive_table_page_uuid,
				mesh_primitive_table_node,
				mesh_primitive_table_size: 0,
				normals_page,
				object_storage_page: object_storage_page_uuid,
				object_storage_node,
				positions_page,
				test_node,
				test_page: test_page_uuid,
				uniforms_page: uniforms_page_uuid,
				uvs_page,
			}
		};

		// create programs/bind groups
		let programs = {
			// create the G-buffer generating program
			let g_buffer_program = {
				// define shader names
				let fragment_shader = "data/main.frag.spv".to_string();
				let vertex_shader = "data/main.vert.spv".to_string();

				// lock shader table
				let shader_table = boss.get_shader_table();
				let mut shader_table = shader_table.write().unwrap();

				// load from carton
				let fragment_shader = shader_table.load_shader_from_carton(&fragment_shader, carton).unwrap();
				let vertex_shader = shader_table.load_shader_from_carton(&vertex_shader, carton).unwrap();

				// create the program
				shader_table.create_render_program("main-shader", fragment_shader, vertex_shader)
			};

			// create the G-buffer combination program
			let composite_program = {
				// define shader names
				let fragment_shader = "data/combine.frag.spv".to_string();
				let vertex_shader = "data/combine.vert.spv".to_string();

				// lock shader table
				let shader_table = boss.get_shader_table();
				let mut shader_table = shader_table.write().unwrap();

				// load from carton
				let fragment_shader = shader_table.load_shader_from_carton(&fragment_shader, carton).unwrap();
				let vertex_shader = shader_table.load_shader_from_carton(&vertex_shader, carton).unwrap();

				// create the program
				shader_table.create_render_program("combination-shader", fragment_shader, vertex_shader)
			};

			// create the depth prepass program
			let prepass_program = {
				// define shader names
				let fragment_shader = "data/depth-prepass.frag.spv".to_string();
				let vertex_shader = "data/depth-prepass.vert.spv".to_string();

				// lock shader table
				let shader_table = boss.get_shader_table();
				let mut shader_table = shader_table.write().unwrap();

				// load from carton
				let fragment_shader = shader_table.load_shader_from_carton(&fragment_shader, carton).unwrap();
				let vertex_shader = shader_table.load_shader_from_carton(&vertex_shader, carton).unwrap();

				// create the program
				shader_table.create_render_program("depth-prepass", fragment_shader, vertex_shader)
			};

			// create the depth pyramid compute shader program
			let depth_pyramid_program = {
				// define shader names
				let compute_shader = "data/depth-pyramid.comp.spv".to_string();

				// lock shader table
				let shader_table = boss.get_shader_table();
				let mut shader_table = shader_table.write().unwrap();

				// load from carton
				let compute_shader = shader_table.load_shader_from_carton(&compute_shader, carton).unwrap();

				// create the program
				shader_table.create_compute_program("depth-pyramid", compute_shader)
			};

			let depth_pyramid_bind_group_layout
				= context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
					entries: &[
						wgpu::BindGroupLayoutEntry {
							binding: 0,
							count: None,
							ty: wgpu::BindingType::Texture {
								multisampled: false,
								sample_type: wgpu::TextureSampleType::Float { filterable: true, },
								view_dimension: wgpu::TextureViewDimension::D2,
							},
							visibility: wgpu::ShaderStages::COMPUTE,
						},
						wgpu::BindGroupLayoutEntry {
							binding: 1,
							count: None,
							visibility: wgpu::ShaderStages::COMPUTE,
							ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
						},
						wgpu::BindGroupLayoutEntry {
							count: None,
							binding: 2,
							ty: wgpu::BindingType::StorageTexture {
								access: wgpu::StorageTextureAccess::WriteOnly,
								format: wgpu::TextureFormat::R32Float,
								view_dimension: wgpu::TextureViewDimension::D2,
							},
							visibility: wgpu::ShaderStages::COMPUTE,
						},
					],
					label: Some("depth-pyramid-bind-group-layout"),
				});

			let depth_pyramid_pipeline_layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				bind_group_layouts: &[&depth_pyramid_bind_group_layout],
				label: Some("depth-pyramid-pipeline-layout"),
				push_constant_ranges: &[wgpu::PushConstantRange {
					range: 0..16,
					stages: wgpu::ShaderStages::COMPUTE,
				}],
			});

			Programs {
				bone_uniforms: HashMap::new(),
				composite_program,
				depth_pyramid_bind_group_layout,
				depth_pyramid_pipeline_layout,
				depth_pyramid_program,
				g_buffer_program,
				object_uniforms: HashMap::new(),
				prepass_program,
			}
		};

		// create render textures
		let render_textures = IndirectPass::create_render_textures(&context, boss.get_surface_config());

		drop(memory);

		// allocate none texture
		{
			let memory = boss.get_memory();
			let mut memory = memory.write().unwrap();
			let format = memory.get_texture_descriptor().format;
			let none_texture = memory.texture_pager.load_qoi("data/none.qoi", format, carton,).unwrap();

			// upload the none texture
			memory.set_none_texture(none_texture);
		}

		Box::new(
			IndirectPass {
				allocated_memory,
				batching_parameters: HashMap::new(),
				bind_groups: None,
				blueprints: Vec::new(),
				compositor_render_bundle: None,
				context,
				enabled: true,
				highest_vertex_offset: 0,
				indices_page_written: 0,
				indices_written: 0,
				memory: boss.get_memory().clone(),
				programs,
				render_textures,
				vertices_page_written: 0,

				x_angle: 0.0,
				y_angle: 0.0,
			}
		)
	}

	/// Gives `Blueprint` ownership over to this `Pass` object.
	pub fn add_blueprint(&mut self, blueprint: Rc<shapes::blueprint::Blueprint>) -> Rc<shapes::blueprint::Blueprint> {
		// collect together the textures for the meshes in the blueprint
		for texture in blueprint.get_textures().iter() {
			let key = shapes::BatchParametersKey {
				texture: texture.clone(),
			};

			if !self.batching_parameters.contains_key(&key) {
				let parameters = shapes::BatchParameters::new(texture.clone());
				let key = parameters.make_key();
				self.batching_parameters.insert(key, parameters);
			}
		}

		self.blueprints.push(blueprint);
		self.blueprints[self.blueprints.len() - 1].clone()
	}

	/// Gives `Shape` ownership over to this `Pass` object.
	pub fn add_shape(&mut self, shape: shapes::Shape) -> Rc<RefCell<shapes::Shape>> {
		let shape = Rc::new(RefCell::new(shape));

		for texture in shape.borrow().get_blueprint().get_textures().iter() {
			let batch = self.batching_parameters.get_mut(&shapes::BatchParametersKey {
				texture: texture.clone(),
			}).unwrap();

			batch.add_shape(shape.clone());
		}

		return shape;
	}

	/// Prepares the uniforms for the current tick.
	fn update_uniforms(&mut self) {
		let aspect_ratio = self.render_textures.window_width as f32 / self.render_textures.window_height as f32;

		let position = glam::Vec4::new(
			75.0 * self.x_angle.cos() * self.y_angle.sin(),
			75.0 * self.x_angle.sin() * self.y_angle.sin(),
			75.0 * self.y_angle.cos(),
			0.0,
		);

		self.x_angle = std::f32::consts::PI;
		self.y_angle = 1.0;

		let projection = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4 / 2.0, aspect_ratio, 1.0, 1000.0);
		let view = glam::Mat4::look_at_rh(
			position.xyz(),
			glam::Vec3::new(30.0, 30.0, 0.0),
			glam::Vec3::Z, // z is up
		);

		let uniform = GlobalUniform {
			camera_position: *(position).as_ref(),
			perspective_matrix: *(projection).as_ref(),
			view_matrix: *(view).as_ref(),
		};

		let memory = self.memory.read().unwrap();
		memory.get_page(self.allocated_memory.uniforms_page)
			.unwrap()
			.write_slice(&self.allocated_memory.global_uniform_node, bytemuck::cast_slice(&[uniform]));
	}

	/// Recreates render textures used in the G-buffer.
	fn create_render_textures(context: &WGPUContext, config: &wgpu::SurfaceConfiguration) -> RenderTextures {
		// create the depth texture
		let depth_texture = context.device.create_texture(&wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Depth32Float,
			label: None,
			mip_level_count: 1,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 1,
				height: config.height,
				width: config.width,
			},
			usage: wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

		// create the diffuse texture
		let diffuse_format = config.format;
		let diffuse_texture = context.device.create_texture(&wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: diffuse_format,
			label: None,
			mip_level_count: 1,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 1,
				height: config.height,
				width: config.width,
			},
			usage: wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let diffuse_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());

		// create the normal texture
		let normal_format = wgpu::TextureFormat::Rgb10a2Unorm; // TODO better format for this?
		let normal_texture = context.device.create_texture(&wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: normal_format,
			label: None,
			mip_level_count: 1,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 1,
				height: config.height,
				width: config.width,
			},
			usage: wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor::default());

		// create the specular texture
		let specular_format = config.format;
		let specular_texture = context.device.create_texture(&wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: specular_format,
			label: None,
			mip_level_count: 1,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 1,
				height: config.height,
				width: config.width,
			},
			usage: wgpu::TextureUsages::TEXTURE_BINDING
				| wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let specular_view = specular_texture.create_view(&wgpu::TextureViewDescriptor::default());

		RenderTextures {
			depth_texture,
			depth_view,
			diffuse_format,
			diffuse_texture,
			diffuse_view,
			normal_format,
			normal_texture,
			normal_view,
			specular_format,
			specular_texture,
			specular_view,
			window_height: config.height,
			window_width: config.width,
		}
	}

	/// Creates a render bundle with commands that are shared between multiple render passes.
	fn create_compositor_bundle(&mut self, pipeline: &wgpu::RenderPipeline) {
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
		encoder.set_bind_group(0, &self.bind_groups.as_ref().unwrap().composite_bind_group, &[]);

		encoder.draw(0..3, 0..1);

		self.compositor_render_bundle = Some(encoder.finish(&wgpu::RenderBundleDescriptor {
			label: None,
		}));
	}
}

/// Pass implementation. Indirectly render all shapes we have ownership over.
impl Pass for IndirectPass<'_> {
	fn render_states<'a>(&'a self) -> Vec<RenderState<'a>> {
		vec![
			RenderState { // state for the G-buffer stage
				depth_stencil: Some(wgpu::DepthStencilState {
					bias: wgpu::DepthBiasState::default(),
					depth_write_enabled: false,
					depth_compare: wgpu::CompareFunction::LessEqual,
					format: wgpu::TextureFormat::Depth32Float,
					stencil: wgpu::StencilState::default(),
				}),
				label: "g-buffer-prepass".to_string(),
				layout: None,
				program: &self.programs.g_buffer_program,
				render_targets: vec![
					Some(self.render_textures.diffuse_format.into()),
					Some(self.render_textures.normal_format.into()),
					Some(self.render_textures.specular_format.into()),
				],
				vertex_attributes: &[
					wgpu::VertexBufferLayout { // vertices
						array_stride: 4 * 3,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x3,
							offset: 0,
							shader_location: 0,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // normals
						array_stride: 4 * 3,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x3,
							offset: 0,
							shader_location: 1,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // uvs
						array_stride: 4 * 2,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x2,
							offset: 0,
							shader_location: 2,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // bone weights
						array_stride: 4 * 4,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x4,
							offset: 0,
							shader_location: 3,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // bone indices
						array_stride: 2 * 4,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Uint16x4,
							offset: 0,
							shader_location: 4,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
				],
			},
			RenderState { // state for the composite stage
				depth_stencil: None,
				label: "composite-prepass".to_string(),
				layout: None,
				program: &self.programs.composite_program,
				render_targets: vec![Some(wgpu::ColorTargetState {
					blend: None,
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				vertex_attributes: &[],
			},
			RenderState { // state for the depth prepass
				depth_stencil: Some(wgpu::DepthStencilState {
					bias: wgpu::DepthBiasState::default(),
					depth_write_enabled: true,
					depth_compare: wgpu::CompareFunction::Less,
					format: wgpu::TextureFormat::Depth32Float,
					stencil: wgpu::StencilState::default(),
				}),
				label: "depth-prepass".to_string(),
				layout: None,
				program: &self.programs.prepass_program,
				render_targets: Vec::new(),
				vertex_attributes: &[
					wgpu::VertexBufferLayout { // vertices
						array_stride: 4 * 3,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x3,
							offset: 0,
							shader_location: 0,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // bone weights
						array_stride: 4 * 4,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Float32x4,
							offset: 0,
							shader_location: 1,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
					wgpu::VertexBufferLayout { // bone indices
						array_stride: 2 * 4,
						attributes: &[wgpu::VertexAttribute {
							format: wgpu::VertexFormat::Uint16x4,
							offset: 0,
							shader_location: 2,
						}],
						step_mode: wgpu::VertexStepMode::Vertex,
					},
				],
			},
		]
	}

	fn compute_states<'a>(&'a self) -> Vec<ComputeState<'a>> {
		vec![ComputeState {
			label: "depth-pyramid-pass".to_string(),
			layout: Some(&self.programs.depth_pyramid_pipeline_layout),
			program: &self.programs.depth_pyramid_program,
		}]
	}

	/// Encode all draw commands.
	fn encode(
		&mut self,
		deltatime: f64,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		compute_pipelines: &Vec<&wgpu::ComputePipeline>,
		view: &wgpu::TextureView
	) {
		// update the uniforms
		self.update_uniforms();

		// break up batches depending on how much we're able to fill a virtual texture quad
		let mut batches = IndirectPass::generate_batches(&self.memory, &self.batching_parameters);

		IndirectPass::buffer_generation(
			&self.memory,
			&mut self.allocated_memory,
			&mut self.programs,
			&mut batches,
			deltatime
		);

		let mut last_rendered_batch = None;

		last_rendered_batch = IndirectPass::depth_prepass(
			self.context.clone(),
			&self.memory,
			&mut self.allocated_memory,
			&mut self.programs,
			&self.bind_groups.as_ref().unwrap(),
			&self.render_textures,
			self.indices_page_written,
			self.vertices_page_written,
			&batches,
			render_pipelines,
			last_rendered_batch
		);

		// run occlusion compute shader
		{
			let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("depth-pyramid-pass-encoder"),
			});

			let depth_pyramid = self.allocated_memory.depth_pyramid.borrow();

			let bind_groups = &self.bind_groups.as_ref().unwrap().depth_pyramid_bind_groups;

			{
				let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
					label: Some("depth-pyramid-pass"),
				});

				compute_pass.set_pipeline(compute_pipelines[0]);

				for i in 0..bind_groups.len() {
					let size = glam::Vec2::new(
						depth_pyramid[i].width as f32,
						depth_pyramid[i].height as f32
					);

					compute_pass.set_push_constants(0, bytemuck::cast_slice(&[size]));
					compute_pass.set_bind_group(0, &self.bind_groups.as_ref().unwrap().depth_pyramid_bind_groups[i], &[]);
					compute_pass.dispatch_workgroups((size.x / 16.0).ceil() as u32, (size.y / 16.0).ceil() as u32, 1);
				}
			}

			self.context.queue.submit(Some(encoder.finish()));
		}

		IndirectPass::g_buffer_pass(
			self.context.clone(),
			&self.memory,
			&mut self.allocated_memory,
			&mut self.programs,
			&self.bind_groups.as_ref().unwrap(),
			&self.render_textures,
			self.indices_page_written,
			self.vertices_page_written,
			&batches,
			render_pipelines,
			last_rendered_batch
		);

		// combine the textures in the G-buffer. doesn't have its own file since this is only like 20 lines long
		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
			label: Some("composite-pass-encoder"),
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
				label: Some("composite-pass"),
			});

			// create the render bundle if necessary
			if self.compositor_render_bundle.is_none() {
				self.create_compositor_bundle(render_pipelines[1]);
			}

			render_pass.execute_bundles([self.compositor_render_bundle.as_ref().unwrap()]);
		}

		self.context.queue.submit(Some(encoder.finish()));
	}

	/// Handle a window resize.
	fn resize(&mut self, config: &wgpu::SurfaceConfiguration) {
		self.compositor_render_bundle = None; // invalidate the render bundle

		self.render_textures.destroy();
		self.render_textures = IndirectPass::create_render_textures(&self.context, config);
	}

	/// Gets the memory usage of the pass' render textures.
	fn get_render_texture_usage(&self) -> u64 {
		let pixels = self.render_textures.window_height * self.render_textures.window_width;

		let mut total = pixels * 4; // diffuse texture
		total += pixels * 4; // specular texture
		total += pixels * 4; // normal texture
		total += pixels * 4; // depth texture

		total as u64
	}

	/// Recreate bind groups.
	fn create_bind_groups(
		&mut self,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		_compute_pipelines: &Vec<&wgpu::ComputePipeline>
	) {
		let depth_pyramid_bind_groups = self.create_depth_pyramid();

		let memory = self.memory.read().unwrap();

		let uniform_bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: memory.get_page(self.allocated_memory.uniforms_page).unwrap().get_buffer(),
						offset: self.allocated_memory.global_uniform_node.offset,
						size: NonZeroU64::new(self.allocated_memory.global_uniform_node.size),
					}),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: memory.get_page(self.allocated_memory.object_storage_page).unwrap().get_buffer(),
						offset: self.allocated_memory.object_storage_node.offset,
						size: NonZeroU64::new(self.allocated_memory.object_storage_node.size),
					}),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: memory.get_page(self.allocated_memory.bone_storage_page).unwrap().get_buffer(),
						offset: self.allocated_memory.bone_storage_node.offset,
						size: NonZeroU64::new(self.allocated_memory.bone_storage_node.size),
					}),
				},
			],
			label: None,
			layout: &render_pipelines[0].get_bind_group_layout(0),
		});

		let sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			label: None,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let texture_bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(memory.get_texture_view()),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&sampler),
				},
			],
			label: None,
			layout: &render_pipelines[0].get_bind_group_layout(1),
		});

		// create the samplers for the G-buffer
		let diffuse_sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let normal_sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let specular_sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		// create G-buffer combiner uniform buffer bind groups
		let composite_bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&self.render_textures.diffuse_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&self.render_textures.normal_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&normal_sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: wgpu::BindingResource::TextureView(&self.render_textures.specular_view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&specular_sampler),
				},
			],
			label: None,
			layout: &render_pipelines[1].get_bind_group_layout(0),
		});

		drop(memory);

		self.bind_groups = Some(BindGroups {
			composite_bind_group,
			depth_pyramid_bind_groups,
			texture_bind_group,
			uniform_bind_group,
		})
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

/// The way I implement indirect rendering requires seperate pages for each vertex attribute.
impl shapes::blueprint::State for IndirectPass<'_> {
	fn calc_first_index(&mut self, num_indices: u32) -> u32 {
		let first_index = self.indices_written;
		self.indices_written += num_indices as u32;
		return first_index;
	}

	fn calc_vertex_offset(&mut self, highest_index: i32) -> i32 {
		let vertex_offset = self.highest_vertex_offset;
		self.highest_vertex_offset += highest_index + 1;
		return vertex_offset;
	}

	fn prepare_mesh_pages(&mut self) {
		// doesn't need to do anything
	}

	fn get_named_node(
		&self,
		name: shapes::blueprint::DataKind,
		size: u64,
		align: u64,
		node_kind: NodeKind,
	) -> Result<Option<Node>, PageError> {
		let page = match name {
			shapes::blueprint::DataKind::BoneIndex => self.allocated_memory.bone_indices,
			shapes::blueprint::DataKind::BoneWeight => self.allocated_memory.bone_weights,
			shapes::blueprint::DataKind::Index => self.allocated_memory.indices_page,
			shapes::blueprint::DataKind::Normal => self.allocated_memory.normals_page,
			shapes::blueprint::DataKind::Position => self.allocated_memory.positions_page,
			shapes::blueprint::DataKind::UV => self.allocated_memory.uvs_page,
			_ => return Ok(None),
		};

		let mut memory = self.memory.write().unwrap();
		memory.get_page_mut(page).unwrap().allocate_node(size, align, node_kind)
			.and_then(|node| {
				Ok(Some(node))
			})
	}

	fn write_node(&mut self, name: shapes::blueprint::DataKind, node: &Node, buffer: Vec<u8>) {
		let page = match name {
			shapes::blueprint::DataKind::BoneIndex => self.allocated_memory.bone_indices,
			shapes::blueprint::DataKind::BoneWeight => self.allocated_memory.bone_weights,
			shapes::blueprint::DataKind::Index => {
				self.indices_page_written += buffer.len() as u64;
				self.allocated_memory.indices_page
			},
			shapes::blueprint::DataKind::Normal => self.allocated_memory.normals_page,
			shapes::blueprint::DataKind::Position => {
				self.vertices_page_written += buffer.len() as u64;
				self.allocated_memory.positions_page
			},
			shapes::blueprint::DataKind::UV => self.allocated_memory.uvs_page,
			_ => return,
		};

		let mut memory = self.memory.write().unwrap();
		memory.write_buffer(page, node, buffer);
	}

	fn get_none_texture(&mut self) -> Rc<textures::Texture> {
		let memory = self.memory.read().unwrap();
		memory.get_none_texture().unwrap().clone()
	}

	fn required_attributes(&self) -> Vec<shapes::blueprint::DataKind> {
		vec![
			shapes::blueprint::DataKind::BoneIndex,
			shapes::blueprint::DataKind::BoneWeight,
			shapes::blueprint::DataKind::Normal,
			shapes::blueprint::DataKind::Position,
			shapes::blueprint::DataKind::UV,
		]
	}

	fn add_mesh_primitive(&mut self, entry: shapes::blueprint::MeshPrimitiveTableEntry) -> u32 {
		static ENTRY_SIZE: usize = std::mem::size_of::<shapes::blueprint::MeshPrimitiveTableEntry>();

		let index = self.allocated_memory.mesh_primitive_table_size;
		self.allocated_memory.mesh_primitive_table_size += 1;

		let memory = self.memory.read().unwrap();
		let page = memory.get_page(self.allocated_memory.mesh_primitive_table_page).unwrap();

		page.write_slice_with_offset(
			&self.allocated_memory.mesh_primitive_table_node,
			(ENTRY_SIZE * index as usize) as u64,
			bytemuck::cast_slice(&[entry])
		);

		return index;
	}
}
