use carton::Carton;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Carton file utility program", override_usage = "cartonbin -s <directory> -o <file>\n       cartonbin -i <file>", arg_required_else_help = true)]
struct Args {
	/// Source directory for generating a carton file.
	#[arg(short, long, requires = "output", conflicts_with = "import")]
	source: Option<String>,

	/// Output file name for generated carton.
	#[arg(short, long, requires = "source", conflicts_with = "import")]
	output: Option<String>,

	/// Carton file name for importing a carton. Exports carton's contents to a directory of the same name as the carton.
	#[arg(short, long, exclusive = true)]
	import: Option<String>,
}

fn main() {
	let args = Args::parse();

	if args.source.is_some() {
		let source = args.source.unwrap();
		let output = args.output.unwrap();

		let mut carton = Carton::default();
		carton.add_directory(&source);
		carton.to_file(&output);

		println!("Directory contents '{}' written to carton '{}'.", source, output);
	} else if args.import.is_some() {
		todo!();
	}
}
