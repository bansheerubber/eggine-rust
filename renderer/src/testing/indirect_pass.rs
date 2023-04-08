use carton::Carton;
use glam::Vec4Swizzles;
use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::{ Pass, shapes, };
use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::{ Memory, Node, NodeKind, PageError, PageUUID, textures, };
use crate::memory_subsystem::textures::Pager;
use crate::shaders::Program;
use crate::state::State;

use super::{ Batch, GlobalUniform, };
use super::uniforms::ObjectUniform;

/// Stores the render targets used by the pass object, recreated whenever the swapchain is out of date.
#[derive(Debug)]
struct RenderTextures {
	composite_bind_group: wgpu::BindGroup,
	depth_view: wgpu::TextureView,
	diffuse_format: wgpu::TextureFormat,
	diffuse_view: wgpu::TextureView,
	normal_format: wgpu::TextureFormat,
	normal_view: wgpu::TextureView,
	specular_format: wgpu::TextureFormat,
	specular_view: wgpu::TextureView,
	window_height: u32,
	window_width: u32,
}

/// Stores program related information used by the pass object.
#[derive(Debug)]
struct Programs {
	bone_uniforms: HashMap<u64, Vec<glam::Mat4>>,
	composite_program: Rc<Program>,
	g_buffer_program: Rc<Program>,
	object_uniforms: HashMap<u64, Vec<ObjectUniform>>,
	texture_bind_group: wgpu::BindGroup,
	uniform_bind_group: wgpu::BindGroup,
}

/// Stores references to the pages allocated by the pass object.
#[derive(Debug)]
struct AllocatedMemory {
	bone_storage_page: PageUUID,
	bone_storage_node: Node,
	bone_indices: PageUUID,
	bone_weights: PageUUID,
	global_uniform_node: Node,
	indices_page: PageUUID,
	indirect_command_buffer: PageUUID,
	indirect_command_buffer_map: HashMap<u64, Vec<u8>>,
	indirect_command_buffer_node: Node,
	max_objects_per_batch: u64,
	normals_page: PageUUID,
	object_storage_page: PageUUID,
	object_storage_node: Node,
	positions_page: PageUUID,
	uniforms_page: PageUUID,
	uvs_page: PageUUID,
}

