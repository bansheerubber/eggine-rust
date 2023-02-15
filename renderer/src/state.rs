use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher, };

use super::shaders::Program;

/// The render state stores intermediate attributes that describe a render pipeline. The `Renderer` will take the
/// intermediate data structures and translate them into the appropriate `wgpu` render pipeline. Render pipelines are
/// cached by the `Renderer`, and since some intermediate attributes cannot be cloned and used as keys in the cache
/// `HashMap`, `State` implements its own key generator.
#[derive(Clone, PartialEq)]
pub struct State<'a> {
	/// Describes the depth stencil used in the pipeline.
	pub depth_stencil: Option<wgpu::DepthStencilState>,
	/// Program to be used in the pipeline.
	pub program: &'a Program,
}

impl State<'_> {
	/// Generate a key to be used in `HashMap`s
	pub fn key(&self) -> StateKey {
		let mut state = DefaultHasher::new();
		self.depth_stencil.hash(&mut state);

		StateKey {
			depth_stencil: state.finish(),
			program: self.program.get_name().to_string(),
		}
	}
}

/// State key used for hash maps.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StateKey {
	depth_stencil: u64,
	program: String,
}
