use bytemuck::{ Pod, Zeroable, };

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub struct VertexUniform {
	pub view_perspective_matrix: [f32; 16],
}
