use super::shaders::Shader;

/// The render state stores intermediate attributes that describe a render pipeline. The `Renderer` will take the
/// intermediate data structures and translate them into the appropriate `wgpu` render pipeline. Render pipelines are
/// cached by the `Renderer`, and since some intermediate attributes cannot be cloned and used as keys in the cache
/// `HashMap`, `State` implements its own key generator.
#[derive(Clone, PartialEq)]
pub struct State<'a> {
	/// Fragment shader to be used in the pipeline.
	pub fragment_shader: &'a Shader,
	/// Vertex shader to be used in the pipeline.
	pub vertex_shader: &'a Shader,
}

impl State<'_> {
	/// Generate a key to be used in `HashMap`s
	pub fn key(&self) -> StateKey {
		StateKey {
			fragment_shader: self.fragment_shader.file_name.to_string(),
			vertex_shader: self.vertex_shader.file_name.to_string(),
		}
	}
}

/// State key used for hash maps.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StateKey {
	fragment_shader: String,
	vertex_shader: String,
}
