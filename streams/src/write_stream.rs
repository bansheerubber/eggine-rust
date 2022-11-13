use crate::{ Encode, EncodeMut, };

/// Stream that encodes Rust objects into the specified `Encoding`, and transforms the data into the specified `Export`
/// type.
pub trait WriteStream<Encoding>: Sized {
	type Error: std::fmt::Debug;
	type Export;

	/// Use the `EncodeMut` trait to encode an object into the stream.
	fn encode_mut<T: EncodeMut<Encoding, Self>>(&mut self, object: &mut T);

	/// Use the `Encode` trait to encode an object into the stream.
	fn encode<T: Encode<Encoding, Self>>(&mut self, object: &T);

	/// Transforms the encoded data into the `Export` object. Consumes the stream until `can_export` returns false.
	fn export(&mut self) -> Result<Self::Export, Self::Error>;

	/// Whether or not we can perform an export.
	fn can_export(&self) -> bool;
}
