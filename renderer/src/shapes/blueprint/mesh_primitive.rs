use bytemuck::{ Pod, Zeroable, };

use crate::memory_subsystem::Node;
use super::Material;

#[derive(Debug)]
pub enum MeshPrimitiveKind {
	Invalid,
	Triangle,
}

/// Represents a .gltf mesh primitive and its data..
#[derive(Debug)]
pub struct MeshPrimitive {
	/// Used for indirect rendering.
	pub first_index: u32,
	/// Points to the primitives's vertex indices. Indices are u16s.
	pub indices: Option<Node>,
	/// The kind of data the primitive represents.
	pub kind: MeshPrimitiveKind,
	/// The material used to render the primitve.
	pub material: Material,
	/// Points to the entry in the mesh primitive table GPU buffer.
	pub mesh_primitive_table_id: u32,
	/// Points to the primitives's normal vec3 data. Normals are f32s.
	pub normals: Option<Node>,
	/// Points to the primitives's vertex vec3 data. Vertices are f32s.
	pub positions: Option<Node>,
	/// Points to the primitives's uv vec2 data. UVs are f32s.
	pub uvs: Option<Node>,
	/// The amount of vertices in the primitive.
	pub vertex_count: u32,
	/// Used for indirect rendering.
	pub vertex_offset: i32,
}

/// Struct that represents mesh primitive metadata stored on a GPU buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct MeshPrimitiveTableEntry {
	pub radius: f32,
	pub vertex_count: u32,
	pub vertex_offset: u32,
	pub _padding: [f32; 1]
}
