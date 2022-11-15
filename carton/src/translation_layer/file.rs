use std::fs;
use std::io::Read;
use streams::{ Decode, Encode, ReadStream, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::StringTable;
use crate::file::{ Compression, File, };

use crate::metadata::encode_metadata;

pub(crate) fn encode_file<T>(stream: &mut T, file: &File, string_table: &mut StringTable)
	-> (StreamPosition, StreamPosition)
where
	T: WriteStream<u8> + U8WriteStream + Seekable
{
	let metadata_position = stream.get_position() as u64;

	if let Some(metadata) = file.get_metadata() {
		encode_metadata(stream, metadata, string_table);
	}

	file.get_compression().encode(stream); // can never be the value 7

	stream.write_u64(file.get_size());
	stream.write_string(file.get_file_name());

	let file_position = stream.get_position();

	let mut vector = Vec::new();
	fs::File::open(file.get_file_name()).unwrap().read_to_end(&mut vector).unwrap();
	stream.write_vector(&vector);

	(metadata_position, file_position)
}

/// Intermediate representation of a `File` object.
#[derive(Debug)]
pub(crate) struct FileDecoder {
	pub(crate) compression: Compression,
	pub(crate) file_name: String,
	pub(crate) file_offset: u64,
	pub(crate) size: u64,
}

/// Decode a file into an intermediate representation, because we have some things that we need to do after decoding
/// that requires additional context, like setting file absolute position
impl<T> Decode<u8, T> for FileDecoder
where
	T: ReadStream<u8> + U8ReadStream + Seekable
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let start = stream.get_position();
		let mut decoder = FileDecoder {
			compression: Compression::None,
			file_name: String::new(),
			file_offset: 0,
			size: 0,
		};

		// read compression level
		let (compression, _) = Compression::decode(stream);
		decoder.compression = compression;

		// read file size
		let (size, _) = stream.read_u64();
		decoder.size = size;

		// read file name
		let (name, _) = stream.read_string();
		decoder.file_name = name;

		// set the file offset, since our vector slice is now positioned at the beginning of the file
		decoder.file_offset = (stream.get_position() - start) as u64;

		return (decoder, stream.get_position());
	}
}
