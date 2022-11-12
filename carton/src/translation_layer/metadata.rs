use std::sync::{ Arc, Mutex };

use crate::Carton;
use crate::metadata::FileMetadata;
use crate::stream::Encode;
use crate::stream::writing::write_vlq;

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileMetadataEncoder<'a> {
	carton: Arc<Mutex<Carton>>,
	metadata: &'a FileMetadata,
}

impl Encode for FileMetadataEncoder<'_> {
	fn encode(&self, vector: &mut Vec<u8>) {
		let mut carton = self.carton.lock().unwrap();

		for (key, value) in self.metadata.get_file_metadata_toml().values.iter() {
			let id = if let Some(id) = carton.string_table.get(&key) {
				id
			} else {
				carton.string_table.insert(&key)
			};

			// write the key's string ID
			write_vlq(id, vector);

			match value {
				toml::Value::String(value) => {
					let id = if let Some(id) = carton.string_table.get(&value) {
						id
					} else {
						carton.string_table.insert(&value)
					};

					// write the value's string ID
					write_vlq(id, vector);
				},
				toml::Value::Integer(_) => todo!("encode metadata interger"),
				toml::Value::Float(_) => todo!("encode metadata float"),
				toml::Value::Boolean(_) => todo!("encode metadata boolean"),
				toml::Value::Datetime(_) => todo!("encode metadata datetime"),
				toml::Value::Array(_) => todo!("encode metadata array"),
				toml::Value::Table(_) => todo!("encode metadata table"),
			}
		}
	}
}
