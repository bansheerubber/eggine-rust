use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::network_stream::{ Error, NetworkStreamError, };

#[derive(Debug, Clone, Copy)]
pub enum DisconnectionReason {
	Invalid,
	Requested,
	Timeout,
}

impl<T> Encode<u8, T, Error> for DisconnectionReason
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		match *self {
			DisconnectionReason::Requested => stream.write_u8(1)?,
			DisconnectionReason::Timeout => stream.write_u8(2)?,
			DisconnectionReason::Invalid => return Err(Box::new(NetworkStreamError::InvalidDisconnectionReason)),
		};
		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for DisconnectionReason
where
	T: ReadStream<u8, Error> + U8ReadStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (byte, position) = stream.read_u8()?;
		let reason = match byte {
			1 => DisconnectionReason::Requested,
			2 => DisconnectionReason::Timeout,
			_ => return Err(Box::new(NetworkStreamError::InvalidDisconnectionReason))
		};
		Ok((reason, position))
	}
}
