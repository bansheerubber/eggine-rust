use std::num::NonZeroU64;

use super::Uniform;

/// Stores the wgpu ShaderModule and uniform descriptor sets
#[derive(Debug)]
pub struct Shader {
	pub file_name: String,
	pub module: wgpu::ShaderModule,
	pub stage: wgpu::ShaderStages,
	pub uniforms: Vec<Uniform>,
}

impl std::hash::Hash for Shader {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.file_name.hash(state);
	}
}

impl PartialEq for Shader {
	fn eq(&self, other: &Self) -> bool {
		self.file_name == other.file_name
	}
}

impl Eq for Shader {}

impl Shader {
	pub fn new(file_name: String, module: wgpu::ShaderModule, stage: wgpu::ShaderStages) -> Self {
		Shader {
			file_name,
			module,
			stage,
			uniforms: Vec::new(),
		}
	}

	pub fn get_bind_group_entries(&self) -> Vec<Vec<wgpu::BindGroupLayoutEntry>> {
		if self.uniforms.len() == 0 {
			return Vec::new();
		}

		let mut entries = vec![
			Vec::new();
			self.uniforms.iter()
				.map(|x| x.set)
				.max()
				.unwrap() as usize + 1
		];

		for uniform in self.uniforms.iter() {
			let layout_entry = match uniform.kind.as_str() {
				"sampler" => {
					wgpu::BindGroupLayoutEntry {
						count: None,
						binding: uniform.binding,
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
						visibility: self.stage,
					}
				},
				"texture2D" => {
					wgpu::BindGroupLayoutEntry {
						count: None,
						binding: uniform.binding,
						ty: wgpu::BindingType::Texture {
							multisampled: false,
							sample_type: wgpu::TextureSampleType::Float {
								filterable: true,
							},
							view_dimension: wgpu::TextureViewDimension::D2,
						},
						visibility: self.stage,
					}
				},
				_ => {
					wgpu::BindGroupLayoutEntry {
						count: None,
						binding: uniform.binding,
						ty: wgpu::BindingType::Buffer {
							has_dynamic_offset: false,
							min_binding_size: NonZeroU64::new(1),
							ty: wgpu::BufferBindingType::Uniform,
						},
						visibility: self.stage,
					}
				}
			};

			entries[uniform.set as usize].push(layout_entry);
		}

		return entries;
	}
}
