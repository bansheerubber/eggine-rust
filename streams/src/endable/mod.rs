/// Describes a stream that can have an end. For more information on how `Error` is meant to be used, see `ReadStream`.
pub trait Endable<Error> {
	fn is_at_end(&mut self) -> Result<bool, Error>;
}
