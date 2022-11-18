use crate::StreamPosition;

/// Decode an object. Returns the deserialized object along with a pointer describing how many encoded primitives were
/// read. For more information on how `Error` is meant to be used, see `ReadStream`.
pub trait Decode<Encoding, Stream, Error>: Sized {
	fn decode(stream: &mut Stream) -> Result<(Self, StreamPosition), Error>;
}
