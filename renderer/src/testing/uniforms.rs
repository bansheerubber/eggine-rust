use bytemuck::{ Pod, Zeroable, };

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct VertexUniform {
	pub perspective_matrix: [f32; 16],
	pub view_matrix: [f32; 16],
}
