use std::collections::HashMap;
use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringStream, U8WriteStream, };

use crate::{ CartonError, Error };
use crate::file::File;

use super::TableID;

/// Maps files to their absolute positions within the carton.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct FileTable {
	/// Mapping of file names to file objects.
	files_by_name: HashMap<String, File>,
	/// Where the file metadata is located. File metadata is positioned before file contents. If we're building a carton
	/// from a directory, then the metadata mapping is only valid during file encoding and remains empty beforehand. If
	/// we're importing a carton from a `.carton` file, then the metadata mapping becomes valid immediately after file
	/// table decoding.
	metadata_positions: HashMap<String, u64>,
	/// Where the file data begins. Represents byte 0 of the file. If we're building a carton from a directory, then the
	/// absolute position mapping is only valid during file encoding and remains empty beforehand. If we're importing a
	/// carton from a `.carton` file, then the file position mapping becomes valid only after we decode the entire carton.
	file_positions: HashMap<String, u64>,
}
impl FileTable {
	/// Adds a standalone file from disk.
	pub fn add_from_disk(&mut self, file: File) {
		self.files_by_name.insert(file.get_file_name().to_string(), file);
	}

	/// Adds a standalone file from the intermediate decode.
	pub(crate) fn add_from_intermediate(&mut self, file: File) {
		self.files_by_name.insert(file.get_file_name().to_string(), file);
	}

	/// Add a file's metadata position into the table during the decode process.
	pub(crate) fn add_position_from_decode(&mut self, file_name: String, metadata_position: u64) {
		self.metadata_positions.insert(file_name.clone(), metadata_position);
	}

	/// Update the file's metadata position and absolute file position.
	pub fn update_position(&mut self, file_name: &str, metadata_position: u64, file_position: u64) {
		self.metadata_positions.insert(String::from(file_name), metadata_position);
		self.file_positions.insert(String::from(file_name), file_position);
	}

	/// Get a reference for file metadata positions.
	pub fn get_metadata_positions(&self) -> &HashMap<String, u64> {
		&self.metadata_positions
	}

	/// Get a reference for file positions.
	pub fn get_file_positions(&self) -> &HashMap<String, u64> {
		&self.file_positions
	}

	/// Get a reference for files by file name
	pub fn get_files_by_name(&self) -> &HashMap<String, File> {
		&self.files_by_name
	}
}

/// The file table stores a key/value pair where the keys are file names and the values are file metadata positions. If
/// we follow the position pointer, we will read either the encoded TOML metadata for the file, or if lacking TOML
/// metadata, the compression level, file size, and file name.
impl<T> Encode<u8, T, Error> for FileTable
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>,
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		stream.write_u8(TableID::FileTable as u8)?;
		stream.write_u64(self.metadata_positions.len() as u64)?;

		for (file_name, position) in self.metadata_positions.iter() {
			stream.write_string(file_name)?;
			stream.write_u64(*position)?;
		}

		Ok(())
	}
}

/// We can only resolve metadata positions at this point. We have to do a separate pass for file starting positions
/// after the initial decode. File starting positions are inferred during metadata decoding.
impl<T> Decode<u8, T, Error> for FileTable
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringStream<Error>,
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (table_id, _) = stream.read_u8()?;
		if table_id != TableID::FileTable as u8 {
			return Err(Box::new(CartonError::UnexpectedTable));
		}

		let mut table = FileTable::default();
		let (row_count, mut position) = stream.read_u64()?;

		for _ in 0..row_count {
			let (file_name, _) = stream.read_string()?;

			let (file_position, new_position) = stream.read_u64()?;
			position = new_position;

			table.add_position_from_decode(file_name, file_position);
		}

		Ok((table, position))
	}
}
