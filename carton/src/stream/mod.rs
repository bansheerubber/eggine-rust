pub mod reading;
pub mod writing;

/// Byte stream for reading/writing data formats into a file, over the network, etc. Streams do not describe the data
/// format used, and instead facilitate the transport of the format.
#[derive(Debug, Default)]
pub struct Stream {
	data: Vec<u8>,
}

impl Stream {
	/// Use the `EncodeMut` trait to encode an object into the stream.
	pub fn encode_mut<T: EncodeMut>(&mut self, object: &mut T) {
		object.encode_mut(&mut self.data);
	}

	/// Use the `Encode` trait to encode an object into the stream.
	pub fn encode<T: Encode>(&mut self, object: &T) {
		object.encode(&mut self.data);
	}

	/// Writes the stream to a file.
	pub fn to_file(&self, file_name: &str) {
		std::fs::write(file_name, &self.data).expect("Could not write stream");
	}

	/// Returns the stream's buffer.
	pub fn get_buffer(&self) -> &Vec<u8> {
		&self.data
	}

	/// Returns the stream's buffer.
	pub fn get_buffer_mut(&mut self) -> &mut Vec<u8> {
		&mut self.data
	}
}

/// Encode an object into a byte vector. Object can mutate itself.
pub trait EncodeMut {
	fn encode_mut(&mut self, vector: &mut Vec<u8>);
}

/// Encode an object into a byte vector.
pub trait Encode {
	fn encode(&self, vector: &mut Vec<u8>);
}

/// Decode an object from a byte slice. Returns the deserialized object along with how many bytes were read.
pub trait Decode: Sized {
	fn decode(vector: &[u8]) -> (Self, &[u8]);
}
