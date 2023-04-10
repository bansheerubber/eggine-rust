use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::memory_subsystem::textures::Pager;
use crate::memory_subsystem::{ Memory, textures, };
use crate::shapes;
use crate::testing::indirect_pass::{ AllocatedMemory, Programs, };
use crate::testing::uniforms::ObjectUniform;
use crate::testing::{ Batch, IndirectPass, };

impl IndirectPass<'_> {
	// This function has its own file because it otherwise makes the `IndirectPass::encode` function unbearable to read.

	/// Iterate through the shapes in the provided batches and populate GPU buffers. The following buffers are filled:
	/// 1. indirect command buffer
	/// 2. uniform buffer
	/// 3. bone uniform buffer
	/// 4. texture buffers
	///
	/// TODO rewrite this so that it only takes one batch instead of the entire batches vector, so batches work properly
	pub(crate) fn buffer_generation(
		memory: &Arc<RwLock<Memory>>,
		allocated_memory: &mut AllocatedMemory,
		programs: &mut Programs,
		batches: &mut Vec<Batch>,
		deltatime: f64,
	) {
		let mut memory = memory.write().unwrap();
		let texture_size = memory.get_texture_descriptor().size.width;

		for batch in batches.iter_mut() {
			static DRAW_INDEXED_DIRECT_SIZE: usize = std::mem::size_of::<wgpu::util::DrawIndexedIndirect>();

			// get bone uniforms map for this batch
			if !programs.bone_uniforms.contains_key(&batch.make_key()) {
				programs.bone_uniforms.insert(batch.make_key(), Vec::new());
			}

			let bone_uniforms = programs.bone_uniforms.get_mut(&batch.make_key()).unwrap();

			// get object uniforms for this batch
			if !programs.object_uniforms.contains_key(&batch.make_key()) {
				programs.object_uniforms.insert(batch.make_key(), Vec::new());
			}

			let object_uniforms = programs.object_uniforms.get_mut(&batch.make_key()).unwrap();

			// get indirect command buffer for this batch
			allocated_memory.indirect_command_buffer_map.insert(
				batch.make_key(), vec![0; batch.meshes_to_draw * DRAW_INDEXED_DIRECT_SIZE]
			);

			let buffer = allocated_memory.indirect_command_buffer_map.get_mut(&batch.make_key()).unwrap();

			// upload textures
			let textures = batch.batch_parameters.iter()
				.map(|x| x.get_texture())
				.collect::<Vec<&Rc<textures::Texture>>>();

			// only upload if the textures aren't uploaded yet
			if !memory.is_same_pager(&batch.texture_pager) {
				memory.reset_pager();

				for texture in textures.iter() {
					memory.upload_texture(texture);
				}
			}

			let shapes = batch.batch_parameters.iter()
				.flat_map(|x| x.get_shapes())
				.collect::<Vec<&Rc<RefCell<shapes::Shape>>>>();

			// iterate through the shapes in the batch and draw them
			for shape in shapes {
				let mut shape = shape.borrow_mut();

				for (node, mesh) in shape.get_blueprint().get_mesh_nodes().iter() { // TODO lets maybe not do a three level nested for loop
					for primitive in mesh.primitives.iter() {
						let texture = &primitive.material.texture;
						if !textures.contains(&texture) { // TODO optimize this whole damn texture thing
							continue;
						}

						if batch.draw_call_count as u64 >= allocated_memory.max_objects_per_batch {
							panic!("Exceeded maximum amount of objects per batch")
						}

						// write draw index indirect into the buffer as fast as possible
						unsafe {
							let command = wgpu::util::DrawIndexedIndirect {
								base_index: primitive.first_index,
								base_instance: 0,
								instance_count: 1,
								vertex_count: primitive.vertex_count,
								vertex_offset: primitive.vertex_offset,
							};

							buffer[
								batch.draw_call_count * DRAW_INDEXED_DIRECT_SIZE..(batch.draw_call_count + 1) * DRAW_INDEXED_DIRECT_SIZE
							].copy_from_slice(
								std::slice::from_raw_parts(
									&command as *const _ as *const u8,
									DRAW_INDEXED_DIRECT_SIZE
								)
							)
						}

						// allocate `bone_uniforms`
						if batch.bone_index + mesh.bones.len() > bone_uniforms.len() {
							bone_uniforms.resize(
								bone_uniforms.len() + mesh.bones.len(),
								glam::Mat4::IDENTITY
							);
						}

						let inverse_transform = node.borrow().transform.inverse();

						// the bone offset is the start of where we're going to write bone uniforms
						let bone_offset = batch.bone_index as u32;

						// set bone uniforms
						for (bone, inverse_bind_matrix) in mesh.bones.iter() { // TODO move matrix multiplications to compute shader?
							bone_uniforms[batch.bone_index] = shape.get_bone_matrix(bone, inverse_bind_matrix, &inverse_transform);
							batch.bone_index += 1;
						}

						// allocate `object_uniforms`
						if batch.draw_call_count >= object_uniforms.len() {
							object_uniforms.push(ObjectUniform::default());
						}

						// put the object uniforms into the array
						let model_matrix = node.borrow().transform.mul_mat4(shape.get_transformation());
						let texture = memory.texture_pager.get_cell(&texture).unwrap();
						object_uniforms[batch.draw_call_count] = ObjectUniform {
							model_matrix: model_matrix.to_cols_array(),
							texture_offset: glam::Vec4::new(
								texture.get_position().x as f32 / texture_size as f32,
								texture.get_position().y as f32 / texture_size as f32,
								texture.get_size() as f32 / texture_size as f32,
								texture.get_size() as f32 / texture_size as f32
							).to_array(),
							roughness: primitive.material.roughness,
							bone_offset,
							_padding: [0.0, 0.0],
						};

						batch.draw_call_count += 1;
					}
				}

				shape.update_animation_timer(deltatime as f32);
			}
		}
	}
}
