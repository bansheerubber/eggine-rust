/// Stores the render targets used by the pass object, recreated whenever the swapchain is out of date.
#[derive(Debug)]
pub(crate) struct RenderTextures {
	pub(crate) _depth_texture: wgpu::Texture,
	pub(crate) depth_view: wgpu::TextureView,
	pub(crate) diffuse_format: wgpu::TextureFormat,
	pub(crate) diffuse_view: wgpu::TextureView,
	pub(crate) normal_format: wgpu::TextureFormat,
	pub(crate) normal_view: wgpu::TextureView,
	pub(crate) specular_format: wgpu::TextureFormat,
	pub(crate) specular_view: wgpu::TextureView,
	pub(crate) window_height: u32,
	pub(crate) window_width: u32,
}
