use std::num::NonZeroU32;

use super::Uniform;

/// Stores the wgpu ShaderModule and uniform descriptor sets.
#[derive(Debug)]
pub struct Shader {
	/// The file name associated with the shader.
	pub file_name: String,
	/// The `wgpu` module created from the shader file.
	pub module: wgpu::ShaderModule,
	/// The stage of the shader (either vertex or fragment).
	pub stage: wgpu::ShaderStages,
	/// The uniforms parsed from the shader source file.
	pub uniforms: Vec<Uniform>,
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
			uniforms: Vec::new(),
		}
	}

	/// Generate bind group layout entries based on the shader's uniforms. The index of the first dimension of the output
	/// is the uniform's set.
	pub fn get_bind_group_entries(&self) -> Vec<Vec<wgpu::BindGroupLayoutEntry>> {
		if self.uniforms.len() == 0 {
			return Vec::new();
		}

		// initialize entries vector
		let mut entries = vec![
			Vec::new();
			self.uniforms.iter()
				.map(|x| x.set)
				.max()
				.unwrap() as usize + 1
		];

		for uniform in self.uniforms.iter() {
			// handle array types
			let count = if uniform.kind.contains("[") {
				NonZeroU32::new(
					*&uniform.kind[uniform.kind.find("[").unwrap() + 1..uniform.kind.find("]").unwrap()]
						.parse::<u32>()
						.unwrap()
				)
			} else {
				None
			};

			let layout_entry = match uniform.kind.as_str() {
				"sampler" => { // sampler uniform
					wgpu::BindGroupLayoutEntry {
						count,
						binding: uniform.binding,
						ty: wgpu::BindingType::Sampler(
							wgpu::SamplerBindingType::Filtering // TODO what should this be
						),
						visibility: self.stage,
					}
				},
				"texture2DArray" => {
					wgpu::BindGroupLayoutEntry {
						count,
						binding: uniform.binding,
						ty: wgpu::BindingType::Texture {
							multisampled: false, // TODO what should this be
							sample_type: wgpu::TextureSampleType::Float {
								filterable: true, // TODO what should this be
							},
							view_dimension: wgpu::TextureViewDimension::D2Array, // TODO what should this be
						},
						visibility: self.stage,
					}
				},
				"texture2D" => { // texture uniform w/ some default options
					wgpu::BindGroupLayoutEntry {
						count,
						binding: uniform.binding,
						ty: wgpu::BindingType::Texture {
							multisampled: false, // TODO what should this be
							sample_type: wgpu::TextureSampleType::Float {
								filterable: true, // TODO what should this be
							},
							view_dimension: wgpu::TextureViewDimension::D2, // TODO what should this be
						},
						visibility: self.stage,
					}
				},
				_ => {
					if uniform.storage {
						wgpu::BindGroupLayoutEntry { // a normal uniform
							count,
							binding: uniform.binding,
							ty: wgpu::BindingType::Buffer {
								has_dynamic_offset: false, // TODO what should this be
								min_binding_size: None, // TODO what should this be
								ty: wgpu::BufferBindingType::Storage {
									read_only: true, // TODO what should this be
								},
							},
							visibility: self.stage,
						}
					} else {
						wgpu::BindGroupLayoutEntry { // a normal uniform
							count,
							binding: uniform.binding,
							ty: wgpu::BindingType::Buffer {
								has_dynamic_offset: false, // TODO what should this be
								min_binding_size: None, // TODO what should this be
								ty: wgpu::BufferBindingType::Uniform,
							},
							visibility: self.stage,
						}
					}
				}
			};

			entries[uniform.set as usize].push(layout_entry);
		}

		return entries;
	}
}
