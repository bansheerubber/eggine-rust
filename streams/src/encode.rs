/// Encode an object. Object can mutate itself.
pub trait EncodeMut<Encoding, Stream> {
	fn encode_mut(&mut self, stream: &mut Stream);
}

/// Encode an object.
pub trait Encode<Encoding, Stream> {
	fn encode(&self, stream: &mut Stream);
}
