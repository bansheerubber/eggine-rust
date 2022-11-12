pub mod writing;

#[derive(Debug, Default)]
pub struct Stream {
	data: Vec<u8>,
}

impl Stream {
	pub fn encode<T: Encode>(&mut self, object: &T) {
		object.encode(&mut self.data);
	}

	pub fn to_file(&self, file_name: &str) {
		std::fs::write(file_name, &self.data).expect("Could not write stream");
	}
}

pub trait Encode {
	fn encode(&self, vector: &mut Vec<u8>);
}

pub trait Decode {

}