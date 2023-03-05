use std::rc::Rc;

use super::Mesh;

/// Represents a node in a GLTF tree.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Node {
	pub children: Vec<Rc<Node>>,
	pub data: NodeData,
	pub parent: Option<Rc<Node>>,
	pub transform: glam::Mat4,
}

/// Stores GLTF data that the eggine cares about. Node types that we don't care about (cameras, lights, etc) are aliased
/// into the `Empty` variant.
#[derive(Debug)]
pub enum NodeData {
	Empty,
	Mesh(Rc<Mesh>),
}
