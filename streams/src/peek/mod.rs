/// Allow a stream to peek a primitive at it's current read/write position. For more information on how `Error` is meant
/// to be used, see `ReadStream`.
pub trait Peekable<Encoding, Error> {
	fn peek(&mut self) -> Result<Encoding, Error>;
}
