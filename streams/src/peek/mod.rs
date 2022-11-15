pub trait Peekable<Encoding> {
	fn peek(&mut self) -> Encoding;
}
