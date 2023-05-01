/// Stores the wgpu ShaderModule and uniform descriptor sets.
#[derive(Debug)]
pub struct Shader {
	/// The file name associated with the shader.
	pub file_name: String,
	/// The `wgpu` module created from the shader file.
	pub module: wgpu::ShaderModule,
	/// The stage of the shader (either vertex or fragment).
	pub stage: wgpu::ShaderStages,
}

impl PartialEq for Shader {
	fn eq(&self, other: &Self) -> bool {
		self.file_name == other.file_name
	}
}

impl Shader {
	/// Create a new shader.
	pub fn new(file_name: String, module: wgpu::ShaderModule, stage: wgpu::ShaderStages) -> Self {
		Shader {
			file_name,
			module,
			stage,
		}
	}
}
