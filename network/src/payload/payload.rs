use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::network_stream::{ Error, NetworkStreamError, };

use super::DisconnectionReason;

/// Describes everything sent in a packet after the header. A payload is broken up into sub-payloads, which are
/// identified using a `SubPayloadType`. Since multiple sub-payloads can be sent in each payload, multiple pieces of
/// data from different contexts can be sent in a packet.
#[derive(Debug, Default)]
pub struct Payload {
	sub_payloads: Vec<SubPayload>,
}

impl Payload {
	/// Add a sub-payload to our sub-payload list.
	pub fn add(&mut self, sub_payload: SubPayload) {
		self.sub_payloads.push(sub_payload);
	}

	/// Get a reference to our sub-payload list.
	pub fn get_all(&self) -> &Vec<SubPayload> {
		&self.sub_payloads
	}
}

impl<T> Encode<u8, T, Error> for Payload
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		for sub_payload in &self.sub_payloads {
			stream.encode(sub_payload)?;
		}
		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for Payload
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringSafeStream<Error> + Endable<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let mut payload = Payload::default();
		let mut position = 0;
		while !stream.is_at_end()? {
			let (sub_payload, new_position) = stream.decode::<SubPayload>()?;
			position = new_position;

			payload.sub_payloads.push(sub_payload);
		}

		Ok((payload, position))
	}
}

/// Represents a piece of the payload. Used to encode data exchanged between both client and server.
#[derive(Debug)]
pub enum SubPayload {
	Disconnect(DisconnectionReason),
	Ping(u64),
	Pong(u64),
}

/// Used to identify sub-payloads in payload encode/decode.
pub enum SubPayloadType {
	Stream					= 1,
	CreateStream		= 2,
	Ping						= 3,
	Pong						= 4,
	Disconnect			= 5,
}

impl<T> Encode<u8, T, Error> for SubPayloadType
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		let value = match *self {
			SubPayloadType::CreateStream => SubPayloadType::CreateStream as u8,
			SubPayloadType::Disconnect => SubPayloadType::Disconnect as u8,
			SubPayloadType::Ping => SubPayloadType::Ping as u8,
			SubPayloadType::Pong => SubPayloadType::Pong as u8,
			SubPayloadType::Stream => SubPayloadType::Stream as u8,
		};

		stream.write_u8(value)
	}
}

impl<T> Decode<u8, T, Error> for SubPayloadType
where
	T: ReadStream<u8, Error> + U8ReadStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (byte, position) = stream.read_u8()?;
		let sub_payload_type = match byte {
			1 => SubPayloadType::Stream,
			2 => SubPayloadType::CreateStream,
			3 => SubPayloadType::Ping,
			4 => SubPayloadType::Pong,
			5 => SubPayloadType::Disconnect,
			_ => return Err(Box::new(NetworkStreamError::InvalidSubPayloadType)),
		};

		Ok((sub_payload_type, position))
	}
}

impl<T> Encode<u8, T, Error> for SubPayload
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		match self {
			SubPayload::Disconnect(reason) => {
				stream.encode(&SubPayloadType::Disconnect)?;
				stream.encode(reason)?;
			}
			SubPayload::Ping(time) => {
				stream.encode(&SubPayloadType::Ping)?;
				stream.write_u64(*time)?;
			},
			SubPayload::Pong(time) => {
				stream.encode(&SubPayloadType::Pong)?;
				stream.write_u64(*time)?;
			},
		};
		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for SubPayload
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringSafeStream<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let (sub_payload_type, _) = stream.decode::<SubPayloadType>()?;
		match sub_payload_type {
			SubPayloadType::CreateStream => todo!(),
			SubPayloadType::Disconnect => {
				let (reason, position) = stream.decode::<DisconnectionReason>()?;
				Ok((SubPayload::Disconnect(reason), position))
			},
			SubPayloadType::Ping => {
				let (time, position) = stream.read_u64()?;
				Ok((SubPayload::Ping(time), position))
			},
			SubPayloadType::Pong => {
				let (time, position) = stream.read_u64()?;
				Ok((SubPayload::Pong(time), position))
			},
			SubPayloadType::Stream => todo!(),
		}
	}
}
