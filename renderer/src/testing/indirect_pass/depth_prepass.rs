use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::boss::WGPUContext;
use crate::memory_subsystem::Memory;
use crate::testing::indirect_pass::{ AllocatedMemory, Batch, BindGroups, IndirectPass, Programs, RenderTextures, };

impl IndirectPass<'_> {
	// This function has its own file because it otherwise makes the `IndirectPass::encode` function unbearable to read.

	/// This performs the depth prepass.
	pub(crate) fn depth_prepass(
		context: Rc<WGPUContext>,
		memory: &Arc<RwLock<Memory>>,
		allocated_memory: &mut AllocatedMemory,
		programs: &mut Programs,
		bind_groups: &BindGroups,
		render_textures: &RenderTextures,
		indices_page_written: u64,
		vertices_page_written: u64,
		batches: &Vec<Batch>,
		pipelines: &Vec<&wgpu::RenderPipeline>,
		mut last_rendered_batch: Option<u64>,
	) -> Option<u64> {
		let memory = memory.read().unwrap();

		let mut depth_buffer_load_op = wgpu::LoadOp::Clear(1.0);

		for batch in batches.iter() {
			let mut encoder = context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("depth-prepass-encoder"),
			});

			if last_rendered_batch.is_none() || batch.make_key() != last_rendered_batch.unwrap() {
				let bone_uniforms = programs.bone_uniforms.get_mut(&batch.make_key()).unwrap();
				let object_uniforms = programs.object_uniforms.get_mut(&batch.make_key()).unwrap();
				let buffer = allocated_memory.indirect_command_buffer_map.get_mut(&batch.make_key()).unwrap();

				// ensure immediate write to the buffer
				memory.get_page(allocated_memory.indirect_command_buffer)
					.unwrap()
					.write_buffer(&allocated_memory.indirect_command_buffer_node, &buffer);

				// write object uniforms to storage buffer
				memory.get_page(allocated_memory.object_storage_page)
					.unwrap()
					.write_slice(
						&allocated_memory.object_storage_node,
						bytemuck::cast_slice(&object_uniforms[0..batch.draw_call_count])
					);

				// write bone matrices to storage buffer
				memory.get_page(allocated_memory.bone_storage_page)
					.unwrap()
					.write_slice(
						&allocated_memory.bone_storage_node,
						bytemuck::cast_slice(&bone_uniforms[0..batch.bone_index])
					);
			}

			// do the render pass
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					color_attachments: &[],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						depth_ops: Some(wgpu::Operations {
							load: depth_buffer_load_op,
							store: true,
						}),
						stencil_ops: None,
						view: &render_textures.depth_view,
					}),
					label: Some("depth-prepass"),
				});

				render_pass.set_pipeline(pipelines[2]);

				// set vertex attributes
				render_pass.set_index_buffer(
					memory.get_page(allocated_memory.indices_page).unwrap().get_buffer().slice(0..indices_page_written),
					wgpu::IndexFormat::Uint32
				);

				render_pass.set_vertex_buffer(
					0, memory.get_page(allocated_memory.positions_page).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					1, memory.get_page(allocated_memory.bone_weights).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					2, memory.get_page(allocated_memory.bone_indices).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				// bind uniforms
				render_pass.set_bind_group(0, &bind_groups.uniform_bind_group, &[]);

				// draw all the objects
				render_pass.multi_draw_indexed_indirect(
					memory.get_page(allocated_memory.indirect_command_buffer).unwrap().get_buffer(), 0, batch.draw_call_count as u32
				);

				// set clear ops
				depth_buffer_load_op = wgpu::LoadOp::Load;
			}

			context.queue.submit(Some(encoder.finish()));

			last_rendered_batch = Some(batch.make_key());
		}

		return last_rendered_batch;
	}
}
