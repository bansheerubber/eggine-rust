use std::fmt::Debug;

/// Allow a stream to peek a primitive at it's current read/write position.
pub trait Peekable<Encoding> {
	type Error: Debug;

	fn peek(&mut self) -> Result<Encoding, Self::Error>;
}
