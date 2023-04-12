use carton::Carton;
use clap::Parser;
use std::process::{ Command, Stdio, };
use std::io::{ Read, Write, };
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(about = "Carton file utility program", override_usage = "cartonbin -s <directory> -o <file>\n       cartonbin -i <file>", arg_required_else_help = true)]
struct Args {
	/// Source directory for generating a carton file.
	#[arg(short, long, requires = "output", conflicts_with = "import")]
	source: Option<String>,

	/// Output file name for generated carton.
	#[arg(short, long, requires = "source")]
	output: Option<String>,

	/// Compile shader source code into SPIR-V using `glslc` before packing the carton.
	#[arg(long, requires = "source", conflicts_with = "import")]
	shaders: bool,

	/// Carton file name for importing a carton. Exports carton's contents to a directory of the same name as the carton.
	#[arg(short, long)]
	import: Option<String>,

	/// Overwrites files with conflicting names on carton import.
	#[arg(long)]
	overwrite: bool,

	/// Compresses files while generating a carton file.
	#[arg[short, long, requires = "source", conflicts_with = "import"]]
	compress: bool,
}

#[derive(Debug)]
struct CommandError(String);

// run a command and return the stdout
fn run_command(command: &mut Command) -> Result<String, CommandError> {
	let child = command.stdout(Stdio::piped())
		.spawn();

	// make sure the command spawned
	let child = match child {
		Err(error) => {
			return Err(CommandError(format!("child spawn error: {:?}", error)));
		},
		Ok(child) => child,
	};

	// make sure we got an output
	let child = match child.wait_with_output() {
		Err(error) => {
			return Err(CommandError(format!("child wait error: {:?}", error)));
		},
		Ok(child) => child,
	};

	if !child.status.success() {
		return Err(CommandError(format!("child returned non-zero exit code: {:?}", child.status.code())));
	}

	// make sure we decode the utf8 correctly
	match String::from_utf8(child.stdout) {
		Err(error) => {
			return Err(CommandError(format!("utf8 decode error: {:?}", error)));
		},
		Ok(stdout) => Ok(stdout),
	}
}

fn main() {
	let args = Args::parse();

	if args.source.is_some() {
		let source = args.source.unwrap();
		let output = args.output.unwrap();
		let shaders = args.shaders;

		// go through directory and compile shaders using glslc
		if shaders {
			for entry in WalkDir::new(source.clone()) {
				let entry = entry.unwrap();
				if entry.metadata().unwrap().is_file() {
					let file_name = entry.path().to_str().unwrap();
					if (!file_name.contains(".vert") && !file_name.contains(".frag") && !file_name.contains(".comp"))
						|| file_name.contains(".spv")
					{
						continue;
					}

					let shader_stage = if file_name.contains(".vert") {
						"vert"
					} else if file_name.contains(".frag") {
						"frag"
					} else {
						"comp"
					};

					let output = run_command(
						Command::new("glslc")
							.arg(file_name)
							.arg(format!("-fshader-stage={}", shader_stage))
							.arg("-o")
							.arg(format!("{}.spv", file_name))
					).unwrap().trim().to_string();

					if output.len() != 0 {
						println!("{}", output.trim());
					}
				}
			}
		}

		let mut carton = Carton::new(args.compress);
		carton.add_directory(&source);
		carton.to_file(&output);

		println!("Directory contents '{}' written to carton '{}'.", source, output);
	} else if let Some(import) = args.import {
		let output_directory = if args.output.is_some() {
			format!("{}/", args.output.unwrap())
		} else {
			format!("{}/", import.replace(".carton", ""))
		};

		// check if import path is valid
		let path = std::path::Path::new(&import);
		if path.is_dir() {
			eprintln!("Error: The import file is not a carton file.");
			return;
		} else if !path.exists() {
			eprintln!("Error: The import file does not exist.");
			return;
		};

		// check if output directory is valid
		if output_directory.len() > 0 && !args.overwrite {
			let path = std::path::Path::new(&output_directory);
			if path.is_dir() {
				eprintln!("Error: The output directory already exists.");
				return;
			}
		}

		let carton = Carton::read(&import).unwrap();
		for file_name in carton.get_file_names().unwrap() {
			match carton.get_file_data(file_name) {
				Ok(mut stream) => {
					let output_file_name = format!("{}{}", output_directory, file_name);
					let output_path = std::path::Path::new(&output_file_name);
					if output_path.exists() && !args.overwrite {
						eprintln!("Error: The file '{}' already exists", output_file_name);
						continue;
					}

					if let Err(error) = std::fs::create_dir_all(output_path.parent().unwrap()) {
						eprintln!("Error: Could not create directories for file '{}': {:?}", output_file_name, error);
						continue;
					}

					let Ok(mut file) = std::fs::File::create(output_path) else {
						eprintln!("Error: Could not create output file '{}'", output_file_name);
						continue;
					};

					let file_parameters = carton.get_file(file_name).unwrap();
					let mut buffer = Vec::new();
					buffer.resize(file_parameters.get_size() as usize, 0);

					if let Err(error) = stream.read(&mut buffer) {
						eprintln!("Error: Could not read data from file '{}': {:?}", file_name, error);
						continue;
					}

					if let Err(error) = file.write(&buffer) {
						eprintln!("Error: Could not write data to file '{}': {:?}", output_file_name, error);
						continue;
					}
				},
				Err(error) => {
					eprintln!("Could not read file '{}': {:?}", file_name, error);
				},
			}
		}
	}
}
