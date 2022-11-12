use std::fs;
use std::io::Read;
use std::sync::{ Arc, Mutex };

use crate::Carton;
use crate::file::File;
use crate::stream::Encode;
use crate::stream::writing::{ write_string, write_u64 };

use super::FileMetadataEncoder;

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileEncoder<'a> {
	pub(crate) carton: Arc<Mutex<Carton>>,
	pub(crate) file: &'a File,
}

impl Encode for FileEncoder<'_> {
	fn encode(&self, vector: &mut Vec<u8>) {
		let mut carton = self.carton.lock().unwrap();
		carton.file_table.update_position(self.file.get_file_name(), vector.len() as u64);
		drop(carton);

		if let Some(metadata) = self.file.get_metadata() {
			let encoder = FileMetadataEncoder {
				carton: self.carton.clone(),
				metadata,
			};

			encoder.encode(vector);
		}

		write_u64(self.file.get_size(), vector);
		write_string(self.file.get_file_name(), vector);
		let mut file = fs::File::open(self.file.get_file_name()).unwrap();
		file.read(vector).unwrap();
	}
}
