use std::collections::HashMap;

use crate::file::File;
use crate::stream::{ Decode, Encode, };
use crate::stream::reading::{ read_string, read_u64, };
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

	pub(crate) fn add_from_decode(&mut self, file_name: String, position: u64) {
		self.positions.insert(file_name, position);
	}
}

impl Encode for FileTable {
	fn encode(&self, vector: &mut Vec<u8>) {
		write_u64(self.positions.len() as u64, vector);

		for (file_name, position) in self.positions.iter() {
			write_string(file_name, vector);
			write_u64(*position, vector);
		}
	}
}

impl Decode for FileTable {
	fn decode(vector: &[u8]) -> (Self, &[u8]) {
		let mut vector = vector;
		let mut table = FileTable::default();

		let (row_count, new_position) = read_u64(vector);
		vector = new_position;

		for _ in 0..row_count {
			let (file_name, new_position) = read_string(vector);
			vector = new_position;

			let (file_position, new_position) = read_u64(vector);
			vector = new_position;

			table.add_from_decode(file_name, file_position);
		}

		return (table, vector);
	}
}
