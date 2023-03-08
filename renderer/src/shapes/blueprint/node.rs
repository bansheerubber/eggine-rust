use std::cell::RefCell;
use std::rc::Rc;

use super::Mesh;

/// Represents a node in a GLTF tree.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Node {
	pub children: Vec<Rc<RefCell<Node>>>,
	pub data: NodeData,
	/// TODO replace with eggine generated ID?
	pub gltf_id: usize,
	pub local_transform: glam::Mat4,
	pub parent: Option<Rc<RefCell<Node>>>,
	pub transform: glam::Mat4,
}

impl Node {
	pub fn get_mesh(&self) -> Option<Rc<Mesh>> {
		match &self.data {
			NodeData::Mesh(mesh) => Some(mesh.clone()),
			_ => None,
		}
	}

	/// Accumulates together the transforms from the `Node`'s parents.
	pub fn accumulate_transform(node: Option<Rc<RefCell<Node>>>, local_transform: glam::Mat4) -> glam::Mat4 {
		let mut accumulator = local_transform;

		let mut next = node;
		loop {
			if let Some(temp) = next {
				let parent_transform = temp.borrow().local_transform;
				accumulator = parent_transform.mul_mat4(&accumulator);
				next = temp.borrow().parent.clone();
			} else {
				break;
			}
		}

		accumulator
	}
}

/// Stores GLTF data that the eggine cares about. Node types that we don't care about (cameras, lights, etc) are aliased
/// into the `Empty` variant.
#[derive(Debug)]
pub enum NodeData {
	Bone,
	Empty,
	Mesh(Rc<Mesh>),
}
