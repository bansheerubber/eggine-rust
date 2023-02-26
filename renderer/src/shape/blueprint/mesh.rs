use std::rc::Rc;

use crate::memory_subsystem::{ Node, textures, };

/// Represents buffer data associated with a particular mesh.
#[derive(Debug)]
pub struct Mesh {
	/// Used for indirect rendering.
	pub first_index: u32,
	/// Points to the mesh's vertex indices. Indices are u16s.
	pub indices: Option<Node>,
	/// Points to the mesh's normal vec3 data. Normals are f32s.
	pub normals: Option<Node>,
	/// The texture of the mesh.
	pub texture: Option<Rc<textures::Texture>>,
	/// Points to the mesh's uv vec2 data. UVs are f32s.
	pub uvs: Option<Node>,
	/// Points to the mesh's vertex vec3 data. Vertices are f32s.
	pub vertices: Option<Node>,
	/// The amount of vertices in the mesh.
	pub vertex_count: u32,
	/// Used for indirect rendering.
	pub vertex_offset: i32,
}
