use crate::stream::Encode;
use crate::stream::Stream;
use crate::StringTable;
use crate::stream::writing::write_byte;
use crate::stream::writing::write_char;

/// Representation of a carton file.
#[derive(Debug)]
pub struct Carton {
	pub string_table: StringTable,
	pub version: u8,
}

impl Default for Carton {
	fn default() -> Self {
		Carton {
			string_table: Default::default(),
			version: 2,
		}
	}
}

impl Carton {
	pub fn to_file(&self, file_name: &str) {
		let mut stream = Stream::default();
		stream.encode(self);

		stream.to_file(file_name)
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
		write_byte(self.version, vector);

		self.string_table.encode(vector);
	}
}