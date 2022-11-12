use std::fs;
use std::io::Read;
use std::sync::{ Arc, Mutex };

use crate::Carton;
use crate::file::File;
use crate::stream::Encode;
use crate::stream::writing::{ write_string, write_u64 };

/// Encodes a `FileMetadata` object
#[derive(Debug)]
pub(crate) struct FileEncoder<'a> {
	carton: Arc<Mutex<Carton>>,
	file: &'a File,
}

impl Encode for FileEncoder<'_> {
	fn encode(&self, vector: &mut Vec<u8>) {
		write_u64(self.file.get_size(), vector);
		write_string(self.file.get_file_name(), vector);
		let mut file = fs::File::open(self.file.get_file_name()).unwrap();
		file.read(vector).unwrap();
	}
}
