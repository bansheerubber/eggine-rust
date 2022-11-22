use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::error::BoxedNetworkError;
use crate::server::ntp_server::NTP_MAGIC_NUMBER;

#[derive(Debug, Eq, PartialEq)]
pub struct NtpClientPacket {
	pub magic_number: String,
}

impl<T> Encode<u8, T, BoxedNetworkError> for NtpClientPacket
where
	T: WriteStream<u8, BoxedNetworkError> + U8WriteStream<BoxedNetworkError>
{
	fn encode(&self, stream: &mut T) -> Result<(), BoxedNetworkError> {
		stream.write_string(&self.magic_number)
	}
}

impl<T> Decode<u8, T, BoxedNetworkError> for NtpClientPacket
where
	T: ReadStream<u8, BoxedNetworkError> + U8ReadStream<BoxedNetworkError> + U8ReadStringSafeStream<BoxedNetworkError> + Endable<BoxedNetworkError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), BoxedNetworkError> {
		let (magic_number, position) = stream.read_string_safe(
			NTP_MAGIC_NUMBER.len() as u64, NTP_MAGIC_NUMBER.len() as u64
		)?;

		Ok((NtpClientPacket {
			magic_number,
		}, position))
	}
}

#[derive(Debug)]
pub struct NtpServerPacket {
	pub receive_time: i128,
	pub send_time: i128,
}

impl<T> Encode<u8, T, BoxedNetworkError> for NtpServerPacket
where
	T: WriteStream<u8, BoxedNetworkError> + U8WriteStream<BoxedNetworkError>
{
	fn encode(&self, stream: &mut T) -> Result<(), BoxedNetworkError> {
		stream.write_u64((self.receive_time >> 64) as u64)?;
		stream.write_u64((self.receive_time & 0xFFFF_FFFF_FFFF_FFFF) as u64)?;

		stream.write_u64((self.send_time >> 64) as u64)?;
		stream.write_u64((self.send_time & 0xFFFF_FFFF_FFFF_FFFF) as u64)
	}
}

impl<T> Decode<u8, T, BoxedNetworkError> for NtpServerPacket
where
	T: ReadStream<u8, BoxedNetworkError> + U8ReadStream<BoxedNetworkError> + U8ReadStringSafeStream<BoxedNetworkError> + Endable<BoxedNetworkError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), BoxedNetworkError> {
		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, _) = stream.read_u64()?;
		let receive_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		let (lower_half, _) = stream.read_u64()?;
		let (upper_half, position) = stream.read_u64()?;
		let send_time = ((upper_half as u128) << 64 | lower_half as u128) as i128;

		Ok((NtpServerPacket {
			receive_time,
			send_time,
		}, position))
	}
}
