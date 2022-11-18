use crate::Decode;

pub type StreamPosition = u64;
pub type StreamPositionDelta = u64;

/// Stream that imports in data with the specified `Encoding`, and can decode the data into Rust objects.
pub trait ReadStream<Encoding, Error>: Sized {
	type Import;

	/// Use the `Decode` trait to decode an object out of the stream. Consumes the stream until `can_decode` returns
	/// false.
	fn decode<T: Decode<Encoding, Self, Error>>(&mut self) -> Result<(T, StreamPosition), Error>;

	/// Whether or not we have enough data to decode.
	fn can_decode(&self) -> bool;

	/// Import data into the stream.
	fn import(&mut self, import: Self::Import) -> Result<(), Error>;
}
