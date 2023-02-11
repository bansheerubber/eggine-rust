use super::shaders::Shader;

/// The render state stores attributes that describe a render pipeline. The `Renderer` will take the intermediate
/// data structures and translate them into the appropriate `wgpu` render pipeline. Render pipelines are cached by the
/// `Renderer`, and the render state must be hashable to assist caching.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct State<'a> {
	pub fragment_shader: &'a Shader,
	pub vertex_shader: &'a Shader,
}

impl State<'_> {
	pub fn key(&self) -> StateKey {
		StateKey {
			fragment_shader: self.fragment_shader.file_name.to_string(),
			vertex_shader: self.vertex_shader.file_name.to_string(),
		}
	}
}

/// State key used for hash maps
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct StateKey {
	fragment_shader: String,
	vertex_shader: String,
}
