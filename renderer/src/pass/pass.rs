use crate::shaders::ShaderTable;
use crate::state::State;

/// Produces a ready-to-render framebuffer that is composited with other `Pass`s by the `Boss`. A `Pass` controls its
/// own scene state. A `Pass` that implements a city renderer will store references to all buildings, roads, props, etc.
/// It will manage the mesh buffers and decide how to use them in renderering. The `Boss` has control of some render
/// state (like render pipeline creation), so a `Pass` must communicate with the `Boss` to acquire the necessary state
/// to create a `wgpu::RenderPass`.
pub trait Pass {
	/// Called by the `Boss` so it can prepare any needed pipelines for `encode`.
	fn states<'a>(&self, shader_table: &'a ShaderTable) -> Vec<State<'a>>;

	/// Encodes draw calls into the specified encoder.
	fn encode(
		&self, encoder: &mut wgpu::CommandEncoder, pipelines: &Vec<&wgpu::RenderPipeline>, view: &wgpu::TextureView
	);
}

impl std::fmt::Debug for dyn Pass + 'static {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{:?}", self))
	}
}
