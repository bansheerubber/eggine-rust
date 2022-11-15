use crate::{ StreamPosition, StreamPositionDelta, };

/// Reads one byte.
pub fn read_u8(vector: &[u8]) -> (u8, StreamPositionDelta) {
	(vector[0], 1)
}

/// Reads a `u8` and converts it into a `char`.
pub fn read_char(vector: &[u8]) -> (char, StreamPositionDelta) {
	(vector[0] as char, 1)
}

/// Reads two bytes in little-endian format
pub fn read_u16(vector: &[u8]) -> (u16, StreamPositionDelta) {
	let mut number = 0;
	for i in 0..2 {
		let (byte, _) = read_u8(&vector[i..]);
		number |= (byte as u16) << (i * 8);
	}
	return (number, 2);
}

/// Reads four bytes in little-endian format
pub fn read_u32(vector: &[u8]) -> (u32, StreamPositionDelta) {
	let mut number = 0;
	for i in 0..4 {
		let (byte, _) = read_u8(&vector[i..]);
		number |= (byte as u32) << (i * 8);
	}
	return (number, 4);
}

/// Reads eight bytes in little-endian format
pub fn read_u64(vector: &[u8]) -> (u64, StreamPositionDelta) {
	let mut number = 0;
	for i in 0..8 {
		let (byte, _) = read_u8(&vector[i..]);
		number |= (byte as u64) << (i * 8);
	}
	return (number, 8);
}

/// Reads a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
/// bits. 1 if there are, 0 if there aren't. Integers within the range of `0..2**60` are supported.
pub fn read_vlq(vector: &[u8]) -> (u64, StreamPositionDelta) {
	let mut number = 0;
	let mut read = 0;
	loop {
		let (bytes, _) = read_u16(&vector[read..]);
		number |= (bytes as u64 & 0x7FFF) << (read / 2 * 15);
		read += 2;

		if bytes & 0x8000 == 0 || read >= 8 {
			break;
		}
	}
	return (number, read as StreamPositionDelta);
}

/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
/// characters.
pub fn read_string(vector: &[u8]) -> (String, StreamPositionDelta) {
	let (length, read_bytes) = read_vlq(vector);

	let mut output = String::new();
	for i in 0..length as usize {
		output.push(vector[read_bytes as usize + i] as char);
	}
	return (output, read_bytes + length);
}

/// Trait for a stream that implements `u8` reading.
pub trait U8ReadStream {
	/// Reads one byte.
	fn read_u8(&mut self) -> (u8, StreamPosition);

	/// Reads a char as a `u8`.
	fn read_char(&mut self) -> (char, StreamPosition);

	/// Reads two bytes in little-endian format.
	fn read_u16(&mut self) -> (u16, StreamPosition);

	/// Reads four bytes in little-endian format.
	fn read_u32(&mut self) -> (u32, StreamPosition);

	/// Reads eight bytes in little-endian format.
	fn read_u64(&mut self) -> (u64, StreamPosition);

	/// Reads a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
	/// bits. 1 if there are, 0 if there aren't. Integers within the range of `0..2**60` are supported.
	fn read_vlq(&mut self) -> (u64, StreamPosition);

	/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
	/// characters.
	fn read_string(&mut self) -> (String, StreamPosition);
}
