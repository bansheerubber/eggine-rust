/// Writes one byte.
pub fn write_u8(byte: u8, vector: &mut Vec<u8>) {
	vector.push(byte);
}

/// Writes a char as a `u8`.
pub fn write_char(character: char, vector: &mut Vec<u8>) {
	vector.push(character as u8);
}

/// Writes two bytes in little-endian format.
pub fn write_u16(number: u16, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..2 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes four bytes in little-endian format.
pub fn write_u32(number: u32, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..4 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes eight bytes in little-endian format.
pub fn write_u64(number: u64, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..8 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
/// bits. 1 if there are, 0 if there aren't. Integers within the range of `0..2**60` are supported.
pub fn write_vlq(number: u64, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..4 {
		let number = if shift >> 15 != 0 {
			(shift as u16 & 0x7FFF) | 0x8000
		} else {
			shift as u16 & 0x7FFF
		};

		write_u16(number, vector);

		shift >>= 15;

		if shift == 0 {
			break;
		}
	}
}

/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
/// characters.
pub fn write_string(string: &str, vector: &mut Vec<u8>) {
	write_vlq(string.len() as u64, vector);

	for character in string.chars() {
		write_char(character, vector);
	}
}

/// Trait for a stream that implements `u8` writing.
pub trait U8WriteStream {
	/// Writes one byte.
	fn write_u8(&mut self, byte: u8);

	/// Writes a char as a `u8`.
	fn write_char(&mut self, character: char);

	/// Writes two bytes in little-endian format.
	fn write_u16(&mut self, number: u16);

	/// Writes four bytes in little-endian format.
	fn write_u32(&mut self, number: u32);

	/// Writes eight bytes in little-endian format.
	fn write_u64(&mut self, number: u64);

	/// Writes a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
	/// bits. 1 if there are, 0 if there aren't. Integers within the range of `0..2**60` are supported.
	fn write_vlq(&mut self, number: u64);

	/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
	/// characters.
	fn write_string(&mut self, string: &str);

	/// Writes a vector to file.
	fn write_vector(&mut self, vector: &Vec<u8>);
}

