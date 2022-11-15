pub mod decode;
pub mod encode;
pub mod read_stream;
pub mod u8_io;
pub mod write_stream;

pub use decode::Decode;
pub use encode::Encode;
pub use encode::EncodeMut;
pub use read_stream::ReadStream;
pub use read_stream::StreamPosition;
pub use read_stream::StreamPositionDelta;
pub use write_stream::WriteStream;

/// Test the `u8` encoding reference implementation of read/write streams.
#[cfg(test)]
mod tests {
	use super::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
	use super::u8_io::reading::{ read_char, read_string, read_u8, read_u16, read_u32, read_u64, read_vlq, };
	use super::u8_io::U8ReadStream;
	use super::u8_io::U8WriteStream;
	use super::u8_io::writing::{ write_char, write_string, write_u8, write_u16, write_u32, write_u64, write_vlq, };

	// write stream definitions
	#[derive(Debug)]
	enum TestStreamError {

	}

	#[derive(Debug, Default)]
	struct TestWriteStream {
		buffer: Vec<u8>,
	}

	impl U8WriteStream for TestWriteStream {
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
	}

	impl WriteStream<u8> for TestWriteStream {
    type Error = TestStreamError;
		type Export = Vec<u8>;

    fn encode_mut<T: crate::EncodeMut<u8, Self>>(&mut self, object: &mut T) {
			object.encode_mut(self);
    }

    fn encode<T: Encode<u8, Self>>(&mut self, object: &T) {
			object.encode(self);
    }

    fn export(&mut self) -> Result<Self::Export, Self::Error> {
			Ok(std::mem::replace(&mut self.buffer, Vec::new()))
    }

    fn can_export(&self) -> bool {
			self.buffer.len() != 0
    }
	}

	// read stream definitions
	#[derive(Debug, Default)]
	struct TestReadStream {
		buffer: Vec<u8>,
		position: StreamPosition,
	}

