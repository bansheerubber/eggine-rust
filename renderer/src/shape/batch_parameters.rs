use std::collections::HashSet;
use std::rc::Rc;

use crate::{ shape, textures, };

/// The parameters that are used in the batching algorithm to generate the smallest number of shape batches.
#[derive(Debug)]
pub(crate) struct BatchParameters {
	shapes: HashSet<Rc<shape::Shape>>,
	texture: Rc<textures::Texture>,
}

impl BatchParameters {
	pub fn add_shape(&mut self, shape: Rc<shape::Shape>) {
		self.shapes.insert(shape);
	}
}
