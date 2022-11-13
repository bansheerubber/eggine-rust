use std::io::Read;

use carton::{ Carton, stream::Decode };

fn main() {
	let mut file = std::fs::File::open("scratch/test.carton").unwrap();
	let mut data = Vec::new();
	file.read_to_end(&mut data).unwrap();

	Carton::decode(&mut data);
}
