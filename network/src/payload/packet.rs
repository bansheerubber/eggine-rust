use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::network_stream::Error;

use super::{ Payload, SubPayload, };

/// Packets are the format used by the client and server to communicate information. They are synchronized using
/// sequence numbers that is not revealed to any other clients connected to the server.
#[derive(Debug)]
pub struct Packet {
	/// Used to determine which out of the last 128 sent packets were received correctly.
	pub acknowledge_mask: [u64; 2],
	/// The highest acknowledged sequence we received from someone.
	pub highest_acknowledged_sequence: u32,
	/// The sequence number identifying this packet on the connection that sent it.
	pub sequence_number: u32,
	/// Packet contents.
	payload: Payload,
}

impl Packet {
	pub fn new(sequence_number: u32, highest_acknowledged_sequence: u32) -> Self {
		Packet {
			acknowledge_mask: [0; 2],
			highest_acknowledged_sequence,
			sequence_number,
			payload: Payload::default(),
		}
	}

	/// Resets the payload and configures the sequence numbers/acknowledgement mask for the next send.
	pub fn next(&mut self, last_sequence_number: u32) {
		// TODO acknowledge mask
		self.sequence_number += 1; // TODO overflow
		self.highest_acknowledged_sequence = last_sequence_number;
		self.payload = Payload::default();
	}

	pub fn add_sub_payload(&mut self, sub_payload: SubPayload) {
		self.payload.add(sub_payload);
	}

	pub fn get_sub_payloads(&self) -> &Vec<SubPayload> {
		self.payload.get_all()
	}
}

impl<T> Encode<u8, T, Error> for Packet
where
	T: WriteStream<u8, Error> + U8WriteStream<Error>
{
	fn encode(&self, stream: &mut T) -> Result<(), Error> {
		stream.write_u32(self.sequence_number)?;
		stream.write_u32(self.highest_acknowledged_sequence)?;

		for part in self.acknowledge_mask {
			stream.write_u64(part)?;
		}

		stream.encode(&self.payload)?;

		Ok(())
	}
}

impl<T> Decode<u8, T, Error> for Packet
where
	T: ReadStream<u8, Error> + U8ReadStream<Error> + U8ReadStringSafeStream<Error> + Endable<Error>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), Error> {
		let mut packet = Packet {
			acknowledge_mask: [0; 2],
			highest_acknowledged_sequence: 0,
			sequence_number: 0,
			payload: Payload::default(),
		};

		packet.sequence_number = stream.read_u32()?.0;
		packet.highest_acknowledged_sequence = stream.read_u32()?.0;

		for i in 0..packet.acknowledge_mask.len() {
			packet.acknowledge_mask[i] = stream.read_u64()?.0;
		}

		let (payload, position) = stream.decode::<Payload>()?;
		packet.payload = payload;

		Ok((packet, position))
	}
}
