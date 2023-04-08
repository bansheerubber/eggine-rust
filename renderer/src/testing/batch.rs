use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher, };

use crate::memory_subsystem::textures;
use crate::shapes;

#[derive(Debug)]
pub(crate) struct Batch<'a> {
	pub batch_parameters: Vec<&'a shapes::BatchParameters>,
	pub meshes_to_draw: usize,
	pub texture_pager: textures::VirtualPager,
}

impl Batch<'_> {
	pub fn make_key(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		self.hash(&mut hasher);
		hasher.finish()
	}
}

impl Hash for Batch<'_> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		for parameters in self.batch_parameters.iter() {
			parameters.hash(state);
		}
	}
}

impl Eq for Batch<'_> {}

impl PartialEq for Batch<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.batch_parameters == other.batch_parameters
	}
}
