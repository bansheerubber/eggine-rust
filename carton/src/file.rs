use std::path::Path;

use crate::metadata::FileMetadata;
use crate::stream::Encode;
use crate::stream::writing::{ write_u8, write_u16, };

/// Represents the compression algorithm used for a file.
#[derive(Debug)]
pub enum Compression {
	/// No compression. ID is encoded as 0.
	None,
	/// ZStd compression with specified level. ID is encoded as 1.
	ZStd(i8),
}

/// Compression is encoded as a 2 byte ID with a varying amount of bytes that describe the configuration settings of the
/// compression algorithm.
impl Encode for Compression {
	fn encode(&self, vector: &mut Vec<u8>) {
		match *self {
    	Compression::None => {
				write_u16(0, vector);
			}
    	Compression::ZStd(level) => {
				write_u16(1, vector);
				write_u8(level as u8, vector);
			}
		}
	}
}

/// Represents a file in a carton.
#[derive(Debug)]
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

	pub fn get_compression(&self) -> &Compression {
		&self.compression
	}

	pub fn get_file_name(&self) -> &str {
		&self.file_name
	}

	pub fn get_metadata(&self) -> &Option<FileMetadata> {
		&self.metadata
	}

	pub fn get_size(&self) -> u64 {
		self.size
	}
}
