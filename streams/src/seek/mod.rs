use crate::StreamPosition;

/// Allow a stream to seek to a position within the stream.
pub trait Seekable {
	fn seek(&mut self, position: StreamPosition);
	fn get_position(&mut self) -> StreamPosition;
}