/// Renders `Shape`s using deferred shading w/ indirect draw calls.
#[derive(Debug)]
pub struct IndirectPass<'a> {
	/// The pages and nodes allocated on the GPU.
	allocated_memory: AllocatedMemory,
	/// The batches of shapes used during rendering.
	batching_parameters: HashMap<shapes::BatchParametersKey, shapes::BatchParameters>,
	/// The blueprints used by the shapes rendered by this pass implementation.
	blueprints: Vec<Rc<shapes::blueprint::Blueprint>>,
	/// Since the compositor step's render operations do not change frame-to-frame, pre-record the operations to a render
	/// bundle for improved performance.
	compositor_render_bundle: Option<wgpu::RenderBundle>,
	/// Stores `wgpu` created objects required for basic rendering operations.
	context: Rc<WGPUContext>,
	/// Used for the `vertex_offset` for meshes in an indirect indexed draw call.
	highest_vertex_offset: i32,
	/// The amount of bytes written to the indices page.
	indices_page_written: u64,
	/// The total number of indices written into the index buffer. Used to calculate the `first_index` for meshes in an
	/// indirect indexed draw call.
	indices_written: u32,
	/// Reference to the memory subsystem, used to allocate/write data.
	memory: Arc<RwLock<Memory<'a>>>,
	/// Program related information.
	programs: Programs,
	/// The render textures used in the initial passes in the deferred shading pipeline.
	render_textures: RenderTextures,
	/// The amount of bytes written to the vertices page.
	vertices_page_written: u64,

	x_angle: f32,
	y_angle: f32,
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
				8_000_000, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST
			);

			// create node that fills entire indirect command buffer page
			let indirect_command_buffer_node = memory.get_page_mut(indirect_command_buffer)
				.unwrap()
				.allocate_node(8_000_000, 1, NodeKind::Buffer)
				.unwrap();

			// create the uniforms page
			let uniforms_page_uuid = memory.new_page(5_000, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
			let uniform_page = memory.get_page_mut(uniforms_page_uuid).unwrap();
			let global_uniform_node = uniform_page.allocate_node(
				std::mem::size_of::<GlobalUniform>() as u64, 4, NodeKind::Buffer
			).unwrap();

			// create the storage buffer for object uniforms
			let object_storage_size = 5_000_000;
			let object_storage_page_uuid = memory.new_page(
				object_storage_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
			);

			let object_page = memory.get_page_mut(object_storage_page_uuid).unwrap();
			let object_storage_node = object_page.allocate_node(
				object_storage_size, 4, NodeKind::Buffer
			).unwrap();

			let max_objects_per_batch = object_storage_size / std::mem::size_of::<ObjectUniform>() as u64;

			// create the storage buffer for object bone matrices
			let bone_storage_size = 60_000_000;
			let bone_storage_page_uuid = memory.new_page(
				bone_storage_size, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
			);

			let bone_page = memory.get_page_mut(bone_storage_page_uuid).unwrap();
			let bone_storage_node = bone_page.allocate_node(
				bone_storage_size, 4, NodeKind::Buffer
			).unwrap();

			// create vertex attribute pages
			let positions_page = memory.new_page(36_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
			let normals_page = memory.new_page(36_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
			let uvs_page = memory.new_page(24_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
			let indices_page = memory.new_page(24_000_000, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);

			let bone_indices = memory.new_page(48_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
			let bone_weights = memory.new_page(48_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);

			AllocatedMemory {
				bone_storage_page: bone_storage_page_uuid,
				bone_storage_node,
				bone_indices,
				bone_weights,
				global_uniform_node,
				indices_page,
				indirect_command_buffer,
				indirect_command_buffer_map: HashMap::new(),
				indirect_command_buffer_node,
				max_objects_per_batch,
				normals_page,
				object_storage_page: object_storage_page_uuid,
				object_storage_node,
				positions_page,
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
				shader_table.create_program("main-shader", fragment_shader, vertex_shader)
			};

			// create uniforms bind groups
			let uniform_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
							buffer: memory.get_page(allocated_memory.uniforms_page).unwrap().get_buffer(),
							offset: allocated_memory.global_uniform_node.offset,
							size: NonZeroU64::new(allocated_memory.global_uniform_node.size),
						}),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
							buffer: memory.get_page(allocated_memory.object_storage_page).unwrap().get_buffer(),
							offset: allocated_memory.object_storage_node.offset,
							size: NonZeroU64::new(allocated_memory.object_storage_node.size),
						}),
					},
					wgpu::BindGroupEntry {
						binding: 2,
						resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
							buffer: memory.get_page(allocated_memory.bone_storage_page).unwrap().get_buffer(),
							offset: allocated_memory.bone_storage_node.offset,
							size: NonZeroU64::new(allocated_memory.bone_storage_node.size),
						}),
					},
				],
				label: None,
				layout: g_buffer_program.get_bind_group_layouts()[0],
			});

			let sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
				address_mode_u: wgpu::AddressMode::ClampToEdge,
				address_mode_v: wgpu::AddressMode::ClampToEdge,
				address_mode_w: wgpu::AddressMode::ClampToEdge,
				label: None,
				mag_filter: wgpu::FilterMode::Linear,
				min_filter: wgpu::FilterMode::Nearest,
				mipmap_filter: wgpu::FilterMode::Nearest,
				..Default::default()
			});

			let texture_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
				layout: g_buffer_program.get_bind_group_layouts()[1],
			});

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
				shader_table.create_program("combination-shader", fragment_shader, vertex_shader)
			};

			Programs {
				bone_uniforms: HashMap::new(),
				composite_program,
				g_buffer_program,
    		object_uniforms: HashMap::new(),
				texture_bind_group,
				uniform_bind_group,
			}
		};

		// create render textures
		let render_textures = IndirectPass::create_render_textures(
			&context, boss.get_surface_config(), programs.composite_program.clone()
		);

		drop(memory);

		// allocate none texture
		let memory = boss.get_memory();
		let mut memory = memory.write().unwrap();
		let format = memory.get_texture_descriptor().format;
		let none_texture = memory.texture_pager.load_qoi("data/none.qoi", format, carton,).unwrap();

		// upload the none texture
		memory.set_none_texture(none_texture);

		Box::new(
			IndirectPass {
				allocated_memory,
				batching_parameters: HashMap::new(),
				blueprints: Vec::new(),
				compositor_render_bundle: None,
				context,
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

		self.x_angle += 0.01;
		self.y_angle = 1.0;

		let projection = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4 / 2.0, aspect_ratio, 0.1, 10000.0);
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
	fn create_render_textures(
		context: &WGPUContext, config: &wgpu::SurfaceConfiguration, combination_program: Rc<Program>
	) -> RenderTextures {
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
				| wgpu::TextureUsages::COPY_DST
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
				| wgpu::TextureUsages::COPY_DST
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
				| wgpu::TextureUsages::COPY_DST
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
				| wgpu::TextureUsages::COPY_DST
				| wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[],
		});

		let specular_view = specular_texture.create_view(&wgpu::TextureViewDescriptor::default());

		// create the samplers for the G-buffer
		let diffuse_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let normal_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
			label: None,
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		});

		let specular_sampler = context.device.create_sampler(&wgpu::SamplerDescriptor {
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
		let composite_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&diffuse_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&normal_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::Sampler(&normal_sampler),
				},
				wgpu::BindGroupEntry {
					binding: 4,
					resource: wgpu::BindingResource::TextureView(&specular_view),
				},
				wgpu::BindGroupEntry {
					binding: 5,
					resource: wgpu::BindingResource::Sampler(&specular_sampler),
				},
			],
			label: None,
			layout: combination_program.get_bind_group_layouts()[0],
		});

		RenderTextures {
			composite_bind_group,
			depth_view,
			diffuse_format,
			diffuse_view,
			normal_format,
			normal_view,
			specular_format,
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
		encoder.set_bind_group(0, &self.render_textures.composite_bind_group, &[]);

		encoder.draw(0..3, 0..1);

		self.compositor_render_bundle = Some(encoder.finish(&wgpu::RenderBundleDescriptor {
			label: None,
		}));
	}
}

