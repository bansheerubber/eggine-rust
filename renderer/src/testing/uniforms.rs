use bytemuck::{ Pod, Zeroable, };

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct GlobalUniform {
	pub camera_position: [f32; 4],
	pub perspective_matrix: [f32; 16],
	pub view_matrix: [f32; 16],
}

/// The per-object information for multidraws.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ObjectUniform {
	pub model_matrix: [f32; 16],
	pub texture_offset: [f32; 4],
	pub roughness: [f32; 4],
}
