use carton::Carton;

fn main() {
	let mut carton = Carton::default();
	carton.string_table.insert("test1");
	carton.string_table.insert("test2");
	carton.string_table.insert("test3");
	carton.string_table.insert("test4");
	carton.string_table.insert("test5");

	carton.add_file("scratch/test/file.txt");

	carton.to_file("scratch/test.carton");
}