/// Pass implementation. Indirectly render all shapes we have ownership over.
impl Pass for IndirectPass<'_> {
	fn states<'a>(&'a self) -> Vec<State<'a>> {
		vec![
			State { // state for the G-buffer stage
				depth_stencil: Some(wgpu::DepthStencilState {
					bias: wgpu::DepthBiasState::default(),
					depth_write_enabled: true,
					depth_compare: wgpu::CompareFunction::Less,
					format: wgpu::TextureFormat::Depth32Float,
					stencil: wgpu::StencilState::default(),
				}),
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
			State { // state for the composite stage
				depth_stencil: None,
				program: &self.programs.composite_program,
				render_targets: vec![Some(wgpu::ColorTargetState {
					blend: None,
					format: wgpu::TextureFormat::Bgra8UnormSrgb,
					write_mask: wgpu::ColorWrites::ALL,
				})],
				vertex_attributes: &[],
			},
		]
	}

	/// Encode all draw commands.
	fn encode(
		&mut self,
		deltatime: f64,
		encoder: &mut wgpu::CommandEncoder,
		pipelines: &Vec<&wgpu::RenderPipeline>,
		view: &wgpu::TextureView
	) {
		// update the uniforms
		self.update_uniforms();

		let mut g_buffer_load_op = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
		let mut depth_buffer_load_op = wgpu::LoadOp::Clear(1.0);

		// break up batches depending on how much we're able to fill a virtual texture quad
		let mut memory = self.memory.write().unwrap();

		let mut sorted_parameters = self.batching_parameters.values()
			.collect::<Vec<&shapes::BatchParameters>>();
		sorted_parameters.sort();

		let mut current_batch = Batch {
    	batch_parameters: Vec::new(),
			meshes_to_draw: 0,
    	texture_pager: textures::VirtualPager::new(20, memory.get_texture_descriptor().size.width as u16),
		};

		let texture_size = memory.get_texture_descriptor().size.width;

		// go through the batch parameters and compute the individual batches we need to draw
		let mut batches = Vec::new();
		for parameters in sorted_parameters {
			let allocated_cell = current_batch.texture_pager.allocate_texture(parameters.get_texture());

			if allocated_cell.is_none() {
				batches.push(current_batch);

				current_batch = Batch {
					batch_parameters: Vec::new(),
					meshes_to_draw: 0,
					texture_pager: textures::VirtualPager::new(20, texture_size as u16),
				};

				current_batch.texture_pager.allocate_texture(parameters.get_texture()).unwrap();
			} else {
				current_batch.batch_parameters.push(parameters);
				current_batch.meshes_to_draw += parameters.get_shapes()
					.fold(0, |acc, shape| acc + shape.borrow().get_blueprint().get_meshes().len());
			}
		}

		if current_batch.batch_parameters.len() != 0 {
			batches.push(current_batch);
		}

		// G-buffer render pass for every single batch
		for batch in batches.iter() {
			static DRAW_INDEXED_DIRECT_SIZE: usize = std::mem::size_of::<wgpu::util::DrawIndexedIndirect>();

			// get bone uniforms map for this batch
			if !self.programs.bone_uniforms.contains_key(&batch.make_key()) {
				self.programs.bone_uniforms.insert(batch.make_key(), Vec::new());
			}

			let bone_uniforms = self.programs.bone_uniforms.get_mut(&batch.make_key()).unwrap();

			// get object uniforms for this batch
			if !self.programs.object_uniforms.contains_key(&batch.make_key()) {
				self.programs.object_uniforms.insert(batch.make_key(), Vec::new());
			}

			let object_uniforms = self.programs.object_uniforms.get_mut(&batch.make_key()).unwrap();

			// get indirect command buffer for this batch
			self.allocated_memory.indirect_command_buffer_map.insert(
				batch.make_key(), vec![0; batch.meshes_to_draw * DRAW_INDEXED_DIRECT_SIZE]
			);

			let buffer = self.allocated_memory.indirect_command_buffer_map.get_mut(&batch.make_key()).unwrap();
			let mut draw_call_count: usize = 0;

			// upload textures
			let textures = batch.batch_parameters.iter()
				.map(|x| x.get_texture())
				.collect::<Vec<&Rc<textures::Texture>>>();

			// only upload if the textures aren't uploaded yet
			if !memory.is_same_pager(&batch.texture_pager) {
				memory.reset_pager();

				for texture in textures.iter() {
					memory.upload_texture(texture);
				}
			}

			let shapes = batch.batch_parameters.iter()
				.flat_map(|x| x.get_shapes())
				.collect::<Vec<&Rc<RefCell<shapes::Shape>>>>();

			let mut bone_index = 0;

			// iterate through the shapes in the batch and draw them
			for shape in shapes {
				let mut shape = shape.borrow_mut();

				for (node, mesh) in shape.get_blueprint().get_mesh_nodes().iter() { // TODO lets maybe not do a three level nested for loop
					for primitive in mesh.primitives.iter() {
						let texture = &primitive.material.texture;
						if !textures.contains(&texture) { // TODO optimize this whole damn texture thing
							continue;
						}

						if draw_call_count as u64 >= self.allocated_memory.max_objects_per_batch {
							panic!("Exceeded maximum amount of objects per batch")
						}

						// write draw index indirect into the buffer as fast as possible
						unsafe {
							let command = wgpu::util::DrawIndexedIndirect {
								base_index: primitive.first_index,
								base_instance: 0,
								instance_count: 1,
								vertex_count: primitive.vertex_count,
								vertex_offset: primitive.vertex_offset,
							};

							buffer[
								draw_call_count * DRAW_INDEXED_DIRECT_SIZE..(draw_call_count + 1) * DRAW_INDEXED_DIRECT_SIZE
							].copy_from_slice(
								std::slice::from_raw_parts(
									&command as *const _ as *const u8,
									DRAW_INDEXED_DIRECT_SIZE
								)
							)
						}

						// allocate `bone_uniforms`
						if bone_index + mesh.bones.len() > bone_uniforms.len() {
							bone_uniforms.resize(
								bone_uniforms.len() + mesh.bones.len(),
								glam::Mat4::IDENTITY
							);
						}

						let inverse_transform = node.borrow().transform.inverse();

						// the bone offset is the start of where we're going to write bone uniforms
						let bone_offset = bone_index as u32;

						// set bone uniforms
						for (bone, inverse_bind_matrix) in mesh.bones.iter() { // TODO move matrix multiplications to compute shader?
							bone_uniforms[bone_index] = shape.get_bone_matrix(bone, inverse_bind_matrix, &inverse_transform);
							bone_index += 1;
						}

						// allocate `object_uniforms`
						if draw_call_count >= object_uniforms.len() {
							object_uniforms.push(ObjectUniform::default());
						}

						// put the object uniforms into the array
						let model_matrix = node.borrow().transform.mul_mat4(shape.get_transformation());
						let texture = memory.texture_pager.get_cell(&texture).unwrap();
						object_uniforms[draw_call_count as usize] = ObjectUniform {
							model_matrix: model_matrix.to_cols_array(),
							texture_offset: glam::Vec4::new(
								texture.get_position().x as f32 / texture_size as f32,
								texture.get_position().y as f32 / texture_size as f32,
								texture.get_size() as f32 / texture_size as f32,
								texture.get_size() as f32 / texture_size as f32
							).to_array(),
							roughness: primitive.material.roughness,
							bone_offset,
							_padding: [0.0, 0.0],
						};

						draw_call_count += 1;
					}
				}

				shape.update_animation_timer(deltatime as f32);
			}

			// ensure immediate write to the buffer
			memory.get_page(self.allocated_memory.indirect_command_buffer)
				.unwrap()
				.write_buffer(&self.allocated_memory.indirect_command_buffer_node, &buffer);

			// write object uniforms to storage buffer
			memory.get_page(self.allocated_memory.object_storage_page)
				.unwrap()
				.write_slice(
					&self.allocated_memory.object_storage_node,
					bytemuck::cast_slice(&object_uniforms[0..draw_call_count as usize])
				);

			// write bone matrices to storage buffer
			memory.get_page(self.allocated_memory.bone_storage_page)
				.unwrap()
				.write_slice(
					&self.allocated_memory.bone_storage_node,
					bytemuck::cast_slice(&bone_uniforms[0..bone_index])
				);

			// do the render pass
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					color_attachments: &[
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &self.render_textures.diffuse_view,
						}),
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &self.render_textures.normal_view,
						}),
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &self.render_textures.specular_view,
						}),
					],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						depth_ops: Some(wgpu::Operations {
							load: depth_buffer_load_op,
							store: true,
						}),
						stencil_ops: None,
						view: &self.render_textures.depth_view,
					}),
					label: None,
				});

				render_pass.set_pipeline(pipelines[0]);

				// set vertex attributes
				render_pass.set_index_buffer(
					memory.get_page(self.allocated_memory.indices_page).unwrap().get_buffer().slice(0..self.indices_page_written),
					wgpu::IndexFormat::Uint32
				);

				render_pass.set_vertex_buffer(
					0, memory.get_page(self.allocated_memory.positions_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					1, memory.get_page(self.allocated_memory.normals_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					2, memory.get_page(self.allocated_memory.uvs_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					3, memory.get_page(self.allocated_memory.bone_weights).unwrap().get_buffer().slice(0..self.vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					4, memory.get_page(self.allocated_memory.bone_indices).unwrap().get_buffer().slice(0..self.vertices_page_written)
				);

				// bind uniforms
				render_pass.set_bind_group(0, &self.programs.uniform_bind_group, &[]);
				render_pass.set_bind_group(1, &self.programs.texture_bind_group, &[]);

				// draw all the objects
				render_pass.multi_draw_indexed_indirect(
					memory.get_page(self.allocated_memory.indirect_command_buffer).unwrap().get_buffer(), 0, draw_call_count as u32
				);

				// set clear ops
				g_buffer_load_op = wgpu::LoadOp::Load;
				depth_buffer_load_op = wgpu::LoadOp::Load;
			}
		}

		drop(memory);

		// combine the textures in the G-buffer
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
				label: None,
			});

			// create the render bundle if necessary
			if self.compositor_render_bundle.is_none() {
				self.create_compositor_bundle(pipelines[1]);
			}

			render_pass.execute_bundles([self.compositor_render_bundle.as_ref().unwrap()]);
		}
	}

	/// Handle a window resize.
	fn resize(&mut self, config: &wgpu::SurfaceConfiguration) {
		self.compositor_render_bundle = None; // invalidate the render bundle

		self.render_textures = IndirectPass::create_render_textures(
			&self.context, config, self.programs.composite_program.clone()
		);
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
}
