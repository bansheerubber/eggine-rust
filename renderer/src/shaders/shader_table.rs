use carton::Carton;
use streams::u8_io::U8ReadStream;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::{ ComputeProgram, Program, Shader, };

#[derive(Debug)]
pub enum ShaderError {
	FileError,
	UnrecognizedExtension,
}

/// Manages shader loading, and stores/manages loaded shaders.
#[derive(Debug)]
pub struct ShaderTable {
	/// Map of program names to objects.
	compute_programs: HashMap<String, Rc<ComputeProgram>>,
	context: Rc<WGPUContext>,
	/// Map of program names to objects.
	render_programs: HashMap<String, Rc<Program>>,
}

impl ShaderTable {
	/// Creates a new `ShaderTable`.
	pub fn new(context: Rc<WGPUContext>) -> Self {
		ShaderTable {
			compute_programs: HashMap::new(),
			context,
			render_programs: HashMap::new(),
		}
	}

	/// Creates a program from the supplied shaders and inserts into the shader table.
	pub fn create_render_program(&mut self, name: &str, fragment_shader: Shader, vertex_shader: Shader) -> Rc<Program> {
		let program = Rc::new(Program::new(name, fragment_shader, vertex_shader));
		self.render_programs.insert(name.to_string(), program.clone());
		return program;
	}

	/// Creates a program from the supplied shader and inserts into the shader table.
	pub fn create_compute_program(&mut self, name: &str, compute_shader: Shader) -> Rc<ComputeProgram> {
		let program = Rc::new(ComputeProgram::new(name, compute_shader));
		self.compute_programs.insert(name.to_string(), program.clone());
		return program;
	}

	/// Loads a SPIR-V shader from file. Expects a file named `[name].(frag|vert).spv` and an associated source file named
	/// `[name].(frag|vert)`. The source file is parsed for its uniform information.
	pub fn load_shader_from_file(&mut self, file_name: &str) -> Result<Shader, ShaderError> {
		// determine stage based on file name (".frag" for fragment shaders, ".vert" for vertex shaders)
		let stage = if file_name.contains(".frag.spv") {
			wgpu::ShaderStages::FRAGMENT
		} else if file_name.contains(".vert.spv") {
			wgpu::ShaderStages::VERTEX
		} else if file_name.contains(".comp.spv") {
			wgpu::ShaderStages::COMPUTE
		} else {
			return Err(ShaderError::UnrecognizedExtension);
		};

		// try opening the file
		let Ok(mut file) = File::open(file_name) else {
			return Err(ShaderError::FileError);
		};

		// needed for the file size
		let Ok(metadata) = std::fs::metadata(file_name) else {
			return Err(ShaderError::FileError);
		};

		// read binary data into buffer
		let mut buffer = vec![0; metadata.len() as usize];
		if let Err(_) = file.read(&mut buffer) {
			return Err(ShaderError::FileError);
		}

		// create the shader module from SPIR-V
		let module = self.context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::util::make_spirv(&buffer),
    });

		// create a new shader helper object, also parse uniforms from source code
		Ok(Shader {
			file_name: file_name.to_string(),
			module,
			stage,
		})
	}

	/// Loads a SPIR-V shader from a carton. Expects a file named `[name].(frag|vert).spv` and an associated source file
	/// named `[name].(frag|vert)`. The source file is parsed for its uniform information.
	pub fn load_shader_from_carton(&mut self, file_name: &str, carton: &mut Carton) -> Result<Shader, ShaderError> {
		// determine stage based on file name (".frag" for fragment shaders, ".vert" for vertex shaders)
		let stage = if file_name.contains(".frag.spv") {
			wgpu::ShaderStages::FRAGMENT
		} else if file_name.contains(".vert.spv") {
			wgpu::ShaderStages::VERTEX
		} else if file_name.contains(".comp.spv") {
			wgpu::ShaderStages::COMPUTE
		} else {
			return Err(ShaderError::UnrecognizedExtension);
		};

		// open SPIR-V file stream
		let mut file_stream = carton.get_file_data(file_name).unwrap();
		let binary_buffer = file_stream.read_vector(file_stream.file.get_size() as usize).unwrap().0;

		// create the shader module from SPIR-V
		let module = self.context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::util::make_spirv(&binary_buffer),
    });

		// create a new shader helper object, also parse uniforms from source code
		Ok(Shader {
			file_name: file_name.to_string(),
			module,
			stage,
		})
	}

	/// Retreives the shader associated with the supplied file name.
	pub fn get_program(&self, name: &str) -> Option<Rc<Program>> {
		self.render_programs.get(name).cloned()
	}
}
