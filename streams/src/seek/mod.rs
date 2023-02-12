use crate::StreamPosition;

/// Allow a stream to seek to a position within the stream. For more information on how `Error` is meant to be used, see
/// `ReadStream`.
pub trait Seekable<Error> {
	/// Seeks to the absolute position specified by position.
	fn seek(&mut self, position: StreamPosition) -> Result<(), Error>;
	/// Gets the current position in the stream.
	fn get_position(&mut self) -> Result<StreamPosition, Error>;
}
