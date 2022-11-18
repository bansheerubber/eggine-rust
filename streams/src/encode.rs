/// Encode an object. Object can mutate itself.
pub trait EncodeMut<Encoding, Stream, Error> {
	fn encode_mut(&mut self, stream: &mut Stream) -> Result<(), Error>;
}

/// Encode an object.
pub trait Encode<Encoding, Stream, Error> {
	fn encode(&self, stream: &mut Stream) -> Result<(), Error>;
}
