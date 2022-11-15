use std::collections::{ BTreeMap, HashMap, };
use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

/// Data structure that stores all strings used in a carton. Any strings outside of the string table are meant to be
/// referenced using their corresponding string table ID. String table IDs are within the range `0..2**60`.
///
/// Strings can have a length within a range `0..2**60`.
///
/// String table IDs are written using a variable length format with a 2 byte granularity.
#[derive(Debug, Default, PartialEq)]
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

	/// Get a string by ID from the string table
	pub fn get_from_index(&self, index: u64) -> Option<&String> {
		self.sorted_mapping.get(&index)
	}
}

impl<T> Encode<u8, T> for StringTable
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		stream.write_u64(self.sorted_mapping.len() as u64);

		let sorted_mapping = self.sorted_mapping.iter().map(|(_, v)| v).collect::<Vec<&String>>();
		for string in sorted_mapping {
			stream.write_string(string);
		}
	}
}

impl<T> Decode<u8, T> for StringTable
where
	T: ReadStream<u8> + U8ReadStream
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let mut table = StringTable::default();

		let (row_count, mut position) = stream.read_u64();

		for _ in 0..row_count {
			let (string, new_position) = stream.read_string();
			position = new_position;
			table.insert(&string);
		}

		return (table, position);
	}
}
