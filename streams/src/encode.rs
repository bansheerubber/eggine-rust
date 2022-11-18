/// Encode an object. Object can mutate itself. For more information on how `Error` is meant to be used, see
/// `ReadStream`.
pub trait EncodeMut<Encoding, Stream, Error> {
	fn encode_mut(&mut self, stream: &mut Stream) -> Result<(), Error>;
}

/// Encode an object. For more information on how `Error` is meant to be used, see `ReadStream`.
pub trait Encode<Encoding, Stream, Error> {
	fn encode(&self, stream: &mut Stream) -> Result<(), Error>;
}
