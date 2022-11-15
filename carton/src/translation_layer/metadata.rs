use streams::{ EncodeMut, ReadStream, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::StringTable;
use crate::metadata::FileMetadata;

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileMetadataEncoder<'a> {
	pub(crate) metadata: &'a FileMetadata,
	pub(crate) string_table: &'a mut StringTable,
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

fn encode_value<T>(value: &toml::Value, stream: &mut T, string_table: &mut StringTable)
where
T: WriteStream<u8> + U8WriteStream + Seekable
{
	match value {
    toml::Value::String(value) => {
			let id = if let Some(id) = string_table.get(&value) {
				id
			} else {
				string_table.insert(&value)
			};

			stream.write_u8(TOMLValueType::STRING as u8);
			stream.write_vlq(id);
		},
    toml::Value::Integer(number) => {
			stream.write_u8(TOMLValueType::INTEGER as u8);
			stream.write_vlq(*number as u64);
		},
    toml::Value::Float(_) => todo!(),
    toml::Value::Boolean(number) => {
			stream.write_u8(TOMLValueType::BOOLEAN as u8);
			stream.write_u8(*number as u8);
		},
    toml::Value::Datetime(_) => todo!(),
    toml::Value::Array(array) => {
			stream.write_u8(TOMLValueType::ARRAY as u8);
			stream.write_vlq(array.len() as u64);
			for value in array {
				encode_value(&value, stream, string_table);
			}
		},
    toml::Value::Table(map) => {
			stream.write_u8(TOMLValueType::TABLE as u8);
			stream.write_vlq(map.len() as u64);
			for (key, value) in map {
				let id = if let Some(id) = string_table.get(&key) {
					id
				} else {
					string_table.insert(&key)
				};

				stream.write_vlq(id);
				encode_value(&value, stream, string_table);
			}
		},
	}
}

pub fn decode_value<T>(stream: &mut T, string_table: &mut StringTable) -> (toml::Value, StreamPosition)
where
	T: ReadStream<u8> + U8ReadStream + Seekable
{
	let (value_type, _) = stream.read_u8();

	match TOMLValueType::from(value_type) {
    TOMLValueType::INVALID => unreachable!(),
    TOMLValueType::STRING => {
			let (id, next_position) = stream.read_vlq();
			if let Some(string) = string_table.get_from_index(id) {
				return (toml::Value::String(string.clone()), next_position);
			} else {
				panic!(); // TODO better error handling
			}
		},
    TOMLValueType::INTEGER => {
			let (number, next_position) = stream.read_vlq();
			return (toml::Value::Integer(number as i64), next_position);
		},
    TOMLValueType::FLOAT => todo!(),
    TOMLValueType::BOOLEAN => {
			let (number, next_position) = stream.read_u8();
			return (toml::Value::Boolean(number != 0), next_position);
		},
    TOMLValueType::DATETIME => todo!(),
    TOMLValueType::ARRAY => {
			let (length, mut position) = stream.read_vlq();

			let mut array = Vec::new();
			for _ in 0..length {
				let (value, next_position) = decode_value(stream, string_table);
				position = next_position;

				array.push(value);
			}

			return (toml::Value::Array(array), position);
		},
    TOMLValueType::TABLE => {
			let (length, mut position) = stream.read_vlq();

			let mut map = toml::map::Map::new();
			for _ in 0..length {
				let (id, _) = stream.read_vlq();

				let Some(key) = string_table.get_from_index(id) else {
					panic!(); // TODO better error handling
				};
				let key = key.clone();

				let (value, next_position) = decode_value(stream, string_table);
				position = next_position;

				map.insert(key.clone(), value);
			}

			return (toml::Value::Table(map), position);
		},
	}
}

// Encode the `toml::Value` in the metadata.
impl<T> EncodeMut<u8, T> for FileMetadataEncoder<'_>
where
	T: WriteStream<u8> + U8WriteStream + Seekable
{
	fn encode_mut(&mut self, stream: &mut T) {
		// TODO root should always be a table? 7 is always written first?
		encode_value(self.metadata.get_file_metadata_toml(), stream, self.string_table);
	}
}
