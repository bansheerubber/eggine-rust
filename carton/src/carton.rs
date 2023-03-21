use std::fs::OpenOptions;
use std::io::Write;
use streams::{ Decode, EncodeMut, Endable, ReadStream, Peekable, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringStream, U8WriteStream, };
use walkdir::WalkDir;

use crate::carton_file_stream::CartonFileReadStream;
use crate::{ CartonError, Error, };
use crate::tables::{ FileTable, TableID, };
use crate::tables::StringTable;
use crate::file::{ File, decode_file, encode_file };
use crate::file_stream::{ FileReadStream, FileWriteStream, };
use crate::metadata::decode_value;

const CARTON_VERSION: u8 = 2;

/// Representation of a carton. Cartons are an archive file format designed for efficient storage of video game data.
/// Features include compression of files, a metadata database for looking up files during runtime, and streaming files
/// in discrete chunks for audio streams. The carton file format is built upon a stream encoding/decoding API that is
/// designed to support everything from storing data in files, to sending data over the network. Cartons are designed to
/// be constructed from a directory which includes data to be included in a video game. The carton preserves the file
/// structure and automatically assigns imported files with metadata read from TOML files.
#[derive(Debug)]
pub struct Carton {
	pub(crate) file: Option<std::fs::File>,
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
			file: None,
			file_table: FileTable::default(),
			string_table: StringTable::default(),
			version: CARTON_VERSION,
		}
	}
}

impl Carton {
	/// Write the carton to a file.
	pub fn to_file(&mut self, file_name: &str) {
		let mut stream = FileWriteStream::new(file_name).unwrap();
		stream.encode_mut(self).unwrap();
		stream.export().unwrap();
	}

	/// Decodes the carton from file and sets up file reading.
	pub fn read(file_name: &str) -> Result<Carton, Error> {
		let mut stream = FileReadStream::new(file_name)?;
		let mut new_carton = stream.decode::<Carton>()?.0;

		let file = match OpenOptions::new()
			.read(true)
			.open(file_name)
		{
			Ok(file) => file,
			Err(error) => return Err(Box::new(CartonError::FileError(error))),
		};

		new_carton.file = Some(file);

		Ok(new_carton)
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
				if file_name.contains("metadata") || file_name.contains("toml") {
					continue;
				}

				self.add_file(file_name);
			}
		}
	}

	pub fn get_file_data(&mut self, file_name: &str) -> Result<CartonFileReadStream, Error> {
		if self.file.is_none() {
			return Err(Box::new(CartonError::FileNotOpen));
		}

		if !self.file_table.get_files_by_name().contains_key(file_name) {
			return Err(Box::new(CartonError::DecodedFileNotFound));
		}

		CartonFileReadStream::new(self, &self.file_table.get_files_by_name()[file_name])
	}
}

/// Encode the carton into a `.carton` file. Carton files start with the `CARTON` magic number and the carton encoding
/// version. The encoding reserves a 8 byte number after the version that will point to the file table. Files and their
/// metadata are written first. The file and metadata encoding process updates the internal state of the carton,
/// necessary for completing the file and string tables. Once all files are written, the file table is written with the
/// string table following afterwards. Once the file table and string tables are written, the file table pointer at the
/// start of the file is updated to point to the absolute location of the file table.
impl<T> EncodeMut<u8, T, Error> for Carton
where
	T: WriteStream<u8, Error> + U8WriteStream<Error> + Seekable<Error> + Write
{
	fn encode_mut(&mut self, stream: &mut T) -> Result<(), Error> {
		// write magic number and the version
		stream.write_char('C')?;
		stream.write_char('A')?;
		stream.write_char('R')?;
		stream.write_char('T')?;
		stream.write_char('O')?;
		stream.write_char('N')?;
		stream.write_u8(self.version)?;

		// reserve spot for tables position
		let table_pointer = stream.get_position()?;
		stream.write_u64(0)?;

		// encode files
		let mut positions = Vec::new();
		for file in self.file_table.get_files_by_name().values() {
			let (metadata_position, file_position) = encode_file(stream, file, &mut self.string_table)?;
			positions.push((String::from(file.get_file_name()), metadata_position, file_position));
		}

		// update file positions in file table
		for (file_name, metadata_position, file_position) in positions {
			self.file_table.update_position(&file_name, metadata_position, file_position);
		}

		// write file & string streams to file
		let first_table_position = stream.get_position()?;
		stream.encode(&self.file_table)?;
		stream.encode(&self.string_table)?;

		// write file table position at the top of the file
		stream.seek(table_pointer)?;
		stream.write_u64(first_table_position)?;

		Ok(())
	}
}

/// Decode a `.carton` file. Carton files first check the file's magic number and carton encoding version, and then
/// move onto decoding the file table and string table. Afterwards, all files and their metadata are decoded. The only
/// data loaded into memory are the representations of the files, instead of their entire contents. The API gives the
/// user the option to load the file into memory after decoding. Metadata is always memory resident, since the API gives
/// a way to query files in the carton by searching metadata values.
impl<T> Decode<u8, T, Error> for Carton
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringStream<Error> + Seekable<Error>
		+ Peekable<u8, Error> + Endable<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let mut carton = Carton::default();

		// check the carton magic number
		let mut magic = String::new();
		for _ in 0..6 {
			let (character, _) = stream.read_char()?;
			magic.push(character);
		}

		if magic != "CARTON" {
			return Err(Box::new(CartonError::InvalidMagicNumber));
		}

		// check the carton version
		let (version, _) = stream.read_u8()?;
		if version != CARTON_VERSION {
			return Err(Box::new(CartonError::InvalidVersion));
		}

		// load tables
		let (table_pointer, _) = stream.read_u64()?;
		stream.seek(table_pointer)?;

		while !stream.is_at_end()? {
			let table_id = TableID::from(stream.peek()?);
			match table_id {
    		TableID::Invalid => return Err(Box::new(CartonError::UnexpectedTable)),
    		TableID::FileTable => {
					// partially load the file table, only file name -> metadata position map is valid
					let (file_table, _) = stream.decode::<FileTable>()?;
					carton.file_table = file_table;
				},
    		TableID::StringTable => {
					// fully load the string table
					let (string_table, _) = stream.decode::<StringTable>()?;
					carton.string_table = string_table;
				},
			}
		}

		// load metadata and files from carton in the order that they were written to the `.carton`
		let mut files = Vec::new();
		let mut sorted_metadata_positions = carton.file_table.get_metadata_positions().iter()
			.collect::<Vec<(&String, &u64)>>();
		sorted_metadata_positions.sort_by(|(_, a), (_, b)| a.cmp(b));

		for (file_name, position) in sorted_metadata_positions {
			stream.seek(*position)?;

			let check_value = stream.peek()?;
			let metadata = if check_value == 7 {
				let (metadata, _) = decode_value(stream, &mut carton.string_table)?;
				Some(metadata)
			} else {
				None
			};

			let metadata_length = stream.get_position()? - *position;
			let (file, _) = decode_file(stream)?;
			if &file.file_name != file_name {
				return Err(Box::new(CartonError::UnexpectedFileName));
			}

			files.push((file, *position, metadata_length as u64, metadata));
		}

		// finish off loading the file table
		for (file, position, metadata_length, metadata) in files {
			carton.file_table.update_position(&file.file_name, position, position + metadata_length + file.file_offset);
			carton.file_table.add_from_intermediate(File::from_intermediate(file, metadata));
		}

		Ok((carton, stream.get_position()?))
	}
}
