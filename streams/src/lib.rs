pub mod decode;
pub mod encode;
pub mod read_stream;
pub mod u8_writing;
pub mod write_stream;

pub use decode::Decode;
pub use encode::Encode;
pub use encode::EncodeMut;
pub use read_stream::ReadStream;
pub use write_stream::WriteStream;

#[cfg(test)]
mod tests {
	use super::u8_writing::U8WriteStream;
	use super::u8_writing::writing::{ write_string, write_u8, write_char, write_u16, write_u32, write_u64, write_vlq, };
	use super::{ Encode, WriteStream };

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

	// test object definitions
	#[derive(Debug, Eq, PartialEq)]
	struct NestedTestObject<'a> {
		signed_byte: u8,
		signed_short: u16,
		signed_int: u32,
		signed_long: u64,
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

	#[derive(Debug, Eq, PartialEq)]
	struct TestObject<'a> {
		nested_object: NestedTestObject<'a>,
		signed_byte: u8,
		signed_short: u16,
		signed_int: u32,
		signed_long: u64,
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

	const TEST_OBJECT: TestObject = TestObject {
		nested_object: NestedTestObject {
			signed_byte: 7,
			signed_short: 1589,
			signed_int: 96892,
			signed_long: 906_543_840_289,
			string: "hey there how do you do",
			unsigned_byte: 1,
			unsigned_short: 5814,
			unsigned_int: 100019,
			unsigned_long: 82_457_238_382,
			variable_length: 1_930_283_129,
		},
		signed_byte: 7,
		signed_short: 1589,
		signed_int: 96892,
		signed_long: 906_543_840_289,
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
		let mut stream = TestWriteStream::default();
		assert!(!stream.can_export());
		stream.encode(&TEST_OBJECT);
		assert!(stream.can_export());

		let exported = stream.export().expect("Could not export TestWriteStream");
		assert!(exported.len() > 0);
	}
}
