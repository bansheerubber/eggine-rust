use walkdir::WalkDir;

use crate::file::File;
use crate::file_table::FileTable;
use crate::stream::{ Decode, Encode, EncodeMut, Stream, };
use crate::stream::reading::{ read_char, read_u8, read_u64, };
use crate::stream::writing::{ write_char, write_u8, write_u64, };
use crate::StringTable;
use crate::translation_layer::{ FileDecoder, FileEncoder, };

const CARTON_VERSION: u8 = 2;

/// Representation of a carton. Cartons are an archive file format designed for efficient storage of video game data.
/// Features include compression of files, a metadata database for looking up files during runtime, and streaming files
/// in discrete chunks for audio streams. The carton file format is built upon a stream encoding/decoding API that is
/// designed to support everything from storing data in files, to sending data over the network. Cartons are designed to
/// be constructed from a directory which includes data to be included in a video game. The carton preserves the file
/// structure and automatically assigns imported files with metadata read from TOML files.
#[derive(Debug)]
pub struct Carton {
	/// Keeps track of files in the carton.
	pub(crate) file_table: FileTable,
	/// Stores strings used throughout the carton.
	pub string_table: StringTable,
	/// Version of the carton encoding.
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

	/// Add a directory to the carton. All files in the directory will be added into the carton.
	pub fn add_directory(&mut self, directory_name: &str) {
		for entry in WalkDir::new(directory_name) {
			let entry = entry.unwrap();
			if entry.metadata().unwrap().is_file() {
				let file_name = entry.path().to_str().unwrap();
				if file_name.contains("metadata") {
					continue;
				}

				self.add_file(file_name);
			}
		}
	}
}

/// Encode the carton into a `.carton` file. Carton files start with the `CARTON` magic number and the carton encoding
/// version. The encoding reserves a 8 byte number after the version that will point to the file table. Files and their
/// metadata are written first. The file and metadata encoding process updates the internal state of the carton,
/// necessary for completing the file and string tables. Once all files are written, the file table is written with the
/// string table following afterwards. Once the file table and string tables are written, the file table pointer at the
/// start of the file is updated to point to the absolute location of the file table.
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
				file_position: 0,
				metadata_position: 0,
				string_table: &mut self.string_table,
			};

			encoder.encode_mut(vector);
			positions.push((String::from(file.get_file_name()), encoder.metadata_position, encoder.file_position));
		}

		for (file_name, metadata_position, file_position) in positions {
			self.file_table.update_position(&file_name, metadata_position, file_position);
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

/// Decode a `.carton` file. Carton files first check the file's magic number and carton encoding version, and then
/// move onto decoding the file table and string table. Afterwards, all files and their metadata are decoded. The only
/// data loaded into memory are the representations of the files, instead of their entire contents. The API gives the
/// user the option to load the file into memory after decoding. Metadata is always memory resident, since the API gives
/// a way to query files in the carton by searching metadata values.
impl Decode for Carton {
	fn decode(vector: &[u8]) -> (Self, &[u8]) {
		let mut magic_number_pointer = vector;
		let mut carton = Carton::default();

		// check the carton magic number
		let mut magic = String::new();
		for _ in 0..6 {
			let (character, new_position) = read_char(magic_number_pointer);
			magic_number_pointer = new_position;
			magic.push(character);
		}

		if magic != "CARTON" {
			panic!("Invalid magic number");
		}

		// check the carton version
		let (version, new_position) = read_u8(magic_number_pointer);
		magic_number_pointer = new_position;
		if version != CARTON_VERSION {
			panic!("Invalid version");
		}

		let (file_table_pointer, _) = read_u64(magic_number_pointer);

		// partially load the file table, only file name -> metadata position map is valid
		let (file_table, new_position) = FileTable::decode(&vector[(file_table_pointer as usize)..]);
		carton.file_table = file_table;

		// fully load the string table
		let (string_table, _) = StringTable::decode(new_position);
		carton.string_table = string_table;

		// load metadata and files from carton
		let mut files = Vec::new();
		for (name,  position) in carton.file_table.get_metadata_positions() {
			let (file, _) = FileDecoder::decode(&vector[(*position as usize)..]);
			files.push(file);
		}

		// finish off loading the file table
		println!("{:?}", files);

		return (carton, vector);
	}
}