	impl U8ReadStream for TestReadStream {
		fn read_u8(&mut self) -> (u8, StreamPosition) {
			let (number, read_bytes) = read_u8(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (number, self.position);
		}

		fn read_char(&mut self) -> (char, StreamPosition) {
			let (character, read_bytes) = read_char(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (character, self.position);
		}

		fn read_u16(&mut self) -> (u16, StreamPosition) {
			let (number, read_bytes) = read_u16(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (number, self.position);
		}

		fn read_u32(&mut self) -> (u32, StreamPosition) {
			let (number, read_bytes) = read_u32(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (number, self.position);
		}

		fn read_u64(&mut self) -> (u64, StreamPosition) {
			let (number, read_bytes) = read_u64(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (number, self.position);
		}

		fn read_vlq(&mut self) -> (u64, StreamPosition) {
			let (number, read_bytes) = read_vlq(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (number, self.position);
		}

		fn read_string(&mut self) -> (String, StreamPosition) {
			let (string, read_bytes) = read_string(&self.buffer[self.position as usize..]);
			self.position += read_bytes;
			return (string, self.position);
		}
	}

	impl ReadStream<u8> for TestReadStream {
		type Error = TestStreamError;
		type Import = Vec<u8>;

    fn decode<T: Decode<u8, Self>>(&mut self) -> T {
			T::decode(self).0
    }

		fn can_decode(&self) -> bool {
			return self.buffer.len() != 0;
		}

		fn import(&mut self, buffer: Self::Import) -> Result<(), Self::Error> {
			self.buffer = buffer;
			Ok(())
		}
	}

	// test object definitions
	#[derive(Debug, Eq, PartialEq)]
	struct NestedTestObject<'a> {
		signed_byte: i8,
		signed_short: i16,
		signed_int: i32,
		signed_long: i64,
		string: &'a str,
		unsigned_byte: u8,
		unsigned_short: u16,
		unsigned_int: u32,
		unsigned_long: u64,
		variable_length: u64,
	}

	impl<T> Encode<u8, T> for NestedTestObject<'_>
	where
		T: WriteStream<u8> + U8WriteStream
	{
		fn encode(&self, stream: &mut T) {
			stream.write_vlq(self.variable_length);

			stream.write_string(&self.string);

			stream.write_u8(self.unsigned_byte);
			stream.write_u16(self.unsigned_short);
			stream.write_u32(self.unsigned_int);
			stream.write_u64(self.unsigned_long);

			stream.write_u8(self.signed_byte as u8);
			stream.write_u16(self.signed_short as u16);
			stream.write_u32(self.signed_int as u32);
			stream.write_u64(self.signed_long as u64);
		}
	}

	impl<T> Decode<u8, T> for NestedTestObject<'_>
	where
		T: ReadStream<u8> + U8ReadStream
	{
    fn decode(stream: &mut T) -> (Self, StreamPosition) {
			let (variable_length, position) = stream.read_vlq();

			let string = Box::leak(stream.read_string().0.into_boxed_str());

			let unsigned_byte = stream.read_u8().0;
			let unsigned_short = stream.read_u16().0;
			let unsigned_int = stream.read_u32().0;
			let unsigned_long = stream.read_u64().0;

			let signed_byte = stream.read_u8().0 as i8;
			let signed_short = stream.read_u16().0 as i16;
			let signed_int = stream.read_u32().0 as i32;
			let signed_long = stream.read_u64().0 as i64;

			return (
				NestedTestObject {
					signed_byte,
					signed_short,
					signed_int,
					signed_long,
					string,
					unsigned_byte,
					unsigned_short,
					unsigned_int,
					unsigned_long,
					variable_length,
				},
				position,
			);
    }
	}

	#[derive(Debug, Eq, PartialEq)]
	struct TestObject<'a> {
		nested_object: NestedTestObject<'a>,
		signed_byte: i8,
		signed_short: i16,
		signed_int: i32,
		signed_long: i64,
		string: &'a str,
		unsigned_byte: u8,
		unsigned_short: u16,
		unsigned_int: u32,
		unsigned_long: u64,
		variable_length: u64,
	}

	impl<T> Encode<u8, T> for TestObject<'_>
	where
		T: WriteStream<u8> + U8WriteStream
	{
		fn encode(&self, stream: &mut T) {
			self.nested_object.encode(stream);

			stream.write_u8(self.signed_byte as u8);
			stream.write_u16(self.signed_short as u16);
			stream.write_u32(self.signed_int as u32);
			stream.write_u64(self.signed_long as u64);

			stream.write_string(&self.string);

			stream.write_u8(self.unsigned_byte);
			stream.write_u16(self.unsigned_short);
			stream.write_u32(self.unsigned_int);
			stream.write_u64(self.unsigned_long);

			stream.write_vlq(self.variable_length);
		}
	}

	impl<T> Decode<u8, T> for TestObject<'_>
	where
		T: ReadStream<u8> + U8ReadStream
	{
    fn decode(stream: &mut T) -> (Self, StreamPosition) {
			let nested_object = NestedTestObject::decode(stream).0;

			let signed_byte = stream.read_u8().0 as i8;
			let signed_short = stream.read_u16().0 as i16;
			let signed_int = stream.read_u32().0 as i32;
			let signed_long = stream.read_u64().0 as i64;

			let string = Box::leak(stream.read_string().0.into_boxed_str());

			let unsigned_byte = stream.read_u8().0;
			let unsigned_short = stream.read_u16().0;
			let unsigned_int = stream.read_u32().0;
			let unsigned_long = stream.read_u64().0;

			let (variable_length, position) = stream.read_vlq();

			return (
				TestObject {
					nested_object,
					signed_byte,
					signed_short,
					signed_int,
					signed_long,
					string,
					unsigned_byte,
					unsigned_short,
					unsigned_int,
					unsigned_long,
					variable_length,
				},
				position,
			);
    }
	}

	const TEST_OBJECT: TestObject = TestObject {
		nested_object: NestedTestObject {
			signed_byte: -7,
			signed_short: -1589,
			signed_int: -96892,
			signed_long: -906_543_840_289,
			string: "hey there how do you do",
			unsigned_byte: 1,
			unsigned_short: 5814,
			unsigned_int: 100019,
			unsigned_long: 82_457_238_382,
			variable_length: 1_930_283_129,
		},
		signed_byte: -7,
		signed_short: -1589,
		signed_int: -96892,
		signed_long: -906_543_840_289,
		string: "anyone else want to go to [funny location goes here]",
		unsigned_byte: 1,
		unsigned_short: 5814,
		unsigned_int: 100019,
		unsigned_long: 82_457_238_382,
		variable_length: 1_930_283_129,
	};

	#[test]
	fn can_export() {
		let mut stream = TestWriteStream::default();
		assert!(!stream.can_export());
		stream.encode(&TEST_OBJECT);
		assert!(stream.can_export());

		let exported = stream.export().expect("Could not export TestWriteStream");
		assert!(exported.len() > 0);
	}

	#[test]
	fn can_decode() {
		let mut stream = TestReadStream::default();
		assert!(!stream.can_decode());
		stream.import(vec![0, 1, 2, 3]).expect("Could not import TestReadStream");
		assert!(stream.can_decode());
	}

	#[test]
	fn has_equality() {
		let mut stream = TestWriteStream::default();
		stream.encode(&TEST_OBJECT);
		let exported = stream.export().expect("Could not export TestWriteStream");

		let mut stream = TestReadStream::default();
		stream.import(exported).expect("Could not import TestReadStream");
		let test_object = stream.decode::<TestObject>();
		assert!(test_object == TEST_OBJECT);
	}
}
