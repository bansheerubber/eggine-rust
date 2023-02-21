use std::collections::HashSet;
use std::collections::hash_set::Iter;
use std::rc::Rc;

use crate::{ shape, textures, };

/// The parameters that are used in the batching algorithm to generate the smallest number of shape batches.
#[derive(Debug)]
pub(crate) struct BatchParameters {
	shapes: HashSet<Rc<shape::Shape>>,
	texture: Rc<textures::Texture>,
}

impl PartialEq for BatchParameters {
	fn eq(&self, other: &Self) -> bool {
		self.texture == other.texture
	}
}

impl PartialOrd for BatchParameters {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.texture.get_size().partial_cmp(&other.texture.get_size())
	}
}

impl BatchParameters {
	pub fn add_shape(&mut self, shape: Rc<shape::Shape>) {
		self.shapes.insert(shape);
	}

	pub fn get_shapes(&self) -> Iter<'_, Rc<shape::Shape>> {
		self.shapes.iter()
	}
}
