use carton::Carton;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::{ Pass, shape, };
use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::{ Memory, Node, NodeKind, PageError, PageUUID, };
use crate::shaders::ShaderTable;
use crate::state::State;

/// Renders `Shape`s using a indirect buffer.
#[derive(Debug)]
pub struct IndirectPass {
	blueprints: Vec<Rc<shape::Blueprint>>,
	context: Rc<WGPUContext>,
	fragment_shader: String,
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
	shapes: Vec<shape::Shape>,
	vertices_page: PageUUID,
	/// The amount of bytes written to the vertices page.
	vertices_page_written: u64,
	vertex_shader: String,
}

impl IndirectPass {
	pub fn new(boss: &mut Boss, carton: &mut Carton) -> Self {
		let memory = boss.get_memory();
		let mut memory = memory.write().unwrap();

		// create indirect command buffer page
		let indirect_command_buffer = memory.new_page(
			8_000_000, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST
		);

		// create node that fills entire indirect command buffer page
		let indirect_command_buffer_node = memory.get_page_mut(indirect_command_buffer)
			.unwrap()
			.allocate_node(8_000_000, 1, NodeKind::Buffer)
			.unwrap();

		// load the shaders from carton
		let fragment_shader = "data/main.vert.spv".to_string();
		let vertex_shader = "data/main.vert.spv".to_string();

		let shader_table = boss.get_shader_table();
		let mut shader_table = shader_table.write().unwrap();
		shader_table.load_shader_from_carton(&fragment_shader, carton).unwrap();
		shader_table.load_shader_from_carton(&vertex_shader, carton).unwrap();

		IndirectPass {
			blueprints: Vec::new(),
			context: boss.get_context().clone(),
			fragment_shader,
			highest_vertex_offset: 0,
			indices_page: memory.new_page(96_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
			indices_written: 0,
			indices_page_written: 0,
			indirect_command_buffer,
			indirect_command_buffer_node,
			memory: boss.get_memory().clone(),
			shapes: Vec::new(),
			vertices_page: memory.new_page(256_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
			vertices_page_written: 0,
			vertex_shader,
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
}

/// Pass implementation. Indirectly render all shapes we have ownership over.
impl Pass for IndirectPass {
	fn states<'a>(&self, shader_table: &'a ShaderTable) -> Vec<State<'a>> {
		vec![State {
			fragment_shader: shader_table.get_shader(&self.fragment_shader).unwrap(),
			vertex_shader: shader_table.get_shader(&self.vertex_shader).unwrap(),
		}]
	}

	fn encode(
		&self, encoder: &mut wgpu::CommandEncoder, pipelines: &Vec<&wgpu::RenderPipeline>, view: &wgpu::TextureView
	) {
		let mut buffer = Vec::new();

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

			render_pass.set_pipeline(pipelines[0]);

			render_pass.set_index_buffer(
				memory.get_page(self.indices_page).unwrap().get_buffer().slice(0..self.indices_page_written),
				wgpu::IndexFormat::Uint32
			);

			render_pass.set_vertex_buffer(
				0, memory.get_page(self.vertices_page).unwrap().get_buffer().slice(0..self.vertices_page_written)
			);

			// draw all the objects
			render_pass.multi_draw_indexed_indirect(
				memory.get_page(self.indirect_command_buffer).unwrap().get_buffer(), 0, draw_call_count
			);
		}
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
		self.highest_vertex_offset += highest_index as i32;
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
