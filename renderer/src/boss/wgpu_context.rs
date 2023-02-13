/// Immutable context that describes all the `wgpu` related objects we created while initializing the renderer.
#[derive(Debug)]
pub struct WGPUContext {
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub instance: wgpu::Instance,
	pub queue: wgpu::Queue,
	pub surface: wgpu::Surface,
	pub swapchain_capabilities: wgpu::SurfaceCapabilities,
	pub swapchain_format: wgpu::TextureFormat,
	pub window: winit::window::Window,
}
