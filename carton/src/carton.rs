use crate::file::File;
use crate::file_table::FileTable;
use crate::stream::Encode;
use crate::stream::Stream;
use crate::StringTable;
use crate::stream::writing::write_u8;
use crate::stream::writing::write_char;

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
			version: 2,
		}
	}
}

impl Carton {
	/// Write the carton to a file.
	pub fn to_file(&self, file_name: &str) {
		let mut stream = Stream::default();
		stream.encode(self);

		stream.to_file(file_name);
	}

	/// Add a file to the carton. The file will be written into the carton archive format when it is exported.
	pub fn add_file(&mut self, file_name: &str) {
		self.file_table.add_from_disk(File::from_file(file_name).unwrap());
	}
}

impl Encode for Carton {
	fn encode(&self, vector: &mut Vec<u8>) {
		// write magic number and the version
		write_char('C', vector);
		write_char('A', vector);
		write_char('R', vector);
		write_char('T', vector);
		write_char('O', vector);
		write_char('N', vector);
		write_u8(self.version, vector);

		self.string_table.encode(vector);
	}
}