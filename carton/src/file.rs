use std::path::Path;
use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::metadata::FileMetadata;
use crate::translation_layer::FileDecoder;

/// Represents the compression algorithm used for a file.
#[derive(Debug, Eq, PartialEq)]
pub enum Compression {
	/// No compression. ID is encoded as 0.
	None,
	/// ZStd compression with specified level. ID is encoded as 1.
	ZStd(i8),
}

/// Compression is encoded as a 2 byte ID with a varying amount of bytes that describe the configuration settings of the
/// compression algorithm.
impl<T> Encode<u8, T> for Compression
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		match *self {
    	Compression::None => {
				stream.write_u16(0);
			}
    	Compression::ZStd(level) => {
				stream.write_u16(1);
				stream.write_u8(level as u8);
			}
		}
	}
}

/// Compression is encoded as a 2 byte ID with a varying amount of bytes that describe the configuration settings of the
/// compression algorithm.
impl<T> Decode<u8, T> for Compression
where
	T: ReadStream<u8> + U8ReadStream
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let (id, new_position) = stream.read_u16();
		match id {
			0 => (Compression::None, new_position),
			1 => {
				let (level, new_position) = stream.read_u8();
				(Compression::ZStd(level as i8), new_position)
			},
			_ => todo!("Compression algorithm decode not implemented for {}", id),
		}
	}
}

/// Represents a file in a carton.
#[derive(Debug, PartialEq)]
pub struct File {
	/// The compression format of the file.
	compression: Compression,
	/// The filename taken from the input file structure during encoding.
	file_name: String,
	/// The metadata for this file.
	metadata: Option<FileMetadata>,
	/// The size of the file.
	size: u64,
}

/// Represents a problem with reading a file.
#[derive(Debug)]
pub enum FileError {
	/// The specified path does not exist on disk.
	DoesntExist,
}

impl File {
	/// Create a file representation from a file name. Attempts to parse TOML metadata if it finds a `.toml` file that
	/// otherwise has the same name as the input file name.
	pub fn from_file(file_name: &str) -> Result<File, FileError> {
		if !Path::new(file_name).exists() {
			return Err(FileError::DoesntExist);
		}

		let metadata = if Path::new(&format!("{}.toml", file_name)).exists() {
			Some(
				FileMetadata::from_file(&format!("{}.toml", file_name)).unwrap()
			)
		} else {
			None
		};

		Ok(File {
			compression: Compression::None,
			file_name: String::from(file_name),
			metadata,
			size: std::fs::metadata(file_name).unwrap().len(),
		})
	}

	/// Create a file from the decode intermediate representation.
	pub(crate) fn from_intermediate(intermediate: FileDecoder, metadata: Option<toml::Value>) -> File {
		File {
			compression: intermediate.compression,
			file_name: intermediate.file_name.clone(),
			metadata: if let Some(value) = metadata {
				Some(FileMetadata::from_toml_value(&intermediate.file_name, value))
			} else {
				None
			},
			size: intermediate.size,
		}
	}

	/// Get the file's compression level.
	pub fn get_compression(&self) -> &Compression {
		&self.compression
	}

	/// Get the file's name.
	pub fn get_file_name(&self) -> &str {
		&self.file_name
	}

	/// Get the file's metadata.
	pub fn get_metadata(&self) -> &Option<FileMetadata> {
		&self.metadata
	}

	/// Get the file's size.
	pub fn get_size(&self) -> u64 {
		self.size
	}
}
