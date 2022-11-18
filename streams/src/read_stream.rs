use crate::Decode;

pub type StreamPosition = u64;
pub type StreamPositionDelta = u64;

/// Stream that imports in data with the specified `Encoding`, and can decode the data into Rust objects.
///
/// `Error` is used to communicate errors between all parts of the stream implementation. The decoding/encoding traits
/// are meant to be somewhat stream agnostic. `ReadStream`s and `WriteStream`s are to implement different stream
/// reading and writing implementations, like `U8ReadStream` and `U8WriteStream`. Instead of the decoding/encoding
/// traits referencing specific streams, they specify which traits the stream must have in order to decode/encode the
/// struct they are implemented for properly. Errors are designed in the same way, where disparate decode/encode
/// implementations will necessarily return different error types back to the `ReadStream`/`WriteStream`. To properly
/// communicate errors back and forth, I think it is best practice to define `Error` as a `Box<dyn ...>` so that
/// decode/encode implementations can return whatever error they want. This makes error handling a little more
/// complicated in `ReadStream`/`WriteStream`, but has the added benefit of making decode/encode implementations easy to
/// write, which is the primary goal of this streaming library. Decode/encode implementations are meant to pass any
/// error they encounter back up to the `ReadStream`/`WriteStream` instead of trying to handle it themselves.
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
