use super::Shader;

/// Helper object for creating `wgpu::BindGroupLayout`s that are used in the render pipeline.
#[derive(Debug)]
pub struct Program {
	pub fragment_shader: Shader,
	name: String,
	pub vertex_shader: Shader,
}

impl PartialEq for Program {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
	}
}

impl Program {
	/// Creates a program from the supplied shaders.
	pub(crate) fn new(name: &str, fragment_shader: Shader, vertex_shader: Shader) -> Self {
		Program {
			fragment_shader,
			name: name.to_string(),
			vertex_shader,
		}
	}

	pub fn get_name(&self) -> &str {
		&self.name
	}
}
