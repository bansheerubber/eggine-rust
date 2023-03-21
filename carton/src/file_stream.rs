use std::fs::{ File, OpenOptions, };
use std::io::{ Read, Seek, SeekFrom, Write, };

use streams::u8_io::reading::U8ReadStringStream;
use streams::{ Decode, Encode, EncodeMut, Endable, ReadStream, Peekable, Seekable, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::{ CartonError, Error, };

#[derive(Debug)]
pub enum FileStreamError {
	Flush,
	NoneFile,
}

#[derive(Debug)]
pub(crate) struct FileWriteStream {
	file: Option<File>,
}

impl FileWriteStream {
	pub(crate) fn new(file_name: &str) -> Result<Self, Error> {
		let file = match OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open(file_name)
		{
			Ok(file) => file,
			Err(error) => return Err(Box::new(CartonError::FileError(error))),
		};

		Ok(FileWriteStream {
			file: Some(file),
		})
	}

	fn get_file_mut(&mut self) -> Result<&mut File, Error> {
		match self.file.as_mut() {
			Some(file) => Ok(file),
			None => Err(Box::new(CartonError::NoFile))
		}
	}
}

impl U8WriteStream<Error> for FileWriteStream {
	fn write_u8(&mut self, byte: u8) -> Result<(), Error> {
		if let Err(error) = self.get_file_mut()?.write(&[byte]) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_char(&mut self, character: char) -> Result<(), Error> {
		if let Err(error) = self.get_file_mut()?.write(&[character as u8]) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_u16(&mut self, number: u16) -> Result<(), Error> {
		const BYTES: usize = 2;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		if let Err(error) = self.get_file_mut()?.write(&buffer) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_u32(&mut self, number: u32) -> Result<(), Error> {
		const BYTES: usize = 4;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		if let Err(error) = self.get_file_mut()?.write(&buffer) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_u64(&mut self, number: u64) -> Result<(), Error> {
		const BYTES: usize = 8;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		let mut shift = number;
		for i in 0..BYTES {
			buffer[i] = (shift & 0xFF) as u8;
			shift >>= 8;
		}

		if let Err(error) = self.get_file_mut()?.write(&buffer) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_vlq(&mut self, number: u64) -> Result<(), Error> {
		let mut shift = number;
		for _ in 0..4 {
			let number = if shift >> 15 != 0 {
				(shift as u16 & 0x7FFF) | 0x8000
			} else {
				shift as u16 & 0x7FFF
			};

			self.write_u16(number)?;

			shift >>= 15;

			if shift == 0 {
				break;
			}
		}

		Ok(())
	}

	fn write_string(&mut self, string: &str) -> Result<(), Error> {
		self.write_vlq(string.len() as u64)?;
		if let Err(error) = self.get_file_mut()?.write(string.as_bytes()) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn write_vector(&mut self, vector: &Vec<u8>) -> Result<(), Error> {
		if let Err(error) = self.get_file_mut()?.write(vector) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}
}

impl Write for FileWriteStream {
	fn write(&mut self, buffer: &[u8]) -> Result<usize, std::io::Error> {
		let file = match self.get_file_mut() {
			Ok(file) => file,
			Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "File not found")),
		};

		file.write(buffer)
	}

	fn flush(&mut self) -> Result<(), std::io::Error> {
		Ok(())
	}
}

impl WriteStream<u8, Error> for FileWriteStream {
	type Export = File;

	fn encode_mut<T>(&mut self, object: &mut T) -> Result<(), Error>
	where
		T: EncodeMut<u8, Self, Error>
	{
		object.encode_mut(self)
	}

	fn encode<T>(&mut self, object: &T) -> Result<(), Error>
	where
		T: Encode<u8, Self, Error>
	{
		object.encode(self)
	}

	fn export(&mut self) -> Result<Self::Export, Error> {
		if let Err(_) = self.get_file_mut()?.flush() {
			Err(Box::new(FileStreamError::Flush))
		} else if self.file.is_none() {
			Err(Box::new(FileStreamError::NoneFile))
		} else {
			Ok(std::mem::replace(&mut self.file, None).unwrap())
		}
	}

	fn can_export(&self) -> bool {
		self.file.is_some()
	}
}

impl Seekable<Error> for FileWriteStream {
	fn seek(&mut self, position: StreamPosition) -> Result<(), Error> {
		if let Err(error) = self.get_file_mut()?.seek(SeekFrom::Start(position)) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn get_position(&mut self) -> Result<StreamPosition, Error> {
		match self.get_file_mut()?.stream_position() {
			Ok(position) => Ok(position),
			Err(error) => Err(Box::new(CartonError::FileError(error))),
		}
	}
}

#[derive(Debug)]
pub struct FileReadStream {
	file: File,
	position: StreamPosition,
}

impl FileReadStream {
	pub fn new(file_name: &str) -> Result<Self, Error> {
		let file = match OpenOptions::new()
			.read(true)
			.open(file_name)
		{
			Ok(file) => file,
			Err(error) => return Err(Box::new(CartonError::FileError(error))),
		};

		Ok(FileReadStream {
			file,
			position: 0,
		})
	}
}

impl U8ReadStream<Error> for FileReadStream {
	fn read_u8(&mut self) -> Result<(u8, StreamPosition), Error> {
		let mut buffer = [0];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += 1;

		Ok((buffer[0], self.position))
	}

	fn read_char(&mut self) -> Result<(char, StreamPosition), Error> {
		let mut buffer = [0];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += 1;

		Ok((buffer[0] as char, self.position))
	}

	fn read_u16(&mut self) -> Result<(u16, StreamPosition), Error> {
		const BYTES: usize = 2;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u16) << (i * 8);
		}

		Ok((number, self.position))
	}

	fn read_u32(&mut self) -> Result<(u32, StreamPosition), Error> {
		const BYTES: usize = 4;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u32) << (i * 8);
		}

		Ok((number, self.position))
	}

	fn read_u64(&mut self) -> Result<(u64, StreamPosition), Error> {
		const BYTES: usize = 8;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += BYTES as u64;

		let mut number = 0;
		for i in 0..BYTES {
			number |= (buffer[i] as u64) << (i * 8);
		}

		Ok((number, self.position))
	}

	fn read_vlq(&mut self) -> Result<(u64, StreamPosition), Error> {
		let mut number = 0;
		let mut read = 0;
		loop {
			let (bytes, _) = self.read_u16()?;
			number |= (bytes as u64 & 0x7FFF) << (read / 2 * 15);
			read += 2;

			if bytes & 0x8000 == 0 || read >= 8 {
				break;
			}
		}

		self.position += read;

		Ok((number, self.position))
	}

	fn read_vector(&mut self, length: usize) -> Result<(Vec<u8>, StreamPosition), Error> {
		let mut output = Vec::new();
		for _ in 0..length {
			output.push(self.read_u8()?.0);
		}

		self.position += length as u64;

		Ok((output, self.position))
	}
}

impl U8ReadStringStream<Error> for FileReadStream {
	fn read_string(&mut self) -> Result<(String, StreamPosition), Error> {
		let (length, _) = self.read_vlq()?;

		let mut buffer = vec![0; length as usize];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position = match self.file.stream_position() {
			Ok(position) => position,
			Err(error) => return Err(Box::new(CartonError::FileError(error))),
		};

		match String::from_utf8(buffer) {
    	Ok(string) => Ok((string, self.position)),
    	Err(error) => Err(Box::new(CartonError::FromUtf8(error))),
		}
	}
}

impl ReadStream<u8, Error> for FileReadStream {
	type Import = ();

	fn decode<T>(&mut self) -> Result<(T, StreamPosition), Error>
	where
		T: Decode<u8, Self, Error>
	{
		T::decode(self)
	}

	fn can_decode(&self) -> bool {
		true
	}

	fn import(&mut self, _: Self::Import) -> Result<(), Error> {
		Ok(())
	}
}

impl Seekable<Error> for FileReadStream {
	fn seek(&mut self, position: StreamPosition) -> Result<(), Error> {
		if let Err(error) = self.file.seek(SeekFrom::Start(position)) {
			Err(Box::new(CartonError::FileError(error)))
		} else {
			Ok(())
		}
	}

	fn get_position(&mut self) -> Result<StreamPosition, Error> {
		match self.file.stream_position() {
			Ok(position) => Ok(position),
			Err(error) => Err(Box::new(CartonError::FileError(error))),
		}
	}
}

impl Peekable<u8, Error> for FileReadStream {
	fn peek(&mut self) -> Result<u8, Error> {
		let mut buffer = [0];
		if let Err(error) = self.file.read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		if let Err(error) = self.file.seek(SeekFrom::Current(-1)) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		Ok(buffer[0])
	}
}

impl Endable<Error> for FileReadStream {
	fn is_at_end(&mut self) -> Result<bool, Error> {
		let mut buffer = [0];
		let bytes_read = match self.file.read(&mut buffer) {
			Ok(bytes_read) => bytes_read,
			Err(error) => return Err(Box::new(CartonError::FileError(error))),
		};

		if bytes_read == 0 {
			Ok(true)
		} else {
			if let Err(error) = self.file.seek(SeekFrom::Current(-1)) {
				Err(Box::new(CartonError::FileError(error)))
			} else {
				Ok(false)
			}
		}
	}
}
