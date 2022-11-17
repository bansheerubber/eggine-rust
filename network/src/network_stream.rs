use streams::{ Decode, Encode, Endable,EncodeMut, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };
use streams::u8_io::writing::{ write_char, write_string, write_u8, write_u16, write_u32, write_u64, write_vlq, };
use streams::u8_io::reading::{
	ReadStringSafeError,
	read_char,
	read_string_safe,
	read_u8,
	read_u16,
	read_u32,
	read_u64,
	read_vlq,
};

#[derive(Debug)]
pub(crate) enum NetworkStreamError {

}

#[derive(Debug)]
pub(crate) struct NetworkWriteStream {
	buffer: Vec<u8>,
}

impl NetworkWriteStream {
	pub(crate) fn new() -> Self {
		NetworkWriteStream {
			buffer: Vec::new(),
		}
	}
}

impl U8WriteStream for NetworkWriteStream {
	fn write_u8(&mut self, byte: u8) {
		write_u8(byte, &mut self.buffer);
	}

	fn write_char(&mut self, character: char) {
		write_char(character, &mut self.buffer);
	}

	fn write_u16(&mut self, number: u16) {
		write_u16(number, &mut self.buffer);
	}

	fn write_u32(&mut self, number: u32) {
		write_u32(number, &mut self.buffer);
	}

	fn write_u64(&mut self, number: u64) {
		write_u64(number, &mut self.buffer);
	}

	fn write_vlq(&mut self, number: u64) {
		write_vlq(number, &mut self.buffer);
	}

	fn write_string(&mut self, string: &str) {
		write_string(string, &mut self.buffer);
	}

	fn write_vector(&mut self, vector: &Vec<u8>) {
		self.buffer.extend(vector);
	}
}

impl WriteStream<u8> for NetworkWriteStream {
	type Error = NetworkStreamError;
	type Export = Vec<u8>;

	fn encode_mut<T: EncodeMut<u8, Self>>(&mut self, object: &mut T) {
		object.encode_mut(self);
	}

	fn encode<T: Encode<u8, Self>>(&mut self, object: &T) {
		object.encode(self);
	}

	fn export(&mut self) -> Result<Self::Export, Self::Error> {
		Ok(std::mem::replace(&mut self.buffer, Vec::new()))
	}

	fn can_export(&self) -> bool {
		self.buffer.len() > 0
	}
}

#[derive(Debug)]
pub(crate) struct NetworkReadStream {
	buffer: Vec<u8>,
	position: StreamPosition,
}

impl NetworkReadStream {
	pub(crate) fn new() -> Self {
		NetworkReadStream {
			buffer: Vec::new(),
			position: 0,
		}
	}
}

impl U8ReadStream for NetworkReadStream {
	fn read_u8(&mut self) -> (u8, StreamPosition) {
		let (byte, delta) = read_u8(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (byte, self.position);
	}

	fn read_char(&mut self) -> (char, StreamPosition) {
		let (character, delta) = read_char(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (character, self.position);
	}

	fn read_u16(&mut self) -> (u16, StreamPosition) {
		let (number, delta) = read_u16(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (number, self.position);
	}

	fn read_u32(&mut self) -> (u32, StreamPosition) {
		let (number, delta) = read_u32(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (number, self.position);
	}

	fn read_u64(&mut self) -> (u64, StreamPosition) {
		let (number, delta) = read_u64(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (number, self.position);
	}

	fn read_vlq(&mut self) -> (u64, StreamPosition) {
		let (number, delta) = read_vlq(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		return (number, self.position);
	}
}

impl U8ReadStringSafeStream for NetworkReadStream {
	fn read_string_safe(&mut self, minimum_length: u64, maximum_length: u64)
		-> Result<(String, StreamPosition), ReadStringSafeError>
	{
		match read_string_safe(&self.buffer[(self.position as usize)..], minimum_length, maximum_length) {
			Ok((string, delta)) => {
				self.position += delta;
				Ok((string, self.position))
			},
			Err(error) => Err(error),
		}
	}
}

impl ReadStream<u8> for NetworkReadStream {
	type Error = NetworkStreamError;
	type Import = Vec<u8>;

	fn decode<T: Decode<u8, Self>>(&mut self) -> T {
		T::decode(self).0
	}

	fn can_decode(&self) -> bool {
		true
	}

	fn import(&mut self, vector: Self::Import) -> Result<(), Self::Error> {
		self.buffer = vector;
		self.position = 0;

		Ok(())
	}
}

impl Endable for NetworkReadStream {
	fn is_at_end(&mut self) -> bool {
		self.position >= self.buffer.len() as StreamPosition
	}
}
