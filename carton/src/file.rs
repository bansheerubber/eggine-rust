use std::fs;
use std::io::{ Read, Write, };
use std::path::Path;
use streams::{ Decode, Encode, ReadStream, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringStream, U8WriteStream, };
use zstd::Encoder;

use crate::{ CartonError, Error, };
use crate::tables::StringTable;
use crate::metadata::{ FileMetadata, encode_metadata };

/// Represents the compression algorithm used for a file.
#[derive(Debug, Eq, PartialEq)]
pub enum Compression {
	/// No compression. ID is encoded as 0.
	None,
	/// ZStd compression with specified level and dictionary. ID is encoded as 1.
	ZStd(i8, Vec<u8>),
}

/// Compression is encoded as a 2 byte ID with a varying amount of bytes that describe the configuration settings of the
/// compression algorithm.
impl<T> Encode<u8, T, Error> for Compression
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		match self {
    	Compression::None => {
				stream.write_u16(0)?;
			}
    	Compression::ZStd(level, dictionary) => {
				stream.write_u16(1)?;
				stream.write_u8(*level as u8)?;
				stream.write_u16(dictionary.len() as u16)?;
				stream.write_vector(dictionary)?;
			}
		}

		Ok(())
	}
}

/// Compression is encoded as a 2 byte ID with a varying amount of bytes that describe the configuration settings of the
/// compression algorithm.
impl<T> Decode<u8, T, Error> for Compression
where
	T: ReadStream<u8, Error> + U8ReadStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (id, new_position) = stream.read_u16()?;
		match id {
			0 => Ok((Compression::None, new_position)),
			1 => {
				let (level, _) = stream.read_u8()?;

				let (dictionary_length, _) = stream.read_u16()?;
				let (dictionary, new_position) = stream.read_vector(dictionary_length as usize)?;

				Ok((Compression::ZStd(level as i8, dictionary), new_position))
			},
			_ => Err(Box::new(CartonError::InvalidCompression)),
		}
	}
}

/// First element is size in carton, second element is original file size.
type FileSize = (u64, u64);

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
	size: FileSize,
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

		let size = std::fs::metadata(file_name).unwrap().len();
		let compression = Compression::None;
		let size = if compression == Compression::None {
			(size, size)
		} else {
			(0, size)
		};

		Ok(File {
			compression,
			file_name: String::from(file_name),
			metadata,
			size,
		})
	}

	/// Create a file from the decode intermediate representation.
	pub(crate) fn from_intermediate(intermediate: IntermediateFile, metadata: Option<toml::Value>) -> File {
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

	/// Get the file's original size, before carton compression.
	pub fn get_size(&self) -> u64 {
		self.size.1
	}

	/// Gets the file's compressed size.
	pub fn get_compressed_size(&self) -> u64 {
		self.size.0
	}
}

pub(crate) fn encode_file<T>(stream: &mut T, file: &mut File, string_table: &mut StringTable)
	-> Result<(StreamPosition, StreamPosition), Error>
where
	T: WriteStream<u8, Error> + U8WriteStream<Error> + Seekable<Error> + Write
{
	let metadata_position = stream.get_position()? as u64;

	if let Some(metadata) = file.get_metadata() {
		encode_metadata(stream, metadata, string_table)?;
	}

	let mut raw_file = match fs::File::open(file.get_file_name()) {
    Ok(file) => file,
    Err(error) => return Err(Box::new(CartonError::FileError(error))),
	};

	let data = match file.get_compression() {
    Compression::None => {
			let mut vector = Vec::new();
			if let Err(error) = raw_file.read_to_end(&mut vector) {
				return Err(Box::new(CartonError::FileError(error)));
			}

			vector
		},
    Compression::ZStd(level, dictionary) => {
			let output = Vec::new();

			let encoder = if dictionary.len() > 0 {
				Encoder::new(output, *level as i32)
			} else {
				Encoder::with_dictionary(output, *level as i32, dictionary)
			};

			let mut encoder = match encoder {
				Ok(encoder) => encoder,
				Err(error) => return Err(Box::new(CartonError::FileError(error))),
			};

			let mut vector = Vec::new();
			if let Err(error) = raw_file.read_to_end(&mut vector) {
				return Err(Box::new(CartonError::FileError(error)));
			}

			match encoder.write_all(&vector) { // compress data
				Ok(()) => {},
				Err(error) => return Err(Box::new(CartonError::FileError(error))),
			}

			match encoder.finish() {
				Ok(output) => output,
				Err(error) => return Err(Box::new(CartonError::FileError(error))),
			}
		},
	};

	// update file's original size
	file.size = (data.len() as u64, file.size.1);

	file.get_compression().encode(stream)?; // can never be the value 7

	stream.write_u64(file.get_compressed_size())?;
	stream.write_u64(file.get_size())?;

	stream.write_string(file.get_file_name())?;

	let file_position = stream.get_position()?;

	stream.write_vector(&data)?;

	Ok((metadata_position, file_position))
}

/// Intermediate representation of a `File` object.
pub(crate) struct IntermediateFile {
	pub(crate) compression: Compression,
	pub(crate) file_name: String,
	pub(crate) file_offset: u64,
	pub(crate) size: FileSize,
}

pub(crate) fn decode_file<T>(stream: &mut T) -> Result<(IntermediateFile, StreamPosition), Error>
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringStream<Error> + Seekable<Error>
{
	let start = stream.get_position()?;
	let mut intermediate = IntermediateFile {
		compression: Compression::None,
		file_name: String::new(),
		file_offset: 0,
		size: (0, 0),
	};

	// read compression level
	let (compression, _) = stream.decode::<Compression>()?;
	intermediate.compression = compression;

	// read file size
	let (size, _) = stream.read_u64()?;
	let (original_size, _) = stream.read_u64()?;

	intermediate.size = (size, original_size);

	// read file name
	let (name, _) = stream.read_string()?;
	intermediate.file_name = name;

	// set the file offset, since our vector slice is now positioned at the beginning of the file
	intermediate.file_offset = (stream.get_position()? - start) as u64;

	Ok((intermediate, stream.get_position()?))
}
