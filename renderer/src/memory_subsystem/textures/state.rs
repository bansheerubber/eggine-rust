use std::rc::Rc;

use super::Texture;

/// Controls how a `Texture` allocates memory.
pub trait State {
	fn prepare_new_texture(&mut self);

	fn write_texture(&mut self, texture: Rc<Texture>);
}
