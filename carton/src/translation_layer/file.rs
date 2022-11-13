use std::fs;
use std::io::Read;

use crate::StringTable;
use crate::file::{File, Compression};
use crate::stream::{ Decode, Encode, EncodeMut, };
use crate::stream::reading::{ read_string, read_u64, };
use crate::stream::writing::{ write_string, write_u64 };

use super::{ FileMetadataDecoder, FileMetadataEncoder, };

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileEncoder<'a> {
	pub(crate) file: &'a File,
	pub(crate) file_position: u64,
	pub(crate) metadata_position: u64,
	pub(crate) string_table: &'a mut StringTable,
}

/// Encode the file. We don't implement the `EncodeMut` trait for the file directory since additional context is needed
/// to encode the file.
///
/// File encoding w/ metadata:
/// | metadata (? bytes) | compression (? bytes) | file size (8 bytes) | file name (? bytes) | file contents (`file size` bytes) |
///
/// File encoding w/o metadata:
/// | compression (? bytes) | file size (8 bytes) | file name (? bytes) | file contents (`file size` bytes) |
impl EncodeMut for FileEncoder<'_> {
	fn encode_mut(&mut self, vector: &mut Vec<u8>) {
		self.metadata_position = vector.len() as u64;

		if let Some(metadata) = self.file.get_metadata() {
			let mut encoder = FileMetadataEncoder {
				metadata,
				string_table: self.string_table,
			};

			encoder.encode_mut(vector);
		}

		self.file.get_compression().encode(vector);

		write_u64(self.file.get_size(), vector);
		write_string(self.file.get_file_name(), vector);

		self.file_position = vector.len() as u64;

		fs::File::open(self.file.get_file_name()).unwrap().read_to_end(vector).unwrap();
	}
}

/// Intermediate representation of a `File` object.
#[derive(Debug)]
pub(crate) struct FileDecoder {
	compression: Compression,
	file_name: String,
	file_offset: u64,
	metadata: Option<FileMetadataDecoder>,
	size: u64,
}

/// Translate the intermediate representation into a fully-fledged file object populated with metadata.
impl FileDecoder {
	pub(crate) fn translate(&self, string_table: &mut StringTable) {

	}
}

/// Decode a file into an intermediate representation, because we have some things that we need to do after decoding
/// that requires additional context, like `FileMetadata` string table lookups.
impl Decode for FileDecoder {
	fn decode(vector: &[u8]) -> (Self, &[u8]) {
		let length = vector.len();
		let mut decoder = FileDecoder {
			compression: Compression::None,
			file_name: String::new(),
			file_offset: 0,
			metadata: None,
			size: 0,
		};

		// read compression level
		let (compression, new_position) = Compression::decode(vector);
		let mut vector = new_position;
		decoder.compression = compression;

		// read file size
		let (size, new_position) = read_u64(vector);
		vector = new_position;
		decoder.size = size;

		// read file name
		let (name, new_position) = read_string(vector);
		vector = new_position;
		decoder.file_name = name;

		// set the file offset, since our vector slice is now positioned at the beginning of the file
		decoder.file_offset = (length - vector.len()) as u64;

		return (decoder, vector);
	}
}
