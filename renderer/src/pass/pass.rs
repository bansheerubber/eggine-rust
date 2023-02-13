use crate::Boss;

/// Produces a ready-to-render framebuffer that is composited with other `Pass`s by the `Boss`. A `Pass` controls its
/// own scene state. A `Pass` that implements a city renderer will store references to all buildings, roads, props, etc.
/// It will manage the mesh buffers and decide how to use them in renderering. The `Boss` has control of some render
/// state (like render pipeline creation), so a `Pass` must communicate with the `Boss` to acquire the necessary state
/// to create a `wgpu::RenderPass`.
pub trait Pass {
	fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, boss: &mut Boss);
}