use carton::Carton;
use clap::Parser;
use std::process::{ Command, Stdio, };
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(about = "Carton file utility program", override_usage = "cartonbin -s <directory> -o <file>\n       cartonbin -i <file>", arg_required_else_help = true)]
struct Args {
	/// Source directory for generating a carton file.
	#[arg(short, long, requires = "output", conflicts_with = "import")]
	source: Option<String>,

	/// Output file name for generated carton.
	#[arg(short, long, requires = "source", conflicts_with = "import")]
	output: Option<String>,

	/// Compile shader source code into SPIR-V using `glslc` before packing the carton.
	#[arg(long, requires = "source", conflicts_with = "import")]
	shaders: bool,

	/// Carton file name for importing a carton. Exports carton's contents to a directory of the same name as the carton.
	#[arg(short, long, exclusive = true)]
	import: Option<String>,
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
					if (!file_name.contains(".vert") && !file_name.contains(".frag")) || file_name.contains(".spv") {
						continue;
					}

					let shader_stage = if file_name.contains(".vert") {
						"vert"
					} else {
						"frag"
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

		let mut carton = Carton::default();
		carton.add_directory(&source);
		carton.to_file(&output);

		println!("Directory contents '{}' written to carton '{}'.", source, output);
	} else if args.import.is_some() {
		todo!();
	}
}
