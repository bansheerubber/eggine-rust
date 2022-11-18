use streams::{ Decode, Encode, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8WriteStream, };

#[derive(Debug, Clone, Copy)]
pub enum DisconnectionReason {
	Invalid,
	Requested,
	Timeout,
}

impl<T> Encode<u8, T> for DisconnectionReason
where
	T: WriteStream<u8> + U8WriteStream
{
	fn encode(&self, stream: &mut T) {
		match *self {
			DisconnectionReason::Requested => stream.write_u8(1),
			DisconnectionReason::Timeout => stream.write_u8(2),
			DisconnectionReason::Invalid => panic!("cannot encode invalid disconnection reaosn"),
		};
	}
}

impl<T> Decode<u8, T> for DisconnectionReason
where
	T: ReadStream<u8> + U8ReadStream
{
	fn decode(stream: &mut T) -> (Self, StreamPosition) {
		let (byte, position) = stream.read_u8();
		let reason = match byte {
			1 => DisconnectionReason::Requested,
			2 => DisconnectionReason::Timeout,
			_ => DisconnectionReason::Invalid,
		};
		return (reason, position);
	}
}
