use crate::StreamPosition;

/// Allow a stream to seek to a position within the stream.
pub trait Seekable<Error> {
	fn seek(&mut self, position: StreamPosition) -> Result<(), Error>;
	fn get_position(&mut self) -> Result<StreamPosition, Error>;
}
