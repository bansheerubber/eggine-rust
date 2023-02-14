use std::rc::Rc;

use crate::boss::WGPUContext;

use super::Shader;

/// Helper object for creating `wgpu::BindGroupLayout`s that are used in the render pipeline.
#[derive(Debug)]
pub struct Program {
	pub context: Rc<WGPUContext>,
	pub fragment_shader: Shader,
	pub layouts: Vec<wgpu::BindGroupLayout>,
	pub layout_entries: Vec<Vec<wgpu::BindGroupLayoutEntry>>,
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
	pub(crate) fn new(name: &str, fragment_shader: Shader, vertex_shader: Shader, context: Rc<WGPUContext>) -> Self {
		let mut program = Program {
			context,
			fragment_shader,
			layouts: Vec::new(),
			layout_entries: Vec::new(),
			name: name.to_string(),
			vertex_shader,
		};

		program.create_bind_group_layouts();

		return program;
	}

	/// Gets a vector containing references to the `wgpu::BindGroupLayout`s that this program created.
	pub fn get_bind_group_layouts(&self) -> Vec<&wgpu::BindGroupLayout> {
		let mut output = Vec::new();
		for layout in self.layouts.iter() {
			output.push(layout);
		}

		return output;
	}

	pub fn get_bind_group_layout_entries(&self) -> &Vec<Vec<wgpu::BindGroupLayoutEntry>> {
		&self.layout_entries
	}

	/// Takes the shaders and generates a `wgpu::BindGroupLayout` that is compatible with `wgpu::PipelineLayoutDescriptor`
	/// (see `wgpu::Device::create_pipeline_layout` documentation for the requirements).
	fn create_bind_group_layouts(&mut self) {
		self.layouts.clear();

		let layout_entry_sets = vec![
			self.vertex_shader.get_bind_group_entries(), self.fragment_shader.get_bind_group_entries()
		];

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
			self.layouts.push(self.context.device.create_bind_group_layout(
				&wgpu::BindGroupLayoutDescriptor {
					entries: &entries,
					label: None,
				}
			));

			self.layout_entries.push(entries);
		}
	}

	pub fn get_name(&self) -> &str {
		&self.name
	}
}
