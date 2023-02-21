/// Controls how a `Texture` allocates memory.
pub trait State {
	fn prepare_new_texture(&mut self);

	fn create_texture(&mut self, descriptor: &wgpu::TextureDescriptor) -> wgpu::Texture;

	fn write_texture(&mut self, texture: &wgpu::Texture, descriptor: &wgpu::TextureDescriptor, data: Vec<u8>);
}
