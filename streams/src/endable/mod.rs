pub trait Endable<Error> {
	fn is_at_end(&mut self) -> Result<bool, Error>;
}
