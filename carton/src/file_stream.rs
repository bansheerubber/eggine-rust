use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

use streams::{ Decode, Encode, EncodeMut, Endable, ReadStream, Peekable, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

#[derive(Debug)]
pub enum FileStreamError {
	FlushError,
}

#[derive(Debug)]
pub(crate) struct FileWriteStream {
	file: File,
}

impl FileWriteStream {
	pub(crate) fn new(file_name: &str) -> Self {
		FileWriteStream {
			file: OpenOptions::new()
				.write(true)
				.create(true)
				.open(file_name)
				.expect("Could not open file write stream"),
		}
	}
}

impl U8WriteStream for FileWriteStream {
	fn write_u8(&mut self, byte: u8) {
		self.file.write(&[byte]).expect("Could not write to file");
	}

	fn write_char(&mut self, character: char) {
		self.file.write(&[character as u8]).expect("Could not write to file");
	}

	fn write_u16(&mut self, number: u16) {
		const BYTES: usize = 2;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		self.file.write(&buffer).expect("Could not write to file");
	}

	fn write_u32(&mut self, number: u32) {
		const BYTES: usize = 4;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		self.file.write(&buffer).expect("Could not write to file");
	}

	fn write_u64(&mut self, number: u64) {
		const BYTES: usize = 8;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		self.file.write(&buffer).expect("Could not write to file");
	}

	fn write_vlq(&mut self, number: u64) {
		let mut shift = number;
		for _ in 0..4 {
			let number = if shift >> 15 != 0 {
				(shift as u16 & 0x7FFF) | 0x8000
			} else {
				shift as u16 & 0x7FFF
			};

			self.write_u16(number);

			shift >>= 15;

			if shift == 0 {
				break;
			}
		}
	}

	fn write_string(&mut self, string: &str) {
		self.write_vlq(string.len() as u64);
		self.file.write(string.as_bytes()).expect("Could not write to file");
	}

	fn write_vector(&mut self, vector: &Vec<u8>) {
		self.file.write(vector).expect("Could not write to file");
	}
}

impl WriteStream<u8> for FileWriteStream {
	type Error = FileStreamError;
	type Export = ();

	fn encode_mut<T: EncodeMut<u8, Self>>(&mut self, object: &mut T) {
		object.encode_mut(self);
	}

	fn encode<T: Encode<u8, Self>>(&mut self, object: &T) {
		object.encode(self);
	}

	fn export(&mut self) -> Result<Self::Export, Self::Error> {
		if let Err(_) = self.file.flush() {
			Err(Self::Error::FlushError)
		} else {
			Ok(())
		}
	}

	fn can_export(&self) -> bool {
		true
	}
}

impl Seekable for FileWriteStream {
	fn seek(&mut self, position: StreamPosition) {
		self.file.seek(SeekFrom::Start(position)).expect("Could not seek");
	}

	fn get_position(&mut self) -> StreamPosition {
		self.file.stream_position().expect("Could not get stream position")
	}
}

#[derive(Debug)]
pub struct FileReadStream {
	file: File,
	position: StreamPosition,
}

impl FileReadStream {
	pub fn new(file_name: &str) -> Self {
		FileReadStream {
			file: OpenOptions::new()
				.read(true)
				.open(file_name)
				.expect("Could not open file read stream"),
			position: 0,
		}
	}
}

impl U8ReadStream for FileReadStream {
	fn read_u8(&mut self) -> (u8, StreamPosition) {
		let mut buffer = [0];
		self.file.read(&mut buffer).expect("Could not read from file");
		self.position += 1;

		return (buffer[0], self.position);
	}

	fn read_char(&mut self) -> (char, StreamPosition) {
		let mut buffer = [0];
		self.file.read(&mut buffer).expect("Could not read from file");
		self.position += 1;

		return (buffer[0] as char, self.position);
	}

	fn read_u16(&mut self) -> (u16, StreamPosition) {
		const BYTES: usize = 2;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		self.file.read(&mut buffer).expect("Could not read from file");
		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u16) << (i * 8);
		}

		return (number, self.position);
	}

	fn read_u32(&mut self) -> (u32, StreamPosition) {
		const BYTES: usize = 4;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		self.file.read(&mut buffer).expect("Could not read from file");
		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u32) << (i * 8);
		}

		return (number, self.position);
	}

	fn read_u64(&mut self) -> (u64, StreamPosition) {
		const BYTES: usize = 8;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		self.file.read(&mut buffer).expect("Could not read from file");
		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u64) << (i * 8);
		}

		return (number, self.position);
	}

	fn read_vlq(&mut self) -> (u64, StreamPosition) {
		let mut number = 0;
		let mut read = 0;
		loop {
			let (bytes, _) = self.read_u16();
			number |= (bytes as u64 & 0x7FFF) << (read / 2 * 15);
			read += 2;

			if bytes & 0x8000 == 0 || read >= 8 {
				break;
			}
		}

		self.position += read;

		return (number, self.position);
	}

	fn read_string(&mut self) -> (String, StreamPosition) {
		let (length, _) = self.read_vlq();

		let mut buffer = vec![0; length as usize];
		self.file.read(&mut buffer).expect("Could not read string into buffer");
		self.position = self.file.stream_position().expect("Could not get stream position");

		return (String::from_utf8(buffer).expect("Could not decode utf8"), self.position);
	}
}

impl ReadStream<u8> for FileReadStream {
	type Error = FileStreamError;
	type Import = ();

	fn decode<T: Decode<u8, Self>>(&mut self) -> T {
		T::decode(self).0
	}

	fn can_decode(&self) -> bool {
		true
	}

	fn import(&mut self, _: Self::Import) -> Result<(), Self::Error> {
		Ok(())
	}
}

impl Seekable for FileReadStream {
	fn seek(&mut self, position: StreamPosition) {
		self.file.seek(SeekFrom::Start(position)).expect("Could not seek");
	}

	fn get_position(&mut self) -> StreamPosition {
		self.file.stream_position().expect("Could not get stream position")
	}
}

impl Peekable<u8> for FileReadStream {
	fn peek(&mut self) -> u8 {
		let mut buffer = [0];
		self.file.read(&mut buffer).expect("Could not peek");
		self.file.seek(SeekFrom::Current(-1)).expect("Could not seek");
		return buffer[0];
	}
}

impl Endable for FileReadStream {
	fn is_at_end(&mut self) -> bool {
		let mut buffer = [0];
		let bytes_read = self.file.read(&mut buffer).expect("Could not read from file");

		if bytes_read == 0 {
			return true;
		} else {
			self.file.seek(SeekFrom::Current(-1)).expect("Could not seek");
			return false;
		}
	}
}
