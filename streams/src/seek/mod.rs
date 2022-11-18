use std::fmt::Debug;

use crate::StreamPosition;

/// Allow a stream to seek to a position within the stream.
pub trait Seekable {
	type Error: Debug;

	fn seek(&mut self, position: StreamPosition) -> Result<(), Self::Error>;
	fn get_position(&mut self) -> Result<StreamPosition, Self::Error>;
}
