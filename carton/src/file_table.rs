use std::collections::HashMap;

use crate::file::File;
use crate::stream::Encode;
use crate::stream::writing::{ write_string, write_u64 };

/// Maps files to their absolute positions within the carton.
#[derive(Debug, Default)]
pub(crate) struct FileTable {
	files: Vec<File>,
	positions: HashMap<String, u64>,
}

impl FileTable {
	/// Adds a standalone file from disk.
	pub fn add_from_disk(&mut self, file: File) {
		self.files.push(file);
	}

	pub fn update_position(&mut self, file_name: &str, position: u64) {
		self.positions.insert(String::from(file_name), position);
	}

	pub fn get_files(&self) -> &Vec<File> {
		&self.files
	}
}

impl Encode for FileTable {
	fn encode(&self, vector: &mut Vec<u8>) {
		for (file_name, position) in self.positions.iter() {
			write_string(file_name, vector);
			write_u64(*position, vector);
		}
	}
}
