use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::network_stream::{ Error, NetworkStreamError, };

#[derive(Debug, Clone, Copy)]
pub enum DisconnectionReason {
	/// A client must send a `SubPayload::Disconnect` to the server with the `Requested` variant to be gracefully
	/// disconnected from the server.
	Requested,
	/// The server attempts to send a `Timeout` variant to the client's time-to-live has expired. Since a client that has
	/// timed out may not even be connected to the internet anymore, it is not expected that the client will receive the
	/// message.
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
