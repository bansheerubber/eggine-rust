use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use super::DisconnectionReason;

#[derive(Debug, Default)]
pub struct Payload {
	sub_payloads: Vec<SubPayload>,
}

impl Payload {
	pub fn add(&mut self, sub_payload: SubPayload) {
		self.sub_payloads.push(sub_payload);
	}

	pub fn get_all(&self) -> &Vec<SubPayload> {
		&self.sub_payloads
	}
}

impl<T> Encode<u8, T> for Payload
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		for sub_payload in &self.sub_payloads {
			stream.encode(sub_payload);
		}
	}
}

impl<T> Decode<u8, T> for Payload
where
	T: ReadStream<u8> + U8ReadStream + U8ReadStringSafeStream + Endable
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let mut payload = Payload::default();
		while !stream.is_at_end() {
			payload.sub_payloads.push(stream.decode::<SubPayload>());
		}

		return (payload, 0); // TODO implement correct position
	}
}

#[derive(Debug)]
pub enum SubPayload {
	Disconnect(DisconnectionReason),
	Ping(u64),
	Pong(u64),
}

pub enum SubPayloadType {
	Invalid,
	Stream					= 1,
	CreateStream		= 2,
	Ping						= 3,
	Pong						= 4,
	Disconnect			= 5,
}

impl<T> Encode<u8, T> for SubPayloadType
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		let value = match *self {
			SubPayloadType::CreateStream => SubPayloadType::CreateStream as u8,
			SubPayloadType::Disconnect => SubPayloadType::Disconnect as u8,
			SubPayloadType::Ping => SubPayloadType::Ping as u8,
			SubPayloadType::Pong => SubPayloadType::Pong as u8,
			SubPayloadType::Stream => SubPayloadType::Stream as u8,
			SubPayloadType::Invalid => panic!("cannot encode invalid sub-payload type"),
		};

		stream.write_u8(value);
	}
}

impl<T> Decode<u8, T> for SubPayloadType
where
	T: ReadStream<u8> + U8ReadStream
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let (byte, position) = stream.read_u8();
		let sub_payload_type = match byte {
			1 => SubPayloadType::Stream,
			2 => SubPayloadType::CreateStream,
			3 => SubPayloadType::Ping,
			4 => SubPayloadType::Pong,
			5 => SubPayloadType::Disconnect,
			_ => SubPayloadType::Invalid,
		};

		(sub_payload_type, position)
	}
}

impl<T> Encode<u8, T> for SubPayload
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		match self {
			SubPayload::Disconnect(reason) => {
				stream.encode(&SubPayloadType::Disconnect);
				stream.encode(reason);
			}
			SubPayload::Ping(time) => {
				stream.encode(&SubPayloadType::Ping);
				stream.write_u64(*time);
			},
			SubPayload::Pong(time) => {
				stream.encode(&SubPayloadType::Pong);
				stream.write_u64(*time);
			},
		}
	}
}

impl<T> Decode<u8, T> for SubPayload
where
	T: ReadStream<u8> + U8ReadStream + U8ReadStringSafeStream
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let sub_payload_type = stream.decode::<SubPayloadType>();

		let sub_payload = match sub_payload_type {
			SubPayloadType::CreateStream => todo!(),
			SubPayloadType::Disconnect => {
				SubPayload::Disconnect(stream.decode::<DisconnectionReason>())
			},
			SubPayloadType::Ping => {
				SubPayload::Ping(stream.read_u64().0)
			},
			SubPayloadType::Pong => {
				SubPayload::Pong(stream.read_u64().0)
			},
			SubPayloadType::Stream => todo!(),
			SubPayloadType::Invalid => panic!("cannot decode invalid sub-payload type"),
		};

		return (sub_payload, 0); // TODO implement correct position
	}
}
