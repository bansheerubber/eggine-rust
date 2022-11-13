/// Encode an object. Object can mutate itself.
pub trait EncodeMut<Encoding> {
	fn encode_mut(&mut self, vector: &mut Vec<u8>);
}

/// Encode an object.
pub trait Encode<Encoding> {
	fn encode(&self, vector: &mut Vec<u8>);
}
