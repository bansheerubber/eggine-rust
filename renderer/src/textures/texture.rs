use std::hash::Hash;
use std::rc::Rc;

use carton::Carton;

use super::{ Error, State, };

#[derive(Debug)]
pub struct Texture {
	file_name: String,
	height: u16,
	layer: u32,
	width: u16,
}

impl Hash for Texture {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.file_name.hash(state);
	}
}

impl PartialEq for Texture {
	fn eq(&self, other: &Self) -> bool {
		self.file_name == other.file_name
	}
}

impl Texture {
	/// Load a QOI file from a carton.
	pub fn load<T: State>(
		file_name: &str, carton: &mut Carton, state: &mut Box<T>
	) -> Result<Rc<Texture>, Error> {
		// load the FBX up from the carton
		let qoi_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(Error::CartonError(error)),
			Ok(qoi_stream) => qoi_stream,
		};

		let mut decoder = qoi::Decoder::from_stream(qoi_stream).unwrap();

		let raw_data = decoder.decode_to_vec().unwrap();
		let header = decoder.header();

		let divisor = if header.channels == qoi::Channels::Rgb {
			3
		} else {
			4
		};

		let mut data = Vec::new();
		for y in (0..header.height).rev() { // reverse the image on the y-axis
			for x in 0..header.width {
				let index = (header.height * y + x) as usize;

				let r = raw_data[index * divisor];
				let g = raw_data[index * divisor + 1];
				let b = raw_data[index * divisor + 2];
				let a = if header.channels == qoi::Channels::Rgb {
					255
				} else {
					raw_data[index * divisor + 3]
				};

				data.push(r);
				data.push(g);
				data.push(b);
				data.push(a);
			}
		}

		let layer = state.reserve_texture();
		state.write_texture(layer, data);

		Ok(Rc::new(Texture {
			file_name: file_name.to_string(),
			height: header.height as u16,
			layer,
			width: header.width as u16,
		}))
	}

	pub fn get_size(&self) -> (u16, u16) {
		(self.width, self.height)
	}
}