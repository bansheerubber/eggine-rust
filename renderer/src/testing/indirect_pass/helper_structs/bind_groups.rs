/// Stores bind groups for shaders. Bind groups have a dependency on `RenderState`/`ComputeState` creation.
#[derive(Debug)]
pub(crate) struct BindGroups {
	pub(crate) composite_bind_group: wgpu::BindGroup,
	pub(crate) depth_pyramid_bind_groups: Vec<wgpu::BindGroup>,
	pub(crate) texture_bind_group: wgpu::BindGroup,
	pub(crate) uniform_bind_group: wgpu::BindGroup,
}
