use std::rc::Rc;

use crate::boss::WGPUContext;

use super::Shader;

/// Helper object for creating `wgpu::BindGroupLayout`s that are used in the render pipeline.
#[derive(Debug)]
pub struct ComputeProgram {
	pub context: Rc<WGPUContext>,
	pub layouts: Vec<wgpu::BindGroupLayout>,
	pub layout_entries: Vec<Vec<wgpu::BindGroupLayoutEntry>>,
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
	pub(crate) fn new(name: &str, shader: Shader, context: Rc<WGPUContext>) -> Self {
		let mut program = ComputeProgram {
			context,
			layouts: Vec::new(),
			layout_entries: Vec::new(),
			name: name.to_string(),
			shader,
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

		let layout_entry_sets = self.shader.get_bind_group_entries();

		// combine `BindGroupLayoutEntry` sets from all of the program's shaders
		for entries in layout_entry_sets {
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
