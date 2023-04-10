use std::collections::HashMap;
use std::sync::{ Arc, RwLock, };

use crate::testing::{ Batch, IndirectPass, };
use crate::shapes;
use crate::memory_subsystem::{ Memory, textures, };
use crate::memory_subsystem::textures::Pager;

impl IndirectPass<'_> {
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
