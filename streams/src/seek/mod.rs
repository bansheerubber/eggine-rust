use crate::StreamPosition;

pub trait Seekable {
	fn seek(&mut self, position: StreamPosition);
	fn get_position(&mut self) -> StreamPosition;
}
