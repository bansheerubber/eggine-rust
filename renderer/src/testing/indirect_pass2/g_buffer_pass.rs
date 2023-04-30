use std::sync::{ Arc, RwLock, };

use crate::memory_subsystem::Memory;
use crate::testing::indirect_pass::{ AllocatedMemory, BindGroups, Programs, RenderTextures, };
use crate::testing::{ Batch, IndirectPass, };

impl IndirectPass<'_> {
	// This function has its own file because it otherwise makes the `IndirectPass::encode` function unbearable to read.

	/// This performs the G-buffer render pass.
	///
	/// TODO rewrite this so that it only takes one batch instead of the entire batches vector, so batches work properly
	pub(crate) fn g_buffer_pass(
		memory: &Arc<RwLock<Memory>>,
		allocated_memory: &mut AllocatedMemory,
		programs: &mut Programs,
		bind_groups: &BindGroups,
		render_textures: &RenderTextures,
		indices_page_written: u64,
		vertices_page_written: u64,
		batches: &Vec<Batch>,
		encoder: &mut wgpu::CommandEncoder,
		pipelines: &Vec<&wgpu::RenderPipeline>,
	) {
		let memory = memory.read().unwrap();

		let mut g_buffer_load_op = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);

		for batch in batches.iter() {
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

			// do the render pass
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					color_attachments: &[
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &render_textures.diffuse_view,
						}),
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &render_textures.normal_view,
						}),
						Some(wgpu::RenderPassColorAttachment {
							ops: wgpu::Operations {
								load: g_buffer_load_op,
								store: true,
							},
							resolve_target: None,
							view: &render_textures.specular_view,
						}),
					],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						depth_ops: Some(wgpu::Operations {
							load: wgpu::LoadOp::Load,
							store: false,
						}),
						stencil_ops: None,
						view: &render_textures.depth_view,
					}),
					label: Some("g-buffer-pass"),
				});

				render_pass.set_pipeline(pipelines[0]);

				// set vertex attributes
				render_pass.set_index_buffer(
					memory.get_page(allocated_memory.indices_page).unwrap().get_buffer().slice(0..indices_page_written),
					wgpu::IndexFormat::Uint32
				);

				render_pass.set_vertex_buffer(
					0, memory.get_page(allocated_memory.positions_page).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					1, memory.get_page(allocated_memory.normals_page).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					2, memory.get_page(allocated_memory.uvs_page).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					3, memory.get_page(allocated_memory.bone_weights).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				render_pass.set_vertex_buffer(
					4, memory.get_page(allocated_memory.bone_indices).unwrap().get_buffer().slice(0..vertices_page_written)
				);

				// bind uniforms
				render_pass.set_bind_group(0, &bind_groups.uniform_bind_group, &[]);
				render_pass.set_bind_group(1, &bind_groups.texture_bind_group, &[]);

				// draw all the objects
				render_pass.multi_draw_indexed_indirect(
					memory.get_page(allocated_memory.indirect_command_buffer).unwrap().get_buffer(), 0, batch.draw_call_count as u32
				);

				// set clear ops
				g_buffer_load_op = wgpu::LoadOp::Load;
			}
		}
	}
}
