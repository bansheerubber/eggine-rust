use crate::memory_subsystem::{ Memory, PageUUID, };

/// Describes the buffers bound during an indirect call.
pub struct ShapeBuffer {
	pub index_page: PageUUID,
	pub vertex_page: PageUUID,
}

impl ShapeBuffer {
	pub fn new(memory: &mut Memory, device: &wgpu::Device) -> Self {
		ShapeBuffer {
			index_page: memory.new_page(96_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, device),
			vertex_page: memory.new_page(256_000_000, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, device),
		}
	}
}
