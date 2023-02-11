use carton::Carton;
use streams::u8_io::U8ReadStream;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use super::{ Shader, Uniform, };

#[derive(Debug)]
pub enum ShaderError {
	FileError,
}

pub struct ShaderTable {
	shaders: HashMap<String, Shader>,
}

#[derive(Clone, Eq, PartialEq)]
enum TokenState {
	KindAndName,
	LayoutParameter(String),
	LayoutParameterName,
	None,
}

impl ShaderTable {
	pub fn new() -> Self {
		ShaderTable {
			shaders: HashMap::new(),
		}
	}

	pub fn load_shader_from_file(
		&mut self, file_name: &str, device: &wgpu::Device
	) -> Result<&Shader, ShaderError> {
		let Ok(mut file) = File::open(file_name) else {
			return Err(ShaderError::FileError);
		};

		let Ok(metadata) = std::fs::metadata(file_name) else {
			return Err(ShaderError::FileError);
		};

		let mut buffer = vec![0; metadata.len() as usize];
		if let Err(_) = file.read(&mut buffer) {
			return Err(ShaderError::FileError);
		}

		let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::util::make_spirv(&buffer),
    });

		let stage = if file_name.contains("frag") {
			wgpu::ShaderStages::FRAGMENT
		} else {
			wgpu::ShaderStages::VERTEX
		};

		self.shaders.insert(
			file_name.to_string(),
			Shader {
				file_name: file_name.to_string(),
				module,
				stage,
				uniforms: self.process_uniforms_from_file(&file_name.to_string().replace(".spv", ""))?,
			}
		);

		Ok(&self.shaders[file_name])
	}

	pub fn load_shader_from_carton(
		&mut self, file_name: &str, carton: &mut Carton, device: &wgpu::Device
	) -> Result<&Shader, ShaderError> {
		let mut file_stream = carton.get_file_data(file_name).unwrap();
		let binary_buffer = file_stream.read_vector(file_stream.file.get_size() as usize).unwrap().0;

		let mut file_stream = carton.get_file_data(&file_name.to_string().replace(".spv", "")).unwrap();
		let text_buffer = file_stream.read_vector(file_stream.file.get_size() as usize).unwrap().0;

		let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: None,
			source: wgpu::util::make_spirv(&binary_buffer),
    });

		let stage = if file_name.contains("frag") {
			wgpu::ShaderStages::FRAGMENT
		} else {
			wgpu::ShaderStages::VERTEX
		};

		self.shaders.insert(
			file_name.to_string(),
			Shader {
				file_name: file_name.to_string(),
				module,
				stage,
				uniforms: self.process_uniforms_from_buffer(&text_buffer)?,
			}
		);

		Ok(&self.shaders[file_name])
	}

	pub fn get_shader(&self, file_name: &str) -> &Shader {
		&self.shaders[file_name]
	}

	/// Parse uniforms from the raw text shader so the renderer can assemble DescriptorSetLayoutBindings
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

	/// Parse uniforms from the raw text shader so the renderer can assemble DescriptorSetLayoutBindings
	fn process_uniforms_from_buffer(&self, file_buffer: &Vec<u8>) -> Result<Vec<Uniform>, ShaderError> {
		// stores uniforms that are finished parsing
		let mut uniforms = Vec::new();

		// the uniform we are currently trying to parse
		let mut current_uniform = Uniform {
			binding: 0,
			kind: String::new(),
			name: String::new(),
			set: 0,
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
				&& &token_buffer.as_str()[token_buffer.len() - 1..] == "="
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
					"location" => {},
					"set" => {
						current_uniform.set = token_buffer.as_str()[..token_buffer.len() - 1].trim().parse().unwrap();
					},
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

				if split.len() < 4 {
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
						set: 0,
					};
				}

				// we finally finished parsing a uniform, so reset the state machine
				token_state = TokenState::None;
			}
		}

		Ok(uniforms)
	}
}
