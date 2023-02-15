use carton::Carton;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::{ Pass, shape, };
use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::{ Memory, Node, NodeKind, PageError, PageUUID, };
use crate::shaders::Program;
use crate::state::State;

use super::VertexUniform;

/// Renders `Shape`s using a indirect buffer.
#[derive(Debug)]
pub struct IndirectPass {
	blueprints: Vec<Rc<shape::Blueprint>>,
	context: Rc<WGPUContext>,
	depth_texture: wgpu::Texture,
	depth_view: wgpu::TextureView,
	/// Used for the `vertex_offset` for meshes in an indirect indexed draw call.
	highest_vertex_offset: i32,
	indices_page: PageUUID,
	/// The amount of bytes written to the indices page.
	indices_page_written: u64,
	/// The total number of indices written into the index buffer. Used to calculate the `first_index` for meshes in an
	/// indirect indexed draw call.
	indices_written: u32,
	indirect_command_buffer: PageUUID,
	indirect_command_buffer_node: Node,
	memory: Arc<RwLock<Memory>>,
	program: Rc<Program>,
	shapes: Vec<shape::Shape>,
	uniform_bind_group: wgpu::BindGroup,
	uniforms_page: PageUUID,
	vertices_page: PageUUID,
	/// The amount of bytes written to the vertices page.
	vertices_page_written: u64,
	vertex_uniform_node: Node,
	window_height: u32,
	window_width: u32,

	colors_page: PageUUID,
}

impl IndirectPass {
	pub fn new(boss: &mut Boss, carton: &mut Carton) -> Self {
		let memory = boss.get_memory();
		let mut memory = memory.write().unwrap();

		let context = boss.get_context().clone();

		// create indirect command buffer page
		let indirect_command_buffer = memory.new_page(
			8_000_000, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST
		);

		// create node that fills entire indirect command buffer page
		let indirect_command_buffer_node = memory.get_page_mut(indirect_command_buffer)
			.unwrap()
			.allocate_node(8_000_000, 1, NodeKind::Buffer)
			.unwrap();

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
		let program = shader_table.create_program("main-shader", fragment_shader, vertex_shader);

		let uniforms_page = memory.new_page(5_000, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
		let page = memory.get_page_mut(uniforms_page).unwrap();
		let vertex_uniform_buffer = page.allocate_node(
			std::mem::size_of::<VertexUniform>() as u64, 4, NodeKind::Buffer
		).unwrap();

		// create uniform buffer bind groups
		let uniform_bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
						buffer: page.get_buffer(),
						offset: vertex_uniform_buffer.offset,
						size: NonZeroU64::new(vertex_uniform_buffer.size),
					}),
				}
			],
			label: None,
			layout: program.get_bind_group_layouts()[0],
		});

		// create the depth texture
		let (depth_texture, depth_view) = IndirectPass::create_depth_texture(&context, boss.get_surface_config());

		IndirectPass {
			blueprints: Vec::new(),
			context,
			depth_texture,
			depth_view,
			highest_vertex_offset: 0,
			indices_page: memory.new_page(96_000_000, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST),
			indices_written: 0,
			indices_page_written: 0,
			indirect_command_buffer,
			indirect_command_buffer_node,
			memory: boss.get_memory().clone(),
			program,
			shapes: Vec::new(),
			uniform_bind_group,
			uniforms_page,
			vertices_page: memory.new_page(256_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
			vertices_page_written: 0,
			vertex_uniform_node: vertex_uniform_buffer,
			window_height: boss.get_window_size().0,
			window_width: boss.get_window_size().1,

			colors_page: memory.new_page(256_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
		}
	}

	/// Gives `Blueprint` ownership over to this `Pass` object.
	pub fn add_blueprint(&mut self, blueprint: Rc<shape::Blueprint>) -> Rc<shape::Blueprint> {
		self.blueprints.push(blueprint);
		return self.blueprints[self.blueprints.len() - 1].clone();
	}

	/// Gives `Shape` ownership over to this `Pass` object.
	pub fn add_shape(&mut self, shape: shape::Shape) {
		self.shapes.push(shape);
	}

	fn update_uniforms(&mut self) {
		let aspect_ratio = self.window_width as f32 / self.window_height as f32;

		let projection = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect_ratio, 0.1, 400.0);
		let view = glam::Mat4::look_at_rh(
			glam::Vec3::new(5.0, 5.0, 5.0),
			glam::Vec3::new(0.0, 0.0, 0.0),
			glam::Vec3::Y, // y is up
		);

		let uniform = VertexUniform {
			view_perspective_matrix: *(projection * view).as_ref(),
		};

		let memory = self.memory.read().unwrap();
		memory.get_page(self.uniforms_page)
			.unwrap()
			.write_slice(&self.vertex_uniform_node, bytemuck::cast_slice(&[uniform]));
	}

	fn create_depth_texture(
		context: &WGPUContext, config: &wgpu::SurfaceConfiguration
	) -> (wgpu::Texture, wgpu::TextureView) {
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

		(depth_texture, depth_view)
	}
}

