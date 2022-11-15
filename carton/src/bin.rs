use std::io::Read;

use carton::{Carton, file_stream::FileReadStream};
use streams::ReadStream;

fn main() {
	let mut carton = Carton::default();
	carton.add_directory("scratch/resources");
	carton.to_file("scratch/resources.carton");

	let mut file = std::fs::File::open("scratch/resources.carton").unwrap();
	let mut data = Vec::new();
	file.read_to_end(&mut data).unwrap();

	let mut stream = FileReadStream::new("scratch/resources.carton");
	let new_carton = stream.decode::<Carton>();

	assert!(carton == new_carton);
}
