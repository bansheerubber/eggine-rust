use std::collections::HashMap;

use crate::StringTable;
use crate::metadata::FileMetadata;
use crate::stream::EncodeMut;
use crate::stream::reading::{read_u8, read_vlq};
use crate::stream::writing::{write_vlq, write_u8};

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

fn encode_value(value: &toml::Value, vector: &mut Vec<u8>, string_table: &mut StringTable) {
	match value {
    toml::Value::String(value) => {
			let id = if let Some(id) = string_table.get(&value) {
				id
			} else {
				string_table.insert(&value)
			};

			write_u8(TOMLValueType::STRING as u8, vector);
			write_vlq(id, vector);
		},
    toml::Value::Integer(number) => {
			write_u8(TOMLValueType::INTEGER as u8, vector);
			write_vlq(*number as u64, vector);
		},
    toml::Value::Float(_) => todo!(),
    toml::Value::Boolean(number) => {
			write_u8(TOMLValueType::BOOLEAN as u8, vector);
			write_u8(*number as u8, vector);
		},
    toml::Value::Datetime(_) => todo!(),
    toml::Value::Array(array) => {
			write_u8(TOMLValueType::ARRAY as u8, vector);
			write_vlq(array.len() as u64, vector);
			for value in array {
				encode_value(&value, vector, string_table);
			}
		},
    toml::Value::Table(map) => {
			write_u8(TOMLValueType::TABLE as u8, vector);
			write_vlq(map.len() as u64, vector);
			for (key, value) in map {
				let id = if let Some(id) = string_table.get(&key) {
					id
				} else {
					string_table.insert(&key)
				};

				write_vlq(id, vector);
				encode_value(&value, vector, string_table);
			}
		},
	}
}

pub fn decode_value<'a>(vector: &'a [u8], string_table: &mut StringTable) -> (toml::Value, &'a [u8]) {
	let (value_type, next_position) = read_u8(vector);
	let mut vector = next_position;

	match TOMLValueType::from(value_type) {
    TOMLValueType::INVALID => unreachable!(),
    TOMLValueType::STRING => {
			let (id, next_position) = read_vlq(vector);
			if let Some(string) = string_table.get_from_index(id) {
				return (toml::Value::String(string.clone()), next_position);
			} else {
				panic!(); // TODO better error handling
			}
		},
    TOMLValueType::INTEGER => {
			let (number, next_position) = read_vlq(vector);
			return (toml::Value::Integer(number as i64), next_position);
		},
    TOMLValueType::FLOAT => todo!(),
    TOMLValueType::BOOLEAN => {
			let (number, next_position) = read_u8(vector);
			return (toml::Value::Boolean(number != 0), next_position);
		},
    TOMLValueType::DATETIME => todo!(),
    TOMLValueType::ARRAY => {
			let (length, next_position) = read_vlq(vector);
			vector = next_position;

			let mut array = Vec::new();
			for _ in 0..length {
				let (value, next_position) = decode_value(vector, string_table);
				vector = next_position;

				array.push(value);
			}

			return (toml::Value::Array(array), vector);
		},
    TOMLValueType::TABLE => {
			let (length, next_position) = read_vlq(vector);
			vector = next_position;

			let mut map = toml::map::Map::new();
			for _ in 0..length {
				let (id, next_position) = read_vlq(vector);
				vector = next_position;

				let Some(key) = string_table.get_from_index(id) else {
					panic!(); // TODO better error handling
				};
				let key = key.clone();

				let (value, next_position) = decode_value(vector, string_table);
				vector = next_position;

				map.insert(key.clone(), value);
			}

			return (toml::Value::Table(map), vector);
		},
	}
}

// Encode the `toml::Value` in the metadata.
impl EncodeMut for FileMetadataEncoder<'_> {
	fn encode_mut(&mut self, vector: &mut Vec<u8>) {
		// TODO root should always be a table? 7 is always written first?
		encode_value(self.metadata.get_file_metadata_toml(), vector, self.string_table);
	}
}

/// Intermediate representation of a `FileMetadata` object.
#[derive(Debug)]
pub(crate) struct FileMetadataDecoder {

}
