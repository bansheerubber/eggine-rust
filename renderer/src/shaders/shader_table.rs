use carton::Carton;
use streams::u8_io::U8ReadStream;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::{ ComputeProgram, Program, Shader, Uniform, };

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

/// Describes the state of the uniform parsing state machine.
#[derive(Clone, Eq, PartialEq)]
enum TokenState {
	KindAndName,
	LayoutParameter(String),
	LayoutParameterName,
	None,
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
		let program = Rc::new(Program::new(name, fragment_shader, vertex_shader, self.context.clone()));
		self.render_programs.insert(name.to_string(), program.clone());
		return program;
	}

	/// Creates a program from the supplied shader and inserts into the shader table.
	pub fn create_compute_program(&mut self, name: &str, compute_shader: Shader) -> Rc<ComputeProgram> {
		let program = Rc::new(ComputeProgram::new(name, compute_shader, self.context.clone()));
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
			uniforms: self.process_uniforms_from_file(&file_name.to_string().replace(".spv", ""))?,
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

		// open source code file stream
		let mut file_stream = carton.get_file_data(&file_name.to_string().replace(".spv", "")).unwrap();
		let text_buffer = file_stream.read_vector(file_stream.file.get_size() as usize).unwrap().0;

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
			uniforms: self.process_uniforms_from_buffer(&text_buffer)?,
		})
	}

	/// Retreives the shader associated with the supplied file name.
	pub fn get_program(&self, name: &str) -> Option<Rc<Program>> {
		self.render_programs.get(name).cloned()
	}

	/// Parse uniforms from the shader source codeso the renderer can assemble `wgpu::DescriptorSetLayoutBindings`.
	fn process_uniforms_from_file(&self, file_name: &str) -> Result<Vec<Uniform>, ShaderError> {
		// open the file
		let Ok(mut file) = File::open(file_name) else {
			return Err(ShaderError::FileError);
		};

		// read the file into a u8 buffer
		let mut file_buffer = Vec::new();
		if let Err(_) = file.read_to_end(&mut file_buffer) {
			return Err(ShaderError::FileError);
		}

		self.process_uniforms_from_buffer(&file_buffer)
	}

	/// Parse uniforms from the shader source code so the renderer can assemble `wgpu::DescriptorSetLayoutBindings`.
	fn process_uniforms_from_buffer(&self, file_buffer: &Vec<u8>) -> Result<Vec<Uniform>, ShaderError> {
		// stores uniforms that are finished parsing
		let mut uniforms = Vec::new();

		// the uniform we are currently trying to parse
		let mut current_uniform = Uniform {
			binding: 0,
			kind: String::new(),
			name: String::new(),
			readonly: true,
			set: 0,
			storage: false,
		};

		let mut token_state = TokenState::None;
		let mut token_buffer = String::new();
		for character in file_buffer.iter() {
			// push whitespace if we find a new line
			if *character == 10 || *character == 13 {
				token_buffer.push(' ');
				continue;
			}

			// push a character into the buffer
			token_buffer.push(*character as char);

			// this if statement describes the state machine used to parse uniform declarations
			if token_state == TokenState::None
				&& token_buffer.len() > 7
				&& &token_buffer.as_str()[token_buffer.len() - 7..] == "layout("
			{
				// find "layout" token
				token_state = TokenState::LayoutParameterName;
				token_buffer = String::new();
			} else if token_state == TokenState::LayoutParameterName
				&& (
					&token_buffer.as_str()[token_buffer.len() - 1..] == "="
					|| &token_buffer.as_str()[token_buffer.len() - 1..] == ","
				)
			{
				// find "binding", "set" tokens
				let command = &token_buffer.as_str()[..token_buffer.len() - 1].trim();
				token_state = TokenState::LayoutParameter(command.to_string());
				token_buffer = String::new();
			} else if let TokenState::LayoutParameter(parameter_name) = &token_state {
				// find value assigned to "binding", "set"
				if &token_buffer.as_str()[token_buffer.len() - 1..] != ")"
					&& &token_buffer.as_str()[token_buffer.len() - 1..] != ","
				{
					continue;
				}

				// match parameter name to its value in the `current_uniform`
				match parameter_name.as_str() {
					"binding" => {
						current_uniform.binding = token_buffer.as_str()[..token_buffer.len() - 1].trim().parse().unwrap();
					},
					"set" => {
						current_uniform.set = token_buffer.as_str()[..token_buffer.len() - 1].trim().parse().unwrap();
					},
					"std140" | "location" | "local_size_x" | "local_size_y" | "local_size_z" => {},
					_ => panic!("Could not parse layout parameter '{}'", parameter_name),
				}

				// if we encounter a ')' then we know we're done with the uniform, otherwise we should start searching for
				// the next parameter name
				if &token_buffer.as_str()[token_buffer.len() - 1..] == ")" {
					token_state = TokenState::KindAndName;
				} else {
					token_state = TokenState::LayoutParameterName;
				}

				token_buffer = String::new(); // reset the token buffer
			} else if token_state == TokenState::KindAndName {
				// find uniform types and names
				let split = token_buffer.trim().split(" ").collect::<Vec<&str>>();

				// if we find a semicolon, short circuit
				if split.len() < 4 && !split[split.len() - 1].ends_with(";") {
					continue;
				}

				// only store into current uniform if we're dealing with one
				if split[0] == "uniform" {
					let kind = String::from(split[1]);
					let name = String::from(split[2].replace(";", ""));

					current_uniform.kind = kind;
					current_uniform.name = name;

					uniforms.push(current_uniform);

					// reinitialize current uniform
					current_uniform = Uniform {
						binding: 0,
						kind: String::new(),
						name: String::new(),
						readonly: true,
						set: 0,
						storage: false,
					};
				} else if split.len() > 1 && (split[0] == "buffer" || split[1] == "buffer") {
					let (offset, readonly) = if split[0] == "readonly" {
						(1, true)
					} else {
						(0, false)
					};

					let kind = String::from(split[0 + offset]);
					let name = String::from(split[1 + offset].replace(";", ""));

					current_uniform.kind = kind;
					current_uniform.name = name;
					current_uniform.storage = true;
					current_uniform.readonly = readonly;

					uniforms.push(current_uniform);

					// reinitialize current uniform
					current_uniform = Uniform {
						binding: 0,
						kind: String::new(),
						name: String::new(),
						readonly,
						set: 0,
						storage: false,
					};
				}

				// we finally finished parsing a uniform, so reset the state machine
				token_state = TokenState::None;
			}
		}

		Ok(uniforms)
	}
}
