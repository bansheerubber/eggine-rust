use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use streams::u8_io::reading::U8ReadStringStream;
use streams::{ Endable, Peekable, Seekable, StreamPosition, };
use streams::u8_io::U8ReadStream;

use crate::file::File;
use crate::{ Carton, CartonError, Error, };

/// Reads a file from the carton. All operations are relative to the file's position within the carton.
#[derive(Debug)]
pub struct CartonFileReadStream<'a> {
	pub carton: &'a Carton,
	pub file: &'a File,
	/// The stream's position is virtual and is relative to the position of the file offset in the carton file.
	pub position: StreamPosition,
}

impl<'a> CartonFileReadStream<'a> {
	pub fn new(carton: &'a Carton, file: &'a File) -> Result<Self, Error> {
		Ok(CartonFileReadStream {
			carton,
			file,
			position: 0,
		})
	}

	fn reset_seek(&self) -> Result<(), Error> {
		let file_position = self.carton.file_table.get_file_positions()[self.file.get_file_name()];

		if let Err(error) = self.carton.file.as_ref().unwrap().seek(SeekFrom::Start(file_position + self.position)) {
			return Err(Box::new(CartonError::FileError(error)));
		} else {
			Ok(())
		}
	}
}

impl<'a> U8ReadStream<Error> for CartonFileReadStream<'a> {
	fn read_u8(&mut self) -> Result<(u8, StreamPosition), Error> {
		self.reset_seek()?;

		let mut buffer = [0];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += 1;

		Ok((buffer[0], self.position))
	}

	fn read_char(&mut self) -> Result<(char, StreamPosition), Error> {
		let mut buffer = [0];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += 1;

		Ok((buffer[0] as char, self.position))
	}

	fn read_u16(&mut self) -> Result<(u16, StreamPosition), Error> {
		const BYTES: usize = 2;

		self.reset_seek()?;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
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

		self.reset_seek()?;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
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

		self.reset_seek()?;

		let mut buffer: [u8; BYTES] = [0; BYTES];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
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
		self.reset_seek()?;

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
		self.reset_seek()?;

		let mut buffer = vec![0; length as usize];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position += length as u64;

		Ok((buffer, self.position))
	}
}

impl<'a> U8ReadStringStream<Error> for CartonFileReadStream<'a> {
	fn read_string(&mut self) -> Result<(String, StreamPosition), Error> {
		self.reset_seek()?;

		let (length, position) = self.read_vlq()?;

		let mut buffer = vec![0; length as usize];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		self.position = position + length;

		match String::from_utf8(buffer) {
    	Ok(string) => Ok((string, self.position)),
    	Err(error) => Err(Box::new(CartonError::FromUtf8(error))),
		}
	}
}

impl<'a> Seekable<Error> for CartonFileReadStream<'a> {
	fn seek(&mut self, position: StreamPosition) -> Result<(), Error> {
		self.position = position;
		self.reset_seek()
	}

	fn get_position(&mut self) -> Result<StreamPosition, Error> {
		Ok(self.position)
	}
}

impl<'a> Peekable<u8, Error> for CartonFileReadStream<'a> {
	fn peek(&mut self) -> Result<u8, Error> {
		self.reset_seek()?;

		let mut buffer = [0];
		if let Err(error) = self.carton.file.as_ref().unwrap().read(&mut buffer) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		if let Err(error) = self.carton.file.as_ref().unwrap().seek(SeekFrom::Current(-1)) {
			return Err(Box::new(CartonError::FileError(error)));
		}

		Ok(buffer[0])
	}
}

impl<'a> Endable<Error> for CartonFileReadStream<'a> {
	fn is_at_end(&mut self) -> Result<bool, Error> {
		if self.position >= self.file.get_size() {
			Ok(true)
		} else {
			Ok(false)
		}
	}
}
