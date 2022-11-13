use crate::Decode;

/// Stream that imports in data with the specified `Encoding`, and can decode the data into Rust objects.
pub trait ReadStream<Import, Encoding, Error> {
	/// Use the `Decode` trait to decode an object out of the stream. Consumes the stream until `can_decode` returns
	/// false.
	fn decode<T: Decode<Encoding>>(&mut self) -> T;

	/// Whether or not we have enough data to decode.
	fn can_decode(&self) -> bool;

	/// Import data into the stream.
	fn import(&mut self, import: Import) -> Result<(), Error>;
}
