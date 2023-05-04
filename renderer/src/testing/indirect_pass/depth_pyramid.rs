use std::cell::RefCell;
use std::rc::Rc;

use crate::testing::indirect_pass::IndirectPass;

#[derive(Debug)]
pub struct DepthPyramidTexture<'a> {
	pub descriptor: wgpu::TextureDescriptor<'a>,
	pub height: u32,
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub width: u32,
}

impl<'a> IndirectPass<'a> {
	pub fn create_depth_pyramid(&mut self) -> Vec<wgpu::BindGroup> {
		let mut depth_pyramid = self.allocated_memory.depth_pyramid.borrow_mut();
		depth_pyramid.clear();
		let mut bind_groups = Vec::new();

		let depth_pyramid_sampler = self.context.device.create_sampler(&wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::ClampToEdge,
			address_mode_v: wgpu::AddressMode::ClampToEdge,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			label: Some("depth-pyramid-sampler"),
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::FilterMode::Nearest,
			reduction_mode: Some(wgpu::ReductionMode::Min),
			..Default::default()
		});

		let mut current_size = (self.render_textures.window_width >> 1, self.render_textures.window_height >> 1);
		let mut use_depth_buffer = true;

		loop {
			// create depth pyramid texture
			let descriptor = wgpu::TextureDescriptor {
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::R32Float,
				label: None,
				mip_level_count: 1,
				sample_count: 1,
				size: wgpu::Extent3d {
					depth_or_array_layers: 1,
					height: current_size.1,
					width: current_size.0,
				},
				usage: wgpu::TextureUsages::TEXTURE_BINDING
					| wgpu::TextureUsages::COPY_DST
					| wgpu::TextureUsages::STORAGE_BINDING,
				view_formats: &[],
			};

			let texture = self.context.device.create_texture(&descriptor);
			let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

			let texture_view = if use_depth_buffer {
				&self.render_textures.depth_view
			} else {
				let index = depth_pyramid.len() - 1;
				&depth_pyramid[index].view
			};

			use_depth_buffer = false;

			// create the bind groups
			let depth_pyramid_bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(texture_view), // TODO texture views for bind groups????
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&depth_pyramid_sampler),
					},
					wgpu::BindGroupEntry {
						binding: 2,
						resource: wgpu::BindingResource::TextureView(&view),
					},
				],
				label: Some("depth-pyramid-bind-group"),
				layout: &self.programs.depth_pyramid_bind_group_layout,
			});

			bind_groups.push(depth_pyramid_bind_group);

			depth_pyramid.push(DepthPyramidTexture {
				descriptor,
				height: current_size.1,
				texture,
				view,
				width: current_size.0,
			});

			if current_size.0 == 1 && current_size.1 == 1 {
				break;
			}

			current_size = (u32::max(current_size.0 >> 1, 1), u32::max(current_size.1 >> 1, 1));
		}

		return bind_groups;
	}

	pub fn get_depth_pyramid(&self) -> Rc<RefCell<Vec<DepthPyramidTexture<'a>>>> {
		self.allocated_memory.depth_pyramid.clone()
	}
}
