use carton::{Carton, file_stream::FileReadStream};
use streams::ReadStream;

fn main() {
	let mut carton = Carton::default();
	carton.add_directory("scratch/resources");
	carton.to_file("scratch/resources.carton");

	let mut stream = FileReadStream::new("scratch/resources.carton");
	let new_carton = stream.decode::<Carton>();

	assert!(carton == new_carton);
}
