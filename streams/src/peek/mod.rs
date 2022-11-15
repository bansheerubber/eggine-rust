/// Allow a stream to peek a primitive at it's current read/write position.
pub trait Peekable<Encoding> {
	fn peek(&mut self) -> Encoding;
}
