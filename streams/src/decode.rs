/// Decode an object. Returns the deserialized object along with a pointer describing how many encoded primitives were
/// read.
pub trait Decode<Encoding>: Sized {
	fn decode(vector: &[u8]) -> (Self, &[u8]);
}
