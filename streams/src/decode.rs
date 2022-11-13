use crate::StreamPosition;

/// Decode an object. Returns the deserialized object along with a pointer describing how many encoded primitives were
/// read.
pub trait Decode<Encoding, Stream>: Sized {
	fn decode(stream: &mut Stream) -> (Self, StreamPosition);
}
