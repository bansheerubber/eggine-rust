use crate::file::File;
use crate::file_table::FileTable;
use crate::stream::{ Decode, Encode, EncodeMut, Stream, };
use crate::stream::reading::{ read_char, read_u8, read_u64, };
use crate::stream::writing::{ write_char, write_u8, write_u64, };
use crate::StringTable;
use crate::translation_layer::FileEncoder;

const CARTON_VERSION: u8 = 2;

/// Representation of a carton file.
#[derive(Debug)]
pub struct Carton {
	pub(crate) file_table: FileTable,
	pub string_table: StringTable,
	pub version: u8,
}

impl Default for Carton {
	fn default() -> Self {
		Carton {
			file_table: FileTable::default(),
			string_table: StringTable::default(),
			version: CARTON_VERSION,
		}
	}
}

impl Carton {
	/// Write the carton to a file.
	pub fn to_file(&mut self, file_name: &str) {
		let mut stream = Stream::default();
		stream.encode_mut(self);
		stream.to_file(file_name);
	}

	/// Add a file to the carton. The file will be written into the carton archive format when it is exported.
	pub fn add_file(&mut self, file_name: &str) {
		self.file_table.add_from_disk(File::from_file(file_name).unwrap());
	}
}

impl EncodeMut for Carton {
	fn encode_mut(&mut self, vector: &mut Vec<u8>) {
		// write magic number and the version
		write_char('C', vector);
		write_char('A', vector);
		write_char('R', vector);
		write_char('T', vector);
		write_char('O', vector);
		write_char('N', vector);
		write_u8(self.version, vector);

		// reserve spot for file table position
		let file_table_pointer = vector.len();
		write_u64(0, vector);

		let mut positions = Vec::new();
		for file in self.file_table.get_files() {
			let mut encoder = FileEncoder {
				file,
				position: 0,
				string_table: &mut self.string_table,
			};

			encoder.encode_mut(vector);
			positions.push((String::from(file.get_file_name()), encoder.position));
		}

		for (file_name, position) in positions {
			self.file_table.update_position(&file_name, position);
		}

		let file_table_position = vector.len() as u64;
		self.file_table.encode(vector);
		self.string_table.encode(vector);

		// write file table position at the top of the file
		let mut position_vector = Vec::new();
		write_u64(file_table_position, &mut position_vector);
		for i in 0..4 {
			vector[i + file_table_pointer] = position_vector[i];
		}
	}
}

impl Decode for Carton {
	fn decode(vector: &[u8]) -> (Self, &[u8]) {
		let mut magic_number_pointer = vector;
		let carton = Carton::default();

		let mut magic = String::new();
		for _ in 0..6 {
			let (character, new_position) = read_char(magic_number_pointer);
			magic_number_pointer = new_position;
			magic.push(character);
		}

		if magic != "CARTON" {
			panic!("Invalid magic number");
		}

		let (version, new_position) = read_u8(magic_number_pointer);
		magic_number_pointer = new_position;
		if version != CARTON_VERSION {
			panic!("Invalid version");
		}

		let (file_table_pointer, _) = read_u64(magic_number_pointer);

		return (carton, vector);
	}
}
