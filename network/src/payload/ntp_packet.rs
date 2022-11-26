use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::error::NetworkStreamError;
use crate::ntp::NTP_MAGIC_NUMBER;

/// Header of a NTP packet.
#[derive(Debug, Eq, PartialEq)]
pub struct NtpPacketHeader {
	pub magic_number: String,
	pub packet_type: u8,
}

impl<T> Encode<u8, T, NetworkStreamError> for NtpPacketHeader
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		stream.write_string(&self.magic_number)?;
		stream.write_u8(self.packet_type)
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for NtpPacketHeader
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (magic_number, _) = stream.read_string_safe(
			NTP_MAGIC_NUMBER.len() as u64, NTP_MAGIC_NUMBER.len() as u64
		)?;

		let (packet_type, position) = stream.read_u8()?;

		Ok((NtpPacketHeader {
			packet_type,
			magic_number,
		}, position))
	}
}

/// Sent to a peer in order to get timing information from them.
#[derive(Debug, Eq, PartialEq)]
pub struct NtpRequestPacket {
	pub index: u8,
}

impl<T> Encode<u8, T, NetworkStreamError> for NtpRequestPacket
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		stream.write_u8(self.index)
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for NtpRequestPacket
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (index, position) = stream.read_u8()?;

		Ok((NtpRequestPacket {
			index,
		}, position))
	}
}

/// Sent to a peer after receiving a request for timing information.
#[derive(Debug)]
pub struct NtpResponsePacket {
	pub packet_index: u8,
	pub precision: u64,
	pub receive_time: i128,
	pub send_time: i128,
}

impl<T> Encode<u8, T, NetworkStreamError> for NtpResponsePacket
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		stream.write_u64((self.receive_time >> 64) as u64)?;
		stream.write_u64((self.receive_time & 0xFFFF_FFFF_FFFF_FFFF) as u64)?;

		stream.write_u64((self.send_time >> 64) as u64)?;
		stream.write_u64((self.send_time & 0xFFFF_FFFF_FFFF_FFFF) as u64)?;

		stream.write_u64(self.precision)
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for NtpResponsePacket
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (packet_index, _) = stream.read_u8()?;

		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, _) = stream.read_u64()?;
		let receive_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		let (precision, position) = stream.read_u64()?;

		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, _) = stream.read_u64()?;
		let send_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		Ok((NtpResponsePacket {
			packet_index,
			precision,
			receive_time,
			send_time,
		}, position))
	}
}
