use std::rc::Rc;

use crate::boss::{ Boss, WGPUContext, };
use crate::memory_subsystem::{ Node, NodeKind, PageUUID, };
use crate::shape::Shape;

/// Renders `Shape`s using a indirect buffer.
pub struct IndirectPass {
	context: Rc<WGPUContext>,
	indirect_command_buffer: PageUUID,
	indirect_command_buffer_node: Node,
	shapes: Vec<Shape>,
}

impl IndirectPass {
	pub fn new(context: Rc<WGPUContext>, boss: &mut Boss) -> Self {
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

		IndirectPass {
			context,
			indirect_command_buffer,
			indirect_command_buffer_node,
			shapes: Vec::new(),
		}
	}
}
