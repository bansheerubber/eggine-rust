use carton::Carton;
use std::collections::HashMap;
use std::rc::Rc;

use super::{ Error, Texture, TextureCell, TextureData, TextureRoot, };

/// Manages a quad-tree that describes the physical location of textures on the GPU. Maintain ownership of textures and
/// interacts with the memory subsystem to allocate/deallocate them as needed.
#[derive(Debug)]
pub struct Pager {
	/// Textures allocated on the GPU, as well as their `tree` vector index and the index of the cell within the tree.
	gpu_allocated_textures: HashMap<String, (usize, usize)>,
	/// The textures loaded from carton.
	textures: Vec<Rc<Texture>>,
	/// The physical locations of the textures on the GPU.
	tree: Vec<TextureRoot>,
}

impl Pager {
	/// Create a new texture pager.
	pub fn new(layer_count: usize, size: u16) -> Self {
		Pager {
			gpu_allocated_textures: HashMap::new(),
			textures: Vec::new(),
			tree: vec![TextureRoot::new(size); layer_count as usize],
		}
	}

	/// Load a QOI file from a carton.
	pub fn load_qoi(&mut self, file_name: &str, carton: &mut Carton) -> Result<Rc<Texture>, Error> {
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

		self.textures.push(Rc::new(
			Texture::new(file_name, TextureData::Raw(data), (header.width as u16, header.height as u16))
		));

		Ok(self.textures[self.textures.len() - 1].clone())
	}

	/// Wrapper for allocating a texture onto the quad tree. Returns the position within
	pub fn allocate_texture(&mut self, texture: &Rc<Texture>) -> Option<wgpu::Origin3d> {
		// figure out where to put the texture
		let mut cell_index = None;
		let mut layer = 0;
		for i in 0..self.tree.len() {
			cell_index = self.tree[i].allocate_texture(texture.clone());
			if cell_index.is_some() {
				layer = i;
				break;
			}
		}

		let Some(cell_index) = cell_index else {
			return None;
		};

		self.gpu_allocated_textures.insert(texture.get_file_name().to_string(), (layer, cell_index));

		let position = self.tree[layer].get_cell(cell_index).get_position();
		Some(wgpu::Origin3d {
			x: position.x as u32,
			y: position.y as u32,
			z: layer as u32,
		})
	}

	/// Returns whether or not a texture has been allocated in the GPU.
	pub fn is_gpu_allocated(&self, texture: &Rc<Texture>) -> bool {
		self.gpu_allocated_textures.contains_key(texture.get_file_name())
	}

	/// Returns the cell a texture belongs to.
	pub fn get_cell(&self, texture: &Rc<Texture>) -> Option<&TextureCell> {
		if !self.is_gpu_allocated(&texture) {
			return None;
		}

		let (layer, cell_index) = self.gpu_allocated_textures.get(texture.get_file_name()).unwrap();
		Some(self.tree[*layer].get_cell(*cell_index))
	}
}
