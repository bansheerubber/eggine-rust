use glam::Vec3;
use std::rc::Rc;

use crate::ShapeBlueprint;

pub struct Shape {
	blueprint: Rc<ShapeBlueprint>,
	position: Vec3
}

impl Shape {
	pub fn new(blueprint: Rc<ShapeBlueprint>) -> Self {
		Shape {
			blueprint,
			position: Vec3::default(),
		}
	}

	pub fn write_indirect_buffer(&self, buffer: &mut Vec<u8>) {
		for mesh in self.blueprint.get_meshes().iter() {
			buffer.extend_from_slice(wgpu::util::DrawIndexedIndirect {
				base_index: mesh.first_index,
				base_instance: 0,
				instance_count: 1,
				vertex_count: mesh.vertex_count,
				vertex_offset: mesh.vertex_offset,
			}.as_bytes());
		}
	}
}
