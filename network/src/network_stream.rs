use std::fmt::Debug;
use streams::{ Decode, Encode, Endable,EncodeMut, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };
use streams::u8_io::writing::{ write_char, write_string, write_u8, write_u16, write_u32, write_u64, write_vlq, };
use streams::u8_io::reading::{ read_char, read_string_safe, read_u8, read_u16, read_u32, read_u64, read_vlq, };

/// Describes the type of error encountered while working with a network stream.
#[derive(Debug)]
pub enum NetworkStreamError {
	InvalidDisconnectionReason,
	InvalidMagicNumber,
	InvalidSubPayloadType,
}

/// Network stream error, for use in generics. In order to interact with the network stream subsystem, stream trait
/// implementations must use this error generic.
pub type Error = Box<dyn Debug + 'static>;

/// Used to write data over the network. Network streams encode data into bytes, and have corresponding U8*Stream
/// implementations. Since network streams are built upon a UDP-based protocol, data is necessarily processed in
/// discrete chunks. The implementation of `NetworkWriteStream` reflects this, and data is exported in discrete chunks
/// as well.
///
/// The network stream has two primary uses for the client and server:
/// 1. Encoding/decoding raw data straight into/from a send/recv call.
/// 2. Not implemented yet: An interface to easily exchange data over the network. Streams will automatically handle
/// dropped packet resending, packet reordering, along with other standard network protocol features. Network streams
/// will be created in read/write pairs on both the client and server, and will have an interface reminiscent of other
/// stream APIs in Rust (see stuff like tokio::sync::mpsc, C++ IO streams, etc).
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

impl U8WriteStream<Error> for NetworkWriteStream {
	fn write_u8(&mut self, byte: u8) -> Result<(), Error> {
		write_u8(byte, &mut self.buffer);
		Ok(())
	}

	fn write_char(&mut self, character: char) -> Result<(), Error> {
		write_char(character, &mut self.buffer);
		Ok(())
	}

	fn write_u16(&mut self, number: u16) -> Result<(), Error> {
		write_u16(number, &mut self.buffer);
		Ok(())
	}

	fn write_u32(&mut self, number: u32) -> Result<(), Error> {
		write_u32(number, &mut self.buffer);
		Ok(())
	}

	fn write_u64(&mut self, number: u64) -> Result<(), Error> {
		write_u64(number, &mut self.buffer);
		Ok(())
	}

	fn write_vlq(&mut self, number: u64) -> Result<(), Error> {
		write_vlq(number, &mut self.buffer);
		Ok(())
	}

	fn write_string(&mut self, string: &str) -> Result<(), Error> {
		write_string(string, &mut self.buffer);
		Ok(())
	}

	fn write_vector(&mut self, vector: &Vec<u8>) -> Result<(), Error> {
		self.buffer.extend(vector);
		Ok(())
	}
}

impl WriteStream<u8, Error> for NetworkWriteStream {
	type Export = Vec<u8>;

	fn encode_mut<T: EncodeMut<u8, Self, Error>>(&mut self, object: &mut T) -> Result<(), Error> {
		object.encode_mut(self)
	}

	fn encode<T: Encode<u8, Self, Error>>(&mut self, object: &T) -> Result<(), Error> {
		object.encode(self)
	}

	fn export(&mut self) -> Result<Self::Export, Error> {
		Ok(std::mem::replace(&mut self.buffer, Vec::new()))
	}

	fn can_export(&self) -> bool {
		self.buffer.len() > 0
	}
}

/// Used to read data over the network. Network streams encode data into bytes, and have corresponding U8*Stream
/// implementations. Since network streams are built upon a UDP-based protocol, data is necessarily imported in discrete
/// chunks. The implementation of `NetworkReadStream` reflects this, and gives encode/decode implementations to
/// determine when the end of a discrete chunk is reached via the `Endable` trait.
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

impl U8ReadStream<Error> for NetworkReadStream {
	fn read_u8(&mut self) -> Result<(u8, StreamPosition), Error> {
		let (byte, delta) = read_u8(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((byte, self.position))
	}

	fn read_char(&mut self) -> Result<(char, StreamPosition), Error> {
		let (character, delta) = read_char(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((character, self.position))
	}

	fn read_u16(&mut self) -> Result<(u16, StreamPosition), Error> {
		let (number, delta) = read_u16(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((number, self.position))
	}

	fn read_u32(&mut self) -> Result<(u32, StreamPosition), Error> {
		let (number, delta) = read_u32(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((number, self.position))
	}

	fn read_u64(&mut self) -> Result<(u64, StreamPosition), Error> {
		let (number, delta) = read_u64(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((number, self.position))
	}

	fn read_vlq(&mut self) -> Result<(u64, StreamPosition), Error> {
		let (number, delta) = read_vlq(&self.buffer[(self.position as usize)..]);
		self.position += delta;
		Ok((number, self.position))
	}
}

impl U8ReadStringSafeStream<Error> for NetworkReadStream {
	fn read_string_safe(&mut self, minimum_length: u64, maximum_length: u64)
		-> Result<(String, StreamPosition), Error>
	{
		match read_string_safe(&self.buffer[(self.position as usize)..], minimum_length, maximum_length) {
			Ok((string, delta)) => {
				self.position += delta;
				Ok((string, self.position))
			},
			Err(error) => Err(Box::new(error)),
		}
	}
}

impl ReadStream<u8, Error> for NetworkReadStream {
	type Import = Vec<u8>;

	fn decode<T: Decode<u8, Self, Error>>(&mut self) -> Result<(T, StreamPosition), Error> {
		T::decode(self)
	}

	// TODO have some sort of checksum/parity bit that determines if whatever we just imported is garbage data or not
	fn can_decode(&self) -> bool {
		true
	}

	fn import(&mut self, vector: Self::Import) -> Result<(), Error> {
		self.buffer = vector;
		self.position = 0;

		Ok(())
	}
}

impl Endable<Error> for NetworkReadStream {
	fn is_at_end(&mut self) -> Result<bool, Error> {
		Ok(self.position >= self.buffer.len() as StreamPosition)
	}
}
