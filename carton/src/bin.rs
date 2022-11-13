use std::io::Read;

use carton::Carton;
use carton::stream::Decode;

fn main() {
	// let mut carton = Carton::default();
	// carton.string_table.insert("test1");
	// carton.string_table.insert("test2");
	// carton.string_table.insert("test3");
	// carton.string_table.insert("test4");
	// carton.string_table.insert("test5");

	// carton.add_file("scratch/test/file.txt");

	// carton.to_file("scratch/test.carton");

	let mut file = std::fs::File::open("scratch/test.carton").unwrap();
	let mut data = Vec::new();
	file.read_to_end(&mut data).unwrap();

	Carton::decode(&mut data);
}
