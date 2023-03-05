use std::rc::Rc;

use super::MeshPrimitive;

/// Represents a .gltf mesh object. `Mesh`es store their own transforms since the concept of a gltf node will not have a
/// direct representation in the eggine rendering system.
#[derive(Debug)]
pub struct Mesh {
	pub primitives: Vec<MeshPrimitive>,
}
