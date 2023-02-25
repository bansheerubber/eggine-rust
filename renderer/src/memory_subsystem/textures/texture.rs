use lazy_static::lazy_static;
use std::hash::Hash;
use std::sync::Mutex;

lazy_static! {
	static ref NEXT_TEXTURE_GUID: Mutex<u64> = Mutex::new(0);
}

/// Describes the byte format of a texture.
#[derive(Debug)]
pub enum TextureData {
	Astc(Vec<u8>, wgpu::AstcBlock),
	Raw(Vec<u8>),
}

/// Describes attributes of a texture that was loaded from the carton. Textures are tightly coupled with the memory
/// subsystem, but require a representation that is external of the texture quad tree manager so textures can be backed
/// by CPU memory.
///
/// The texture struct doesn't really do anything on its own. The owner of it (the texture pager) will manipulate it
/// as needed.
#[derive(Debug)]
pub struct Texture {
	data: TextureData,
	file_name: String,
	id: u64,
	height: u16,
	width: u16,
}

impl Hash for Texture {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl PartialEq for Texture {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl Texture {
	/// Create a new texture.
	pub fn new(file_name: &str, data: TextureData, size: (u16, u16)) -> Self {
		let mut next_texture_id = NEXT_TEXTURE_GUID.lock().unwrap();

		let id = *next_texture_id;
		*next_texture_id += 1;

		Texture {
			data,
			file_name: file_name.to_string(),
			id,
			height: size.1,
			width: size.0,
		}
	}

	/// Gets the width and height of the texture.
	pub fn get_size(&self) -> (u16, u16) {
		(self.width, self.height)
	}

	/// Gets the texture's file name.
	pub fn get_file_name(&self) -> &str {
		&self.file_name
	}

	/// Gets the pixel data loaded from the carton.
	pub fn get_data(&self) -> &TextureData {
		&self.data
	}

	/// Gets the unique texture ID.
	pub fn get_id(&self) -> u64 {
		self.id
	}
}