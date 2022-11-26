use std::fmt::Debug;

use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

use crate::error::{ NetworkStreamError, NetworkStreamErrorTrait, };

#[derive(Debug, Clone, Copy)]
pub enum DisconnectionReasonError {
	InvalidDisconnectionReason,
}

impl NetworkStreamErrorTrait for DisconnectionReasonError {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

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

impl<T> Encode<u8, T, NetworkStreamError> for DisconnectionReason
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		match *self {
			DisconnectionReason::Requested => stream.write_u8(1)?,
			DisconnectionReason::Timeout => stream.write_u8(2)?,
		};
		Ok(())
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for DisconnectionReason
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let (byte, position) = stream.read_u8()?;
		let reason = match byte {
			1 => DisconnectionReason::Requested,
			2 => DisconnectionReason::Timeout,
			_ => return Err(Box::new(DisconnectionReasonError::InvalidDisconnectionReason))
		};
		Ok((reason, position))
	}
}
