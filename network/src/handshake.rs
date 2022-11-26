use std::fmt::Debug;

use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStringSafeStream, U8WriteStream, U8ReadStream, };

use crate::error::{ NetworkStreamError, NetworkStreamErrorTrait, };

#[derive(Debug, Clone, Copy)]
pub enum HandshakeError {
	InvalidMagicNumber,
}

impl NetworkStreamErrorTrait for HandshakeError {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

/// The eggine's version. The version satisfies this regex: v([0-9]+).([0-9]+).([a-zA-Z_][a-zA-Z_0-9]+)#([0-9]+).
/// 1st group: Major version. Intended for public consumption, and thus incremented arbitrarily.
/// 2nd group: Minor version. Intended for public consumption, and thus incremented arbitrarily.
/// 3rd group: Git branch the eggine was built on.
/// 4th group: Revision number. Number of commits since the last major-minor release. A major-minor version is released
/// on the eggine's default branch, and resets the revision number.
///
/// The versioning system is designed to include a human-readable canonical representation of a commit in the eggine's
/// game engine repository. This is useful for keeping track of exactly which commit any distributed version of the
/// eggine is running, helping speed up bug fixing.
#[derive(Debug, Eq, PartialEq)]
pub struct Version {
	pub branch: String,
	pub major: u16,
	pub minor: u16,
	pub revision: u16,
}

impl<T> Encode<u8, T, NetworkStreamError> for Version
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		stream.write_u16(self.major)?;
		stream.write_u16(self.minor)?;
		stream.write_u16(self.revision)?;
		stream.write_string(&self.branch)?;
		Ok(())
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for Version
where
T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
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
	/// The identifier that can uniquely identify clients in NTP packets.
	pub ntp_id: u32,
	/// Used to instantiate packet sequence numbers between the client and server. The server initializes all sequence
	/// numbers.
	pub sequences: (u32, u32),
	/// Version of the eggine that the client/server is running on.
	pub version: Version,
}

impl Handshake {
	/// Tests if the two handshakes are compatible.
	pub fn is_compatible(&self, other: &Handshake) -> bool {
		self.checksum == other.checksum
	}
}

impl<T> Encode<u8, T, NetworkStreamError> for Handshake
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		// write magic number
		stream.write_char('E')?;
		stream.write_char('G')?;
		stream.write_char('G')?;
		stream.write_char('I')?;
		stream.write_char('N')?;
		stream.write_char('E')?;

		// write ntp id
		stream.write_u32(self.ntp_id)?;

		// write sequences
		stream.write_u32(self.sequences.0)?;
		stream.write_u32(self.sequences.1)?;

		// write network checksum
		for byte in self.checksum {
			stream.write_u8(byte)?;
		}

		stream.encode(&self.version)?;

		Ok(())
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for Handshake
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		// read the "EGGINE" magic number
		let mut magic_number = String::new();
		for _ in 0..6 {
			magic_number.push(stream.read_char()?.0);
		}

		if magic_number != "EGGINE" {
			return Err(Box::new(HandshakeError::InvalidMagicNumber));
		}

		// read ntp id
		let (ntp_id, _) = stream.read_u32()?;

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
				ntp_id,
				sequences: (sequence1, sequence2),
				version,
			},
			position
		))
	}
}
