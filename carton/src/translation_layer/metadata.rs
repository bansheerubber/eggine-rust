use crate::StringTable;
use crate::metadata::FileMetadata;
use crate::stream::EncodeMut;
use crate::stream::writing::{write_vlq, write_u8};

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileMetadataEncoder<'a> {
	pub(crate) metadata: &'a FileMetadata,
	pub(crate) string_table: &'a mut StringTable,
}

enum TOMLValueType {
	STRING 		= 1,
	INTEGER 	= 2,
	FLOAT 		= 3,
	BOOLEAN 	= 4,
	DATETIME 	= 5,
	ARRAY 		= 6,
	TABLE			= 7,
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

// Encode the `toml::Value` in the metadata.
impl EncodeMut for FileMetadataEncoder<'_> {
	fn encode_mut(&mut self, vector: &mut Vec<u8>) {
		encode_value(self.metadata.get_file_metadata_toml(), vector, self.string_table);
	}
}

/// Intermediate representation of a `FileMetadata` object.
#[derive(Debug)]
pub(crate) struct FileMetadataDecoder {

}
