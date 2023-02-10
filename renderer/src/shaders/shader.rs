use std::num::NonZeroU64;

use super::Uniform;

/// Stores the wgpu ShaderModule and uniform descriptor sets
#[derive(Debug)]
pub struct Shader {
	pub module: wgpu::ShaderModule,
	pub uniforms: Vec<Uniform>,
}

impl Shader {
	pub fn new(module: wgpu::ShaderModule) -> Self {
		Shader {
			module,
			uniforms: Vec::new(),
		}
	}

	pub fn get_bind_group_layout(&self) -> Vec<Vec<wgpu::BindGroupLayoutEntry>> {
		let mut entries = vec![
			Vec::new();
			self.uniforms.iter()
				.map(|x| x.set)
				.max()
				.unwrap() as usize + 1
		];

		for uniform in self.uniforms.iter() {
			entries[uniform.set as usize].push(wgpu::BindGroupLayoutEntry {
				count: None,
				binding: uniform.binding,
				ty: wgpu::BindingType::Buffer {
					has_dynamic_offset: false,
					min_binding_size: NonZeroU64::new(1),
					ty: wgpu::BufferBindingType::Uniform,
				},
				visibility: wgpu::ShaderStages::FRAGMENT,
			});
		}

		return entries;
	}
}
