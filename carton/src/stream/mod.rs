pub mod writing;

#[derive(Debug, Default)]
pub struct Stream {
	data: Vec<u8>,
}

impl Stream {
	pub fn encode_mut<T: EncodeMut>(&mut self, object: &mut T) {
		object.encode_mut(&mut self.data);
	}

	pub fn encode<T: Encode>(&mut self, object: &T) {
		object.encode(&mut self.data);
	}

	pub fn to_file(&self, file_name: &str) {
		std::fs::write(file_name, &self.data).expect("Could not write stream");
	}

	pub fn get_buffer(&self) -> &Vec<u8> {
		&self.data
	}

	pub fn get_buffer_mut(&mut self) -> &mut Vec<u8> {
		&mut self.data
	}
}

pub trait EncodeMut {
	fn encode_mut(&mut self, vector: &mut Vec<u8>);
}

pub trait Encode {
	fn encode(&self, vector: &mut Vec<u8>);
}

pub trait Decode {

}