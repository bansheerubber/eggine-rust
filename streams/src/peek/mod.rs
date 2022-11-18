/// Allow a stream to peek a primitive at it's current read/write position.
pub trait Peekable<Encoding, Error> {
	fn peek(&mut self) -> Result<Encoding, Error>;
}
