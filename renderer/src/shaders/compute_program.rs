use super::Shader;

/// Helper object for creating `wgpu::BindGroupLayout`s that are used in the render pipeline.
#[derive(Debug)]
pub struct ComputeProgram {
	name: String,
	pub shader: Shader,
}

impl PartialEq for ComputeProgram {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
	}
}

impl ComputeProgram {
	/// Creates a program from the supplied shaders.
	pub(crate) fn new(name: &str, shader: Shader) -> Self {
		ComputeProgram {
			name: name.to_string(),
			shader,
		}
	}

	pub fn get_name(&self) -> &str {
		&self.name
	}
}
