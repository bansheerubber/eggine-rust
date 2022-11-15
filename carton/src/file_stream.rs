use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

use streams::{ Decode, Encode, EncodeMut, ReadStream, Peekable, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };
use streams::u8_io::reading::{ read_char, read_u8, read_u16, read_u32, read_u64, read_vlq, };
use streams::u8_io::writing::{ write_char, write_string, write_u8, write_u16, write_u32, write_u64, write_vlq, };

#[derive(Debug)]
pub enum FileStreamError {

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
				.unwrap(),
		}
	}
}

impl U8WriteStream for FileWriteStream {
	fn write_u8(&mut self, byte: u8) {
		let mut vector = Vec::new();
		write_u8(byte, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_char(&mut self, character: char) {
		let mut vector = Vec::new();
		write_char(character, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_u16(&mut self, number: u16) {
		let mut vector = Vec::new();
		write_u16(number, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_u32(&mut self, number: u32) {
		let mut vector = Vec::new();
		write_u32(number, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_u64(&mut self, number: u64) {
		let mut vector = Vec::new();
		write_u64(number, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_vlq(&mut self, number: u64) {
		let mut vector = Vec::new();
		write_vlq(number, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_string(&mut self, string: &str) {
		let mut vector = Vec::new();
		write_string(string, &mut vector);
		self.file.write(&vector).unwrap();
	}

	fn write_vector(&mut self, vector: &Vec<u8>) {
		self.file.write(vector).unwrap();
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
		self.file.flush().unwrap();
		Ok(())
	}

	fn can_export(&self) -> bool {
		true
	}
}

impl Seekable for FileWriteStream {
	fn seek(&mut self, position: StreamPosition) {
		self.file.seek(SeekFrom::Start(position)).unwrap();
	}

	fn get_position(&mut self) -> StreamPosition {
		self.file.stream_position().unwrap()
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
				.unwrap(),
			position: 0,
		}
	}
}

impl U8ReadStream for FileReadStream {
	fn read_u8(&mut self) -> (u8, StreamPosition) {
		let mut buffer = [0];
		self.file.read(&mut buffer).unwrap();

		let (number, read_bytes) = read_u8(&buffer);
		self.position += read_bytes;
		return (number, self.position);
	}

	fn read_char(&mut self) -> (char, StreamPosition) {
		let mut buffer = [0];
		self.file.read(&mut buffer).unwrap();

		let (character, read_bytes) = read_char(&buffer);
		self.position += read_bytes;
		return (character, self.position);
	}

	fn read_u16(&mut self) -> (u16, StreamPosition) {
		let mut buffer = [0, 0];
		self.file.read(&mut buffer).unwrap();

		let (number, read_bytes) = read_u16(&buffer);
		self.position += read_bytes;
		return (number, self.position);
	}

	fn read_u32(&mut self) -> (u32, StreamPosition) {
		let mut buffer = [0, 0, 0, 0];
		self.file.read(&mut buffer).unwrap();

		let (number, read_bytes) = read_u32(&buffer);
		self.position += read_bytes;
		return (number, self.position);
	}

	fn read_u64(&mut self) -> (u64, StreamPosition) {
		let mut buffer = [0, 0, 0, 0, 0, 0, 0, 0];
		self.file.read(&mut buffer).unwrap();

		let (number, read_bytes) = read_u64(&buffer);
		self.position += read_bytes;
		return (number, self.position);
	}

	fn read_vlq(&mut self) -> (u64, StreamPosition) {
		let start = self.file.stream_position().unwrap();
		let mut buffer = [0, 0, 0, 0, 0, 0, 0, 0];
		self.file.read(&mut buffer).unwrap();

		let (number, read_bytes) = read_vlq(&buffer);
		self.position += read_bytes;

		self.file.seek(SeekFrom::Start(start + read_bytes)).unwrap();

		return (number, self.position);
	}

	fn read_string(&mut self) -> (String, StreamPosition) {
		let start = self.file.stream_position().unwrap();
		let mut buffer = [0, 0, 0, 0, 0, 0, 0, 0];
		self.file.read(&mut buffer).unwrap();

		let (length, read_bytes) = read_vlq(&buffer);

		self.file.seek(SeekFrom::Start(start + read_bytes)).unwrap();

		let mut buffer = vec![0; length as usize];
		self.file.read(&mut buffer).unwrap();
		self.position = self.file.stream_position().unwrap();

		return (String::from_utf8(buffer).unwrap(), self.position);
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
		self.file.seek(SeekFrom::Start(position)).unwrap();
	}

	fn get_position(&mut self) -> StreamPosition {
		self.file.stream_position().unwrap()
	}
}

impl Peekable<u8> for FileReadStream {
	fn peek(&mut self) -> u8 {
		let mut buffer = [0];
		self.file.read(&mut buffer).unwrap();
		self.file.seek(SeekFrom::Current(-1)).unwrap();
		return buffer[0];
	}
}
