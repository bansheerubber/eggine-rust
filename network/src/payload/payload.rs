use streams::{ Encode, WriteStream, };
use streams::u8_io::U8WriteStream;

#[derive(Debug, Default)]
pub struct Payload {
	sub_payloads: Vec<SubPayload>,
}

impl Payload {
	pub fn add(&mut self, sub_payload: SubPayload) {
		self.sub_payloads.push(sub_payload);
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

#[derive(Debug)]
pub enum SubPayload {
	Ping(u64),
	Pong(u64),
}

pub enum SubPayloadType {
	Stream					= 1,
	CreateStream		= 2,
	Ping						= 3,
	Pong						= 4,
	DropConnection	= 5,
}

impl<T> Encode<u8, T> for SubPayload
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		match *self {
			SubPayload::Ping(time) => {
				stream.write_u8(SubPayloadType::Ping as u8);
				stream.write_u64(time);
			},
			SubPayload::Pong(time) => {
				stream.write_u8(SubPayloadType::Pong as u8);
				stream.write_u64(time);
			},
		}
	}
}
