use streams::{ ReadStream, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::{ CartonError, Error, };
use crate::tables::StringTable;

/// Emitted when a `.toml` metadata file cannot be parsed.
#[derive(Debug)]
pub enum FileMetadataError {
	/// Could not load the metadata file
	FileError,
	/// Metadata file did not have the .toml extension
	IncorrectExtension,
	/// Metadata file TOML contents could not be deserialized
	ParseError(toml::de::Error),
}

/// Represents metadata for a file stored in a carton. All metadata has a corresponding file it describes. Metadata is
/// interpreted from a TOML file that has the same name as the file it describes with the `.toml` extension appended.
#[derive(Debug, PartialEq)]
pub struct FileMetadata {
	/// File that the `FileMetadata`'s metadata describes.
	file_name: String,
	/// Indexable metadata contents
	value: toml::Value,
}

impl FileMetadata {
	/// Parse a TOML metadata file into a `FileMetadata` struct. A subset of TOML is supported, specifically string
	/// key/value pairs inside of the `values` table.
	///
	/// If parsing fails, the `FileMetadataError` structure can be interrogated to determine the `toml::de::Error` that
	/// was returned.
	pub fn from_file(file_name: &str) -> Result<FileMetadata, FileMetadataError> {
		let length = file_name.len();
		if &file_name[length - 5..] == ".toml" {
			let Ok(contents) = std::fs::read_to_string(file_name) else {
				return Err(FileMetadataError::FileError);
			};

			let value = match toml::from_str::<toml::Value>(&contents) {
				Ok(value) => value,
				Err(error) => return Err(FileMetadataError::ParseError(error)),
			};

			Ok(FileMetadata {
				file_name: file_name[0..length - 5].to_string(),
				value,
			})
		} else {
			Err(FileMetadataError::IncorrectExtension)
		}
	}

	/// Create a metadata object from a pre-existing `toml::Value`
	pub fn from_toml_value(file_name: &str, value: toml::Value) -> FileMetadata {
		FileMetadata {
			file_name: String::from(file_name),
			value,
		}
	}

	pub(crate) fn get_file_metadata_toml(&self) -> &toml::Value {
		&self.value
	}
}

enum TOMLValueType {
	INVALID 	= 0,
	STRING 		= 1,
	INTEGER 	= 2,
	FLOAT 		= 3,
	BOOLEAN 	= 4,
	DATETIME 	= 5,
	ARRAY 		= 6,
	TABLE			= 7,
}

impl From<u8> for TOMLValueType {
	fn from(number: u8) -> Self {
		match number {
			1 => TOMLValueType::STRING,
			2 => TOMLValueType::INTEGER,
			3 => TOMLValueType::FLOAT,
			4 => TOMLValueType::BOOLEAN,
			5 => TOMLValueType::DATETIME,
			6 => TOMLValueType::ARRAY,
			7 => TOMLValueType::TABLE,
			_ => TOMLValueType::INVALID,
		}
	}
}

fn encode_value<T, Error>(value: &toml::Value, stream: &mut T, string_table: &mut StringTable) -> Result<(), Error>
where
	T: WriteStream<u8, Error> + U8WriteStream<Error> + Seekable<Error>
{
	match value {
    toml::Value::String(value) => {
			let id = if let Some(id) = string_table.get(&value) {
				id
			} else {
				string_table.insert(&value)
			};

			stream.write_u8(TOMLValueType::STRING as u8)?;
			stream.write_vlq(id)?;
		},
    toml::Value::Integer(number) => {
			stream.write_u8(TOMLValueType::INTEGER as u8)?;
			stream.write_vlq(*number as u64)?;
		},
    toml::Value::Float(_) => todo!(),
    toml::Value::Boolean(number) => {
			stream.write_u8(TOMLValueType::BOOLEAN as u8)?;
			stream.write_u8(*number as u8)?;
		},
    toml::Value::Datetime(_) => todo!(),
    toml::Value::Array(array) => {
			stream.write_u8(TOMLValueType::ARRAY as u8)?;
			stream.write_vlq(array.len() as u64)?;
			for value in array {
				encode_value(&value, stream, string_table)?;
			}
		},
    toml::Value::Table(map) => {
			stream.write_u8(TOMLValueType::TABLE as u8)?;
			stream.write_vlq(map.len() as u64)?;
			for (key, value) in map {
				let id = if let Some(id) = string_table.get(&key) {
					id
				} else {
					string_table.insert(&key)
				};

				stream.write_vlq(id)?;
				encode_value(&value, stream, string_table)?;
			}
		},
	}

	Ok(())
}

// Encode the `toml::Value` in the metadata.
pub fn encode_metadata<T>(stream: &mut T, metadata: &FileMetadata, string_table: &mut StringTable)
	-> Result<(), Error>
where
	T: WriteStream<u8, Error> + U8WriteStream<Error> + Seekable<Error>
{
	encode_value(metadata.get_file_metadata_toml(), stream, string_table)
}

pub fn decode_value<T>(stream: &mut T, string_table: &mut StringTable)
	-> Result<(toml::Value, StreamPosition), Error>
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + Seekable<Error>
{
	let (value_type, _) = stream.read_u8()?;

	match TOMLValueType::from(value_type) {
    TOMLValueType::INVALID => Err(Box::new(CartonError::InvalidTOMLType(value_type))),
    TOMLValueType::STRING => {
			let (id, next_position) = stream.read_vlq()?;
			if let Some(string) = string_table.get_from_index(id) {
				Ok((toml::Value::String(string.clone()), next_position))
			} else {
				Err(Box::new(CartonError::NotInStringTable(id)))
			}
		},
    TOMLValueType::INTEGER => {
			let (number, next_position) = stream.read_vlq()?;
			Ok((toml::Value::Integer(number as i64), next_position))
		},
    TOMLValueType::FLOAT => todo!(),
    TOMLValueType::BOOLEAN => {
			let (number, next_position) = stream.read_u8()?;
			Ok((toml::Value::Boolean(number != 0), next_position))
		},
    TOMLValueType::DATETIME => todo!(),
    TOMLValueType::ARRAY => {
			let (length, mut position) = stream.read_vlq()?;

			let mut array = Vec::new();
			for _ in 0..length {
				let (value, next_position) = decode_value(stream, string_table)?;
				position = next_position;

				array.push(value);
			}

			Ok((toml::Value::Array(array), position))
		},
    TOMLValueType::TABLE => {
			let (length, mut position) = stream.read_vlq()?;

			let mut map = toml::map::Map::new();
			for _ in 0..length {
				let (id, _) = stream.read_vlq()?;

				let Some(key) = string_table.get_from_index(id) else {
					return Err(Box::new(CartonError::NotInStringTable(id)));
				};
				let key = key.clone();

				let (value, next_position) = decode_value(stream, string_table)?;
				position = next_position;

				map.insert(key.clone(), value);
			}

			Ok((toml::Value::Table(map), position))
		},
	}
}

