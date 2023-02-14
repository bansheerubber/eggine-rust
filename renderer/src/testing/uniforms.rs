#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
struct VertexUniform {
	perspective_matrix: [f32; 16],
}
