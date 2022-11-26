use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::error::NetworkStreamError;
use crate::server::ntp::NTP_MAGIC_NUMBER;

#[derive(Debug, Eq, PartialEq)]
pub struct NtpClientPacket {
	pub index: u8,
	pub magic_number: String,
}

impl<T> Encode<u8, T, NetworkStreamError> for NtpClientPacket
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		stream.write_u8(self.index)?;
		stream.write_string(&self.magic_number)
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for NtpClientPacket
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (index, _) = stream.read_u8()?;

		let (magic_number, position) = stream.read_string_safe(
			NTP_MAGIC_NUMBER.len() as u64, NTP_MAGIC_NUMBER.len() as u64
		)?;

		Ok((NtpClientPacket {
			index,
			magic_number,
		}, position))
	}
}

#[derive(Debug)]
pub struct NtpServerPacket {
	pub packet_index: u8,
	pub precision: u64,
	pub receive_time: i128,
	pub send_time: i128,
}

impl<T> Encode<u8, T, NetworkStreamError> for NtpServerPacket
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

impl<T> Decode<u8, T, NetworkStreamError> for NtpServerPacket
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (packet_index, _) = stream.read_u8()?;

		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, _) = stream.read_u64()?;
		let receive_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, position) = stream.read_u64()?;
		let send_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		let (precision, _) = stream.read_u64()?;

		Ok((NtpServerPacket {
			packet_index,
			precision,
			receive_time,
			send_time,
		}, position))
	}
}
