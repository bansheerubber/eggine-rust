use super::Shader;

pub struct Program<'a> {
	layouts: Vec<wgpu::BindGroupLayout>,
	pub shaders: Vec<&'a Shader>,
}

impl<'q> Program<'q> {
	/// Easy way to create a program;
	pub fn new(shaders: Vec<&'q Shader>) -> Self {
		Program {
			layouts: Vec::new(),
			shaders,
		}
	}

	/// Takes the shaders and generates a `BindGroupLayout` that is compatible with `PipelineLayoutDescriptor` (see
	/// `wgpu::Device::create_pipeline_layout` documentation for the requirements)
	pub fn get_bind_group_layouts(&mut self, device: &wgpu::Device) -> Vec<&wgpu::BindGroupLayout> {
		self.layouts.clear();

		let mut maps = Vec::new();
		for shader in self.shaders.iter() {
			maps.push(shader.get_bind_group_entries());
		}

		// get the highest index in the map
		let highest_index = maps.iter()
			.map(|x| x.len())
			.max()
			.unwrap();

		for i in 0..highest_index {
			let mut entries: Vec<wgpu::BindGroupLayoutEntry> = Vec::new();
			for map in maps.iter() {
				if highest_index > map.len() {
					continue;
				}

				entries.extend(&map[i]);
			}

			self.layouts.push(device.create_bind_group_layout(
				&wgpu::BindGroupLayoutDescriptor {
					entries: &entries,
					label: None,
				}
			));
		}

		let mut layouts = Vec::new();
		for layout in self.layouts.iter() {
			layouts.push(layout);
		}

		return layouts;
	}
}
