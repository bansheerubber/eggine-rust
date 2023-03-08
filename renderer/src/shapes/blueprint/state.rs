use std::rc::Rc;

use crate::memory_subsystem::{ Node, NodeKind, PageError, textures, };

use super::DataKind;

/// Controls how a `Blueprint` allocates memory and helps with calculating `first_index` and `vertex_offset`
/// properties for `Mesh`s
pub trait State {
	/// Calculates the first index of the mesh, which a `Mesh` uses to index into the index buffer.
	///
	/// * `num_indices` - The amount of index values written in the index buffer. Additional disambiguation: index values
	/// are repeated in the index buffer, and `num_indices` includes all the repeats since it is the total number of
	/// indices written into the index buffer.
	fn calc_first_index(&mut self, num_indices: u32) -> u32;

	/// Calculates the vertex offset of the mesh, which a 'Mesh' uses to calculate the base vertex index used to index
	/// into the vertex buffer.
	///
	/// * `last_highest_index` - The highest index value from the mesh that the `vertex_offset` will be assigned to. If an
	/// index buffer uses numbers within the range 0..=32, then the `highest_index` passed to this function should be
	/// `32`.
	fn calc_vertex_offset(&mut self, highest_index: i32) -> i32;

	/// Prepares memory for the next mesh.
	fn prepare_mesh_pages(&mut self);

	/// Gets the node that the blueprint will store vertex attribute information into.
	///
	/// * `name`      - A descriptor for the kind of data stored in the node. Vec3 vertex information would be stored in a
	/// separate node from Vec4 color information, with the correct node specified by `name`. The `name` does not describe
	/// the GLSL type of data stored, so additional parameters are necessary for node allocation.
	/// * `size`      - The expected size of the returned node.
	/// * `align`     - The expected alignment of the returned node.
	/// * `node_kind` - The expected kind of the returned node.
	///
	/// # Return value
	/// Since an implementation of this function may allocate new nodes, a `PageError` is returned so the blueprint can
	/// handle them gracefully. The onus of pretty-printing memory error debug is on callers.
	/// If the unwrapped result is `None`, then the `BlueprintState` implementation does not support storing the kind
	/// of data described by `name`, and `Blueprint` should throw away any such data it loaded.
	fn get_named_node(
		&self,
		name: DataKind,
		size: u64,
		align: u64,
		node_kind: NodeKind,
	) -> Result<Option<Node>, PageError>;

	/// Wrapper function for writing data into the specified node.
	fn write_node(&mut self, name: DataKind, node: &Node, buffer: Vec<u8>);

	/// Gets the none texture.
	fn get_none_texture(&mut self) -> Rc<textures::Texture>;

	/// Gets the vertex attributes that this `State` needs to be allocated. `DataKind::Index` does not count as a vertex
	/// attribute that can be allocated.
	fn required_attributes(&self) -> Vec<DataKind>;
}
