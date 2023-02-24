use std::hash::Hash;

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
	height: u16,
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
	/// Create a new texture.
	pub fn new(file_name: &str, data: TextureData, size: (u16, u16)) -> Self {
		Texture {
			data,
			file_name: file_name.to_string(),
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
}