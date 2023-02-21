/// Controls how a `Texture` allocates memory.
pub trait State {
	fn prepare_new_texture(&mut self);

	fn reserve_texture(&mut self) -> u32;

	fn write_texture(&mut self, layer: u32, data: Vec<u8>);
}