/// Pass implementation. Indirectly render all shapes we have ownership over.
impl Pass for IndirectPass {
	fn states<'a>(&'a self) -> Vec<State<'a>> {
		vec![State {
			depth_stencil: Some(wgpu::DepthStencilState {
				bias: wgpu::DepthBiasState::default(),
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				format: wgpu::TextureFormat::Depth32Float,
				stencil: wgpu::StencilState::default(),
			}),
			program: &self.program,
		}]
	}

	fn encode(
		&mut self, encoder: &mut wgpu::CommandEncoder, pipelines: &Vec<&wgpu::RenderPipeline>, view: &wgpu::TextureView
	) {
		// update the uniforms
		self.update_uniforms();

		let mut buffer = Vec::new();

		// fill the command buffer with calls
		let mut draw_call_count = 0;
		for shape in self.shapes.iter() {
			for mesh in shape.blueprint.get_meshes().iter() {
				buffer.extend_from_slice(wgpu::util::DrawIndexedIndirect {
					base_index: mesh.first_index,
					base_instance: 0,
					instance_count: 1,
					vertex_count: mesh.vertex_count,
					vertex_offset: mesh.vertex_offset,
				}.as_bytes());

				draw_call_count += 1;
			}
		}

		let memory = self.memory.read().unwrap();

		// ensure immediate write to the buffer
		memory.get_page(self.indirect_command_buffer)
			.unwrap()
			.write_buffer(&self.indirect_command_buffer_node, &buffer);

		// handle the render pass stuff
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
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
					view: &self.depth_view,
				}),
				label: None,
			});

			render_pass.set_pipeline(pipelines[0]);

			render_pass.set_index_buffer(
				memory.get_page(self.indices_page).unwrap().get_buffer().slice(0..self.indices_page_written),
				wgpu::IndexFormat::Uint32
			);

			render_pass.set_vertex_buffer(
				0, memory.get_page(self.vertices_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
			);

			render_pass.set_vertex_buffer(
				1, memory.get_page(self.colors_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
			);

			render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

			// draw all the objects
			render_pass.multi_draw_indexed_indirect(
				memory.get_page(self.indirect_command_buffer).unwrap().get_buffer(), 0, draw_call_count
			);
		}
	}

	fn resize(&mut self, config: &wgpu::SurfaceConfiguration) {
		self.window_height = config.height;
		self.window_width = config.width;

		let (depth_texture, depth_view) = IndirectPass::create_depth_texture(&self.context, config);
		self.depth_texture = depth_texture;
		self.depth_view = depth_view;
	}
}

/// The way I implement indirect rendering requires seperate pages for each vertex attribute.
impl shape::BlueprintState for IndirectPass {
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
		name: shape::BlueprintDataKind,
		size: u64,
		align: u64,
		node_kind: NodeKind,
	) -> Result<Option<Node>, PageError> {
		let page = match name {
			shape::BlueprintDataKind::Color => self.colors_page,
			shape::BlueprintDataKind::Index => self.indices_page,
			shape::BlueprintDataKind::Vertex => self.vertices_page,
			_ => return Ok(None),
		};

		let mut memory = self.memory.write().unwrap();
		memory.get_page_mut(page).unwrap().allocate_node(size, align, node_kind)
			.and_then(|node| {
				Ok(Some(node))
			})
	}

	fn write_node(&mut self, name: shape::BlueprintDataKind, node: &Node, buffer: Vec<u8>) {
		let page = match name {
			shape::BlueprintDataKind::Color => self.colors_page,
			shape::BlueprintDataKind::Index => {
				self.indices_page_written += buffer.len() as u64;
				self.indices_page
			},
			shape::BlueprintDataKind::Vertex => {
				self.vertices_page_written += buffer.len() as u64;
				self.vertices_page
			},
			_ => return,
		};

		let mut memory = self.memory.write().unwrap();
		memory.write_buffer(page, node, buffer);
	}
}
