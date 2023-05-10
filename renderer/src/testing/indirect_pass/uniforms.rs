use bytemuck::{ Pod, Zeroable, };

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub(crate) struct GlobalUniform {
	pub(crate) camera_position: [f32; 4],
	pub(crate) perspective_matrix: [f32; 16],
	pub(crate) view_matrix: [f32; 16],
}

/// The per-object information for multidraws.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct ObjectUniform {
	pub(crate) model_matrix: [f32; 16],
	pub(crate) texture_offset: [f32; 4],
	pub(crate) roughness: f32,
	pub(crate) bone_offset: u32,
	pub(crate) mesh_primitive_table_entry: u32,
	pub(crate) _padding: [f32; 1]
}
