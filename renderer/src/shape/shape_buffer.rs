use crate::memory_subsystem::{ Memory, PageUUID, };

/// Describes the buffers bound during an indirect call.
pub struct ShapeBuffer {
	/// Used for the `vertex_offset` of an indirect indexed draw call.
	pub highest_vertex_offset: i32,
	pub index_page: PageUUID,
	/// The total number of indices written into the index buffer. Used to calculate the `first_index` of an indirect
	/// indexed draw call.
	pub indices_written: u32,
	pub vertex_page: PageUUID,
}

impl ShapeBuffer {
	pub fn new(memory: &mut Memory) -> Self {
		ShapeBuffer {
			highest_vertex_offset: 0,
			index_page: memory.new_page(96_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
			indices_written: 0,
			vertex_page: memory.new_page(256_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST),
		}
	}
}
