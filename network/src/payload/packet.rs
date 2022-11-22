use std::any::Any;
use std::collections::HashSet;
use std::hash::Hash;
use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::error::{ BoxedNetworkError, NetworkError, };

use super::{ AcknowledgeMask, Payload, SubPayload, };

#[derive(Debug, Eq, PartialEq)]
pub enum PacketError {
	InvalidContinueBit,
}

impl NetworkError for PacketError {
	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_debug(&self) -> &dyn std::fmt::Debug {
		self
	}
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum PacketExtension {
	Reserved = 0,
}

/// The amount of extensions there are. There can be up to `ceil(EXTENSIONS_COUNT / 32)` extension `u32`s in a packet
/// header.
const EXTENSIONS_COUNT: usize = 1;
const EXTENSION_U32_COUNT: usize = (EXTENSIONS_COUNT + 32 - 1) / 32;

#[derive(Debug)]
pub enum PacketExtensionError {
	InvalidConversion,
}

impl TryInto<PacketExtension> for usize {
	type Error = PacketExtensionError;

	fn try_into(self) -> Result<PacketExtension, Self::Error> {
		match self {
			0 => Ok(PacketExtension::Reserved),
			_ => Err(PacketExtensionError::InvalidConversion),
		}
	}
}

/// Packets are the format used by the client and server to communicate information. They are synchronized using
/// sequence numbers that is not revealed to any other clients connected to the server.
#[derive(Debug)]
pub struct Packet {
	/// Used to determine which out of the last 128 sent packets were received correctly.
	acknowledge_mask: AcknowledgeMask,
	/// The extensions enabled on the packet.
	extensions: HashSet<PacketExtension>,
	/// The highest acknowledged sequence
	highest_acknowledged_sequence: u32,
	/// Packet contents.
	payload: Payload,
	/// The sequence number identifying this packet on the connection that sent it.
	sequence: u32,
}

#[derive(Debug, Default)]
pub(crate) struct HandledResult {
	pub(crate) acknowledged_sequences: Vec<u32>,
	pub(crate) dropped_sequences: Vec<u32>,
	pub(crate) remote_sequence: u32,
	pub(crate) new_highest_acknowledged_sequence: u32,
	pub(crate) new_acknowledge_mask: AcknowledgeMask,
}

impl Packet {
	pub fn new(sequence: u32, highest_acknowledged_sequence: u32) -> Self {
		Packet {
			acknowledge_mask: AcknowledgeMask::default(),
			extensions: HashSet::new(),
			highest_acknowledged_sequence,
			sequence,
			payload: Payload::default(),
		}
	}

	/// Prepares the sequence numbers/acknowledgement mask for the next send
	pub fn prepare(&mut self, acknowledge_mask: AcknowledgeMask, sequence: u32, last_sequence: u32) {
		self.acknowledge_mask = acknowledge_mask;
		self.sequence = sequence;
		self.highest_acknowledged_sequence = last_sequence;
	}

	/// Resets the payload.
	pub fn next(&mut self) {
		self.payload = Payload::default();
	}

	pub fn add_sub_payload(&mut self, sub_payload: SubPayload) {
		self.payload.add(sub_payload);
	}

	pub fn get_sub_payloads(&self) -> &Vec<SubPayload> {
		self.payload.get_all()
	}

