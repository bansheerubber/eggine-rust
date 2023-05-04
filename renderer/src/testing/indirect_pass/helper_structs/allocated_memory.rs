use std::collections::HashMap;

use crate::memory_subsystem::{ Node, PageUUID, };
use crate::testing::indirect_pass::DepthPyramidTexture;

/// Stores references to the pages allocated by the pass object.
#[derive(Debug)]
pub(crate) struct AllocatedMemory<'a> {
	pub(crate) bone_storage_page: PageUUID,
	pub(crate) bone_storage_node: Node,
	pub(crate) bone_indices: PageUUID,
	pub(crate) bone_weights: PageUUID,
	pub(crate) depth_pyramid: Vec<DepthPyramidTexture<'a>>,
	pub(crate) global_uniform_node: Node,
	pub(crate) indices_page: PageUUID,
	pub(crate) indirect_command_buffer: PageUUID,
	pub(crate) indirect_command_buffer_map: HashMap<u64, Vec<u8>>,
	pub(crate) indirect_command_buffer_node: Node,
	pub(crate) max_objects_per_batch: u64,
	pub(crate) normals_page: PageUUID,
	pub(crate) object_storage_page: PageUUID,
	pub(crate) object_storage_node: Node,
	pub(crate) positions_page: PageUUID,
	#[allow(dead_code)]
	pub(crate) test_node: Node,
	#[allow(dead_code)]
	pub(crate) test_page: PageUUID,
	pub(crate) uniforms_page: PageUUID,
	pub(crate) uvs_page: PageUUID,
}
