use crate::state::{ ComputeState, RenderState, };

/// Produces a ready-to-render framebuffer that is composited with other `Pass`s by the `Boss`. A `Pass` controls its
/// own scene state. A `Pass` that implements a city renderer will store references to all buildings, roads, props, etc.
/// It will manage the mesh buffers and decide how to use them in renderering. The `Boss` has control of some render
/// state (like render pipeline creation), so a `Pass` must communicate with the `Boss` to acquire the necessary state
/// to create a `wgpu::RenderPass`.
pub trait Pass {
	/// Called by the `Boss` so it can prepare any needed render pipelines for `encode`.
	fn render_states<'a>(&'a self) -> Vec<RenderState<'a>>;

	/// Called by the `Boss` so it can prepare any needed compute pipelines for `encode`.
	fn compute_states<'a>(&'a self) -> Vec<ComputeState<'a>>;

	/// Encodes draw calls into the specified encoder.
	fn encode(
		&mut self,
		deltatime: f64,
		encoder: &mut wgpu::CommandEncoder,
		render_pipelines: &Vec<&wgpu::RenderPipeline>,
		compute_pipelines: &Vec<&wgpu::ComputePipeline>,
		view: &wgpu::TextureView
	);

	/// Callback for when the `Boss` is resized.
	fn resize(&mut self, config: &wgpu::SurfaceConfiguration);
}

impl std::fmt::Debug for dyn Pass + 'static {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{:?}", self))
	}
}
