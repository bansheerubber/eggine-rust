use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher, };

use super::shaders::{ ComputeProgram, Program, };

/// The render state stores intermediate attributes that describe a render pipeline. The `Renderer` will take the
/// intermediate data structures and translate them into the appropriate `wgpu` render pipeline. Render pipelines are
/// cached by the `Boss`, and since some intermediate attributes cannot be cloned and used as keys in the cache
/// `HashMap`, `State` implements its own key generator.
#[derive(Clone)]
pub struct RenderState<'a> {
	/// Describes the depth stencil used in the pipeline.
	pub depth_stencil: Option<wgpu::DepthStencilState>,
	/// Render state's name.
	pub label: String,
	/// Optional layout parameter is useful for custom definitions of bind group layouts, and push constant ranges.
	pub layout: Option<&'a wgpu::PipelineLayout>,
	/// Program to be used in the pipeline.
	pub program: &'a Program,
	/// The textures the render state's pipeline will render to.
	pub render_targets: Vec<Option<wgpu::ColorTargetState>>,
	/// The vertex attributes required by the render state's pipeline.
	pub vertex_attributes: &'a [wgpu::VertexBufferLayout<'a>],
}

impl RenderState<'_> {
	/// Generate a key to be used in `HashMap`s
	pub fn key(&self) -> RenderStateKey {
		let mut state = DefaultHasher::new();
		self.depth_stencil.hash(&mut state);
		self.render_targets.hash(&mut state);
		self.vertex_attributes.hash(&mut state);

		RenderStateKey {
			program: self.program.get_name().to_string(),
			wgpu_hash: state.finish(),
		}
	}
}

/// State key used for hash maps. TODO probably just make this a u64 hash? idk
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RenderStateKey {
	program: String,
	wgpu_hash: u64,
}

/// Cachable state object used for creating compute pipelines. Since some intermediate attributes cannot be cloned and
/// used as keys in the `Boss`'s cache, `ComputeState` implements its own key generator.
pub struct ComputeState<'a> {
	/// Compute state's name.
	pub label: String,
	/// Optional layout parameter is useful for custom definitions of bind group layouts, and push constant ranges.
	pub layout: Option<&'a wgpu::PipelineLayout>,
	/// Program to be used in the pipeline.
	pub program: &'a ComputeProgram,
}

impl ComputeState<'_> {
	/// Generate a key to be used in `HashMap`s
	pub fn key(&self) -> ComputeStateKey {
		ComputeStateKey {
			program: self.program.get_name().to_string(),
		}
	}
}

/// State key used for hash maps. TODO probably just make this a u64 hash? idk
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ComputeStateKey {
	program: String,
}
