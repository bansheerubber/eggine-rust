use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use super::{ Payload, SubPayload, };

#[derive(Debug)]
pub struct Packet {
	pub acknowledge_mask: [u64; 2],
	pub last_sequence_number: u32,
	pub sequence_number: u32,
	payload: Payload,
}

impl Packet {
	pub fn new(sequence_number: u32, last_sequence_number: u32) -> Self {
		Packet {
			acknowledge_mask: [0; 2],
			last_sequence_number,
			sequence_number,
			payload: Payload::default(),
		}
	}

	pub fn add_sub_payload(&mut self, sub_payload: SubPayload) {
		self.payload.add(sub_payload);
	}

	pub fn get_sub_payloads(&self) -> &Vec<SubPayload> {
		self.payload.get_all()
	}
}

impl<T> Encode<u8, T> for Packet
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		stream.write_u32(self.sequence_number);
		stream.write_u32(self.last_sequence_number);

		for part in self.acknowledge_mask {
			stream.write_u64(part);
		}

		stream.encode(&self.payload);
	}
}

impl<T> Decode<u8, T> for Packet
where
	T: ReadStream<u8> + U8ReadStream + U8ReadStringSafeStream + Endable
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let mut packet = Packet {
			acknowledge_mask: [0; 2],
			last_sequence_number: 0,
			sequence_number: 0,
			payload: Payload::default(),
		};

		packet.sequence_number = stream.read_u32().0;
		packet.last_sequence_number = stream.read_u32().0;

		for i in 0..packet.acknowledge_mask.len() {
			packet.acknowledge_mask[i] = stream.read_u64().0;
		}

		packet.payload = stream.decode::<Payload>();

		return (packet, 0); // TODO validstream position;
	}
}
