use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;
use std::slice::Iter;

use crate::memory_subsystem::textures;
use crate::shapes;

/// The parameters that are used in the batching algorithm to generate the smallest number of shape batches.
#[derive(Debug)]
pub(crate) struct BatchParameters {
	shapes: Vec<Rc<RefCell<shapes::Shape>>>,
	texture: Rc<textures::Texture>,
}

impl Hash for BatchParameters {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.texture.hash(state);
	}
}

impl Eq for BatchParameters {}

impl PartialEq for BatchParameters {
	fn eq(&self, other: &Self) -> bool {
		self.texture == other.texture
	}
}

impl Ord for BatchParameters {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.texture.get_size().cmp(&other.texture.get_size())
	}
}

impl PartialOrd for BatchParameters {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.texture.get_size().partial_cmp(&other.texture.get_size())
	}
}

impl BatchParameters {
	pub fn new(texture: Rc<textures::Texture>) -> Self {
		BatchParameters {
			shapes: Vec::new(),
			texture,
		}
	}

	pub fn add_shape(&mut self, shape: Rc<RefCell<shapes::Shape>>) {
		self.shapes.push(shape);
	}

	pub fn get_shapes(&self) -> Iter<'_, Rc<RefCell<shapes::Shape>>> {
		self.shapes.iter()
	}

	pub fn get_texture(&self) -> &Rc<textures::Texture> {
		&self.texture
	}

	pub fn make_key(&self) -> BatchParametersKey {
		BatchParametersKey {
			texture: self.texture.clone(),
		}
	}
}

/// Cut down version of `BatchParameters` for looking up `BatchParameter` objects.
#[derive(Debug)]
pub(crate) struct BatchParametersKey {
	pub texture: Rc<textures::Texture>,
}

impl Hash for BatchParametersKey {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.texture.hash(state);
	}
}

impl Eq for BatchParametersKey {}

impl PartialEq for BatchParametersKey {
	fn eq(&self, other: &Self) -> bool {
		self.texture == other.texture
	}
}
