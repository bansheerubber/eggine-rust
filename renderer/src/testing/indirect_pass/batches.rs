use std::collections::HashMap;
use std::sync::{ Arc, RwLock, };

use crate::testing::indirect_pass::IndirectPass;
use crate::memory_subsystem::{ Memory, textures, };
use crate::memory_subsystem::textures::Pager;
use crate::shapes;

use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher, };

#[derive(Debug)]
pub(crate) struct Batch<'a> {
	pub batch_parameters: Vec<&'a shapes::BatchParameters>,
	pub bone_index: usize,
	pub draw_call_count: usize,
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


impl IndirectPass<'_> {
	// This function has its own file because it otherwise makes the `IndirectPass::encode` function unbearable to read.

	/// Iterates through the `IndirectPass`'s batch parameters and consolidates them into individual batches based on GPU
	/// parameters. Batches are generated based on the GPU's:
	/// 1. texture page size
	/// 2. other stuff that will be introduced in the future maybe
	pub(crate) fn generate_batches<'a>(
		memory: &Arc<RwLock<Memory>>,
		batching_parameters: &'a HashMap<shapes::BatchParametersKey, shapes::BatchParameters>,
	) -> Vec<Batch<'a>> {
		let memory = memory.read().unwrap();
		let texture_size = memory.get_texture_descriptor().size.width;

		let mut sorted_parameters = batching_parameters.values()
			.collect::<Vec<&shapes::BatchParameters>>();
		sorted_parameters.sort();

		let mut current_batch = Batch {
			batch_parameters: Vec::new(),
			bone_index: 0,
			draw_call_count: 0,
			meshes_to_draw: 0,
			texture_pager: textures::VirtualPager::new(20, memory.get_texture_descriptor().size.width as u16),
		};

		// go through the batch parameters and compute the individual batches we need to draw
		let mut batches = Vec::new();
		for parameters in sorted_parameters {
			let allocated_cell = current_batch.texture_pager.allocate_texture(parameters.get_texture());

			if allocated_cell.is_none() {
				batches.push(current_batch);

				current_batch = Batch {
					batch_parameters: Vec::new(),
					bone_index: 0,
					draw_call_count: 0,
					meshes_to_draw: 0,
					texture_pager: textures::VirtualPager::new(20, texture_size as u16),
				};

				current_batch.texture_pager.allocate_texture(parameters.get_texture()).unwrap();
			} else {
				current_batch.batch_parameters.push(parameters);
				current_batch.meshes_to_draw += parameters.get_shapes()
					.fold(0, |acc, shape| acc + shape.borrow().get_blueprint().get_meshes().len());
			}
		}

		if current_batch.batch_parameters.len() != 0 {
			batches.push(current_batch);
		}

		batches
	}
}
