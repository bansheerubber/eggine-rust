use super::Shader;

/// Helper object for creating `wgpu::BindGroupLayout`s that are used in the render pipeline.
pub struct Program<'a> {
	layouts: Vec<wgpu::BindGroupLayout>,
	pub shaders: Vec<&'a Shader>,
}

impl<'q> Program<'q> {
	/// Creates a program from the supplied shaders.
	pub fn new(shaders: Vec<&'q Shader>) -> Self {
		Program {
			layouts: Vec::new(),
			shaders,
		}
	}

	/// Takes the shaders and generates a `wgpu::BindGroupLayout` that is compatible with `wgpu::PipelineLayoutDescriptor`
	/// (see `wgpu::Device::create_pipeline_layout` documentation for the requirements).
	pub fn get_bind_group_layouts(&mut self, device: &wgpu::Device) -> Vec<&wgpu::BindGroupLayout> {
		self.layouts.clear();

		let mut layout_entry_sets = Vec::new();
		for shader in self.shaders.iter() {
			layout_entry_sets.push(shader.get_bind_group_entries());
		}

		// get the highest index in the map
		let highest_index = layout_entry_sets.iter()
			.map(|x| x.len())
			.max()
			.unwrap();

		// combine `BindGroupLayoutEntry` sets from all of the program's shaders
		for i in 0..highest_index {
			let mut entries: Vec<wgpu::BindGroupLayoutEntry> = Vec::new();
			for map in layout_entry_sets.iter() {
				if highest_index > map.len() {
					continue;
				}

				entries.extend(&map[i]); // combine set vectors into a single set vector
			}

			// create bind group layout from combined entry sets
			self.layouts.push(device.create_bind_group_layout(
				&wgpu::BindGroupLayoutDescriptor {
					entries: &entries,
					label: None,
				}
			));
		}

		// the render pipeline takes a vector of `wgpu::BindGroupLayout` borrows, so that's what we return here
		let mut layouts = Vec::new();
		for layout in self.layouts.iter() {
			layouts.push(layout);
		}

		return layouts;
	}
}
