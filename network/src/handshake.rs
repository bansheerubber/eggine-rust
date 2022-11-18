use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStringSafeStream, U8WriteStream, U8ReadStream, };

use crate::network_stream::{Error, NetworkStreamError};

/// The eggine's version.
#[derive(Debug, Eq, PartialEq)]
pub struct Version {
	pub branch: String,
	pub major: u16,
	pub minor: u16,
	pub revision: u16,
}

impl<T> Encode<u8, T, Error> for Version
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		stream.write_u16(self.major)?;
		stream.write_u16(self.minor)?;
		stream.write_u16(self.revision)?;
		stream.write_string(&self.branch)?;
		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for Version
where
T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringSafeStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (major, _) = stream.read_u16()?;
		let (minor, _) = stream.read_u16()?;
		let (revision, _) = stream.read_u16()?;

		let (branch, position) = stream.read_string_safe(3, 32)?;

		Ok((
			Version {
				branch,
				major,
				minor,
				revision,
			},
			position
		))
	}
}

/// Used to verify a client connection on the server.
#[derive(Debug, Eq, PartialEq)]
pub struct Handshake {
	/// Checksum of the network API. If the checksum between a client and server do not match, then they would be unable
	/// to communicate with each other.
	pub checksum: [u8; 16],
	pub sequences: (u32, u32),
	/// Version of the eggine that the client/server is running on.
	pub version: Version,
}

impl<T> Encode<u8, T, Error> for Handshake
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		stream.write_char('E')?;
		stream.write_char('G')?;
		stream.write_char('G')?;
		stream.write_char('I')?;
		stream.write_char('N')?;
		stream.write_char('E')?;

		stream.write_u32(self.sequences.0)?;
		stream.write_u32(self.sequences.1)?;

		for byte in self.checksum {
			stream.write_u8(byte)?;
		}

		stream.encode(&self.version)?;

		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for Handshake
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringSafeStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		// read the "EGGINE" magic number
		let mut magic_number = String::new();
		for _ in 0..6 {
			magic_number.push(stream.read_char()?.0);
		}

		if magic_number != "EGGINE" {
			return Err(Box::new(NetworkStreamError::InvalidMagicNumber));
		}

		// read the sequence numbers
		let (sequence1, _) = stream.read_u32()?;
		let (sequence2, _) = stream.read_u32()?;

		// read the checksum
		let mut checksum = [0; 16];
		for i in 0..16 {
			checksum[i] = stream.read_u8()?.0;
		}

		let (version, position) = stream.decode::<Version>()?;

		Ok((
			Handshake {
				checksum,
				sequences: (sequence1, sequence2),
				version,
			},
			position
		))
	}
}
