use std::rc::Rc;

use crate::memory_subsystem::textures;

/// Represents a material assigned to a mesh primitive.
#[derive(Debug)]
pub struct Material {
	/// Material roughness.
	pub roughness: f32,
	/// The texture used for the material. Textures can be re-used across materials.
	pub texture: Option<Rc<textures::Texture>>,
}
