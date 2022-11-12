use std::fs;
use std::io::Read;

use crate::StringTable;
use crate::file::File;
use crate::stream::{ Encode, EncodeMut, };
use crate::stream::writing::{ write_string, write_u64 };

use super::FileMetadataEncoder;

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileEncoder<'a> {
	pub(crate) file: &'a File,
	pub(crate) position: u64,
	pub(crate) string_table: &'a mut StringTable,
}

impl EncodeMut for FileEncoder<'_> {
	fn encode_mut(&mut self, vector: &mut Vec<u8>) {
		self.position = vector.len() as u64;

		if let Some(metadata) = self.file.get_metadata() {
			let mut encoder = FileMetadataEncoder {
				metadata,
				string_table: self.string_table,
			};

			encoder.encode_mut(vector);
		}

		self.file.get_compression().encode(vector);

		write_u64(self.file.get_size(), vector);
		println!("{}", self.file.get_size());
		write_string(self.file.get_file_name(), vector);

		let mut file = fs::File::open(self.file.get_file_name()).unwrap();
		file.read_to_end(vector).unwrap();
	}
}
