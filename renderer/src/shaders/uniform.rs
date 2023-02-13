/// Stores data about a uniform, used for generating descriptor sets.
#[derive(Debug)]
pub struct Uniform {
	/// The binding of the uniform as found in the shader source.
	pub binding: u32,
	/// The shader source type of the uniform (a texture, vector, matrix, etc).
	pub kind: String,
	/// The name of the uniform as found in the shader source.
	pub name: String,
	/// The set of the uniform as found in the shader source.
	pub set: u32,
}