	/// Calculate what the local network stack should do next based on a packet received from the remote machine. The
	/// `self` packet is assumed to be a packet from the remote machine, and this function is fed state from the local
	/// machine in order to correctly calculate what the local machine needs to do next.
	pub(crate) fn handle_sequences(
		&self,
		local_highest_acknowledge_received: Option<u32>,
		local_last_sequence_received: Option<u32>,
		local_acknowledge_mask: AcknowledgeMask
	) -> HandledResult {
		let mut result = HandledResult::default();

		result.new_acknowledge_mask = local_acknowledge_mask;

		let remote_acknowledge_mask = &self.acknowledge_mask;
		result.remote_sequence = self.sequence;

		// the sequence the remote acknowledged last
		result.new_highest_acknowledged_sequence = self.highest_acknowledged_sequence;

		// println!(
		// 	"packet info: {} {} {:?}",
		// 	result.remote_sequence,
		// 	result.new_highest_acknowledged_sequence,
		// 	remote_acknowledge_mask
		// );
		// println!(
		// 	"our info: {:?} {:?} {:?}",
		// 	local_last_sequence_received,
		// 	local_highest_acknowledge_received,
		// 	local_acknowledge_mask
		// );

		// update the acknowledge mask
		if let Some(local_last_sequence_received) = local_last_sequence_received {
			result.new_acknowledge_mask.shift(result.remote_sequence - local_last_sequence_received);
		}

		result.new_acknowledge_mask.set_first();

		// check remote acknowledge mask for packets that the remote may have not received
		if let Some(local_highest_acknowledge_received) = local_highest_acknowledge_received {
			for i in (local_highest_acknowledge_received + 1)..=result.new_highest_acknowledged_sequence {
				let tested = remote_acknowledge_mask.test(result.new_highest_acknowledged_sequence - i);
				if tested.is_some() && tested.unwrap() == true {
					result.acknowledged_sequences.push(i);
					// println!("client acknowledged: {}", i);
				} else {
					result.dropped_sequences.push(i);
					// println!("client dropped: {}", i);
				}
			}
		}

		return result;
	}
}

fn encode_extensions(extensions: &HashSet<PacketExtension>) -> [u32; EXTENSION_U32_COUNT] {
	// encode extensions
	let mut u32_array = [0; EXTENSION_U32_COUNT];
	for i in 0..EXTENSIONS_COUNT {
		// encode an extension flag into the current `u32` that we're on
		if extensions.contains(&TryInto::try_into(i).unwrap()) {
			u32_array[i / 31] = 1 << (i % 31);
		}

		// if we're finished encoding 31 bits of extensions but there's more that we have to encode after this, then set
		// the 32nd bit of the encoded `u32` to a 1 so we know that there's more extensions after this
		if i % 31 == 30 && i != EXTENSIONS_COUNT - 1 {
			u32_array[i / 31] = 1 << 31;
		}
	}

	return u32_array;
}

impl<T> Encode<u8, T, BoxedNetworkError> for Packet
where
	T: WriteStream<u8, BoxedNetworkError> + U8WriteStream<BoxedNetworkError>
{
	fn encode(&self, stream: &mut T) -> Result<(), BoxedNetworkError> {
		stream.write_u32(self.sequence)?;
		stream.write_u32(self.highest_acknowledged_sequence)?;

		stream.encode(&self.acknowledge_mask)?;

		for number in encode_extensions(&self.extensions) {
			stream.write_u32(number)?;
		}

		stream.encode(&self.payload)?;

		Ok(())
	}
}

fn decode_extensions<T>(stream: &mut T) -> Result<HashSet<PacketExtension>, BoxedNetworkError>
where
	T: ReadStream<u8, BoxedNetworkError> + U8ReadStream<BoxedNetworkError> + U8ReadStringSafeStream<BoxedNetworkError> + Endable<BoxedNetworkError>
{
	let mut extensions: HashSet<PacketExtension> = HashSet::new();

	// decode extensions
	let mut number: u32 = 0;
	for i in 0..EXTENSIONS_COUNT {
		if i % 32 == 0 {
			number = stream.read_u32()?.0;
		}

		let flag_set = number & (1 << (i % 31));
		if flag_set != 0 {
			extensions.insert(TryInto::try_into(i).unwrap());
		}

		// test last bit in number, and if false, then break since there are no more extension `u32`s to be read
		if i % 31 == 30 && (number & 0x8000_0000) == 0 {
			break;
		}
	}

	// check if the last read `u32` in the extension `u32`s has a zero continue bit. if not, error out
	if (number & 0x8000_0000) != 0 {
		return Err(Box::new(PacketError::InvalidContinueBit));
	}

	Ok(extensions)
}

impl<T> Decode<u8, T, BoxedNetworkError> for Packet
where
	T: ReadStream<u8, BoxedNetworkError> + U8ReadStream<BoxedNetworkError> + U8ReadStringSafeStream<BoxedNetworkError> + Endable<BoxedNetworkError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), BoxedNetworkError> {
		let mut packet = Packet {
			acknowledge_mask: AcknowledgeMask::default(),
			extensions: HashSet::new(),
			highest_acknowledged_sequence: 0,
			sequence: 0,
			payload: Payload::default(),
		};

		packet.sequence = stream.read_u32()?.0;
		packet.highest_acknowledged_sequence = stream.read_u32()?.0;

		packet.acknowledge_mask = stream.decode::<AcknowledgeMask>()?.0;

		packet.extensions = decode_extensions(stream)?;

		let (payload, position) = stream.decode::<Payload>()?;
		packet.payload = payload;

		Ok((packet, position))
	}
}

/// Test packet implementation.
#[cfg(test)]
mod tests {
	use std::collections::HashSet;
	use streams::{ ReadStream, WriteStream, };
	use streams::u8_io::{U8WriteStream, U8ReadStream};

	use crate::error::BoxedNetworkError;
	use crate::network_stream::{ NetworkReadStream, NetworkWriteStream };

	use super::{
		PacketError,
		PacketExtension,
		EXTENSION_U32_COUNT,
		EXTENSIONS_COUNT,
		decode_extensions,
		encode_extensions,
	};

	// custom encode function that writes flags following the extension flag specification
	fn generate_flags_stream<T>(stream: &mut T, set_all_continue_bits: bool) -> HashSet<PacketExtension>
	where
		T: WriteStream<u8, BoxedNetworkError> + U8WriteStream<BoxedNetworkError>
	{
		let mut flags = HashSet::new();
		for n in 0..EXTENSION_U32_COUNT {
			let mut number = 0;
			for i in 0..31 {
				let flag = TryInto::try_into(i + n * 31);
				if let Ok(flag) = flag {
					flags.insert(flag);
					number |= 1 << i;
				}
			}

			// incorrectly set all continue bits to 1, or set continue bits if there's `u32`s following the one we're about
			// to write
			if set_all_continue_bits {
				number |= 1 << 31;
			} else if n != EXTENSION_U32_COUNT - 1 {
				number |= 1 << 31;
			}

			stream.write_u32(number).expect("Could not write number to stream");
		}

		return flags;
	}

	// generate a flags hash set
	fn generate_valid_hashset() -> HashSet<PacketExtension> {
		let mut flags = HashSet::new();
		for i in 0..EXTENSIONS_COUNT {
			if i % 2 == 0 {
				continue;
			}

			let flag = TryInto::try_into(i);
			if let Ok(flag) = flag {
				flags.insert(flag);
			}
		}

		return flags;
	}

	/// Ensure that decoding detects incorrect continue bit set.
	#[test]
	fn extension_flags_decode1() {
		let mut write_stream = NetworkWriteStream::new();
		generate_flags_stream(&mut write_stream, true);

		// write garbage after flags
		let garbage = 0xBEEF_BEEF_BEEF_BEEF;
		write_stream.write_u64(garbage).expect("Could not write padding to stream");

		// read the data we just wrote
		let mut read_stream = NetworkReadStream::new();
		read_stream.import(
			write_stream.export().expect("Could not export test stream")
		).expect("Could not import test stream");

		// decode the extensions. since we set the continue bit incorrectly, it should emit an error
		let result = decode_extensions(&mut read_stream);
		assert_eq!(
			result.unwrap_err().as_any().downcast_ref::<PacketError>().expect("Could not downcast `PacketError`"),
			&PacketError::InvalidContinueBit
		);

		// check to see if the garbage is preserved
		assert_eq!(read_stream.read_u64().expect("Could not read number").0, garbage);
	}

	/// Ensure that decoding produces correct extension flags.
	#[test]
	fn extension_flags_decode2() {
		let mut write_stream = NetworkWriteStream::new();
		let flags = generate_flags_stream(&mut write_stream, false);

		// write garbage after flags
		let garbage = 0xBEEF_BEEF_BEEF_BEEF;
		write_stream.write_u64(garbage).expect("Could not write padding to stream");

		// read the data we just wrote
		let mut read_stream = NetworkReadStream::new();
		read_stream.import(
			write_stream.export().expect("Could not export test stream")
		).expect("Could not import test stream");

		// decode the extensions, and test them against the flags we wrote
		let result = decode_extensions(&mut read_stream);
		assert_eq!(result.expect("Could not unwrap result"), flags);

		// check to see if the garbage is preserved
		assert_eq!(read_stream.read_u64().expect("Could not read number").0, garbage);
	}

	/// Ensure that encoding produces correct extension flags upon decode.
	#[test]
	fn extension_flags_encode1() {
		// encode the flags into the stream
		let flags = generate_valid_hashset();
		let mut write_stream = NetworkWriteStream::new();
		for number in encode_extensions(&flags) {
			write_stream.write_u32(number).expect("Could not write number to stream");
		}

		// write garbage after flags
		let garbage = 0xBEEF_BEEF_BEEF_BEEF;
		write_stream.write_u64(garbage).expect("Could not write padding to stream");

		// read the data we just wrote
		let mut read_stream = NetworkReadStream::new();
		read_stream.import(
			write_stream.export().expect("Could not export test stream")
		).expect("Could not import test stream");

		// decode the extensions, and test them against the flags we wrote
		let result = decode_extensions(&mut read_stream);
		assert_eq!(result.expect("Could not unwrap result"), flags);

		// check to see if the garbage is preserved
		assert_eq!(read_stream.read_u64().expect("Could not read number").0, garbage);
	}
}
