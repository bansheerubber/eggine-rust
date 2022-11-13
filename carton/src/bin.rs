use std::io::Read;

use carton::Carton;
use carton::stream::Decode;

fn main() {
	// let mut carton = Carton::default();
	// carton.add_directory("scratch/resources");
	// carton.to_file("scratch/resources.carton");

	let mut file = std::fs::File::open("scratch/resources.carton").unwrap();
	let mut data = Vec::new();
	file.read_to_end(&mut data).unwrap();

	Carton::decode(&mut data);
}
