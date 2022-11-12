use std::collections::{ BTreeMap, HashMap, };

use crate::stream::Encode;
use crate::stream::writing::write_string;

/// Data structure that stores all strings used in a carton. Any strings outside of the string table are meant to be
/// referenced using their corresponding string table ID. String table IDs are within the range `0..2**60`.
///
/// Strings can have a length within a range `0..2**60`.
///
/// String table IDs are written using a variable length format with a 2 byte granularity.
#[derive(Debug, Default)]
pub struct StringTable {
	mapping: HashMap<String, u64>,
	sorted_mapping: BTreeMap<u64, String>,
}

impl StringTable {
	/// Insert a string into the string table. Duplicates cannot be inserted.
	pub fn insert(&mut self, string: &str) -> u64 {
		let index = self.mapping.len() as u64;
		self.mapping.insert(String::from(string), index);
		self.sorted_mapping.insert(index, String::from(string));
		return index;
	}

	/// Get a string's ID from the string table.
	pub fn get(&self, string: &str) -> Option<u64> {
		if let Some(index) = self.mapping.get(string) {
			Some(*index)
		} else {
			None
		}
	}
}

impl Encode for StringTable {
	fn encode(&self, vector: &mut Vec<u8>) {
		let sorted_mapping = self.sorted_mapping.iter().map(|(_, v)| v).collect::<Vec<&String>>();
		for string in sorted_mapping {
			write_string(string, vector);
		}
	}
}
