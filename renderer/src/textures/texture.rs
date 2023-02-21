use std::rc::Rc;

use carton::Carton;

use super::{ Error, State, };

#[derive(Debug)]
pub struct Texture {
	texture: wgpu::Texture,
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

		// translate QOI format to image format
		let format = if header.colorspace == qoi::ColorSpace::Srgb {
			wgpu::TextureFormat::Rgba8UnormSrgb
		} else {
			wgpu::TextureFormat::Rgba8Unorm
		};

		let divisor = if header.channels == qoi::Channels::Rgb {
			3
		} else {
			4
		};

		let mut data = Vec::new();
		for i in 0..raw_data.len() / divisor {
			let r = raw_data[i * divisor];
			let g = raw_data[i * divisor + 1];
			let b = raw_data[i * divisor + 2];
			let a = if header.channels == qoi::Channels::Rgb {
				255
			} else {
				raw_data[i * divisor + 3]
			};

			data.push(r);
			data.push(g);
			data.push(b);
			data.push(a);

			// pad with zeros
			if (data.len() / 4) % header.width as usize == 0 && i != 0 {
				let padding = 256 - data.len() % 256; // the amount of bytes we have to pad
				for _ in 0..padding {
					data.push(0);
				}
			}
		}

		let descriptor = wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format,
			label: None,
			mip_level_count: 1,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 1,
				height: header.height,
				width: header.width,
			},
			view_formats: &[],
		};

		let texture = state.create_texture(&descriptor);
		state.write_texture(&texture, &descriptor, data);

		Ok(Rc::new(Texture {
			texture,
		}))
	}
}