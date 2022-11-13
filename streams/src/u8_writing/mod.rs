pub mod reading;
pub mod writing;

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
}
