use std::collections::HashMap;
use std::num::NonZeroU64;
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::{ Node, Page, };
use super::page::PageUUID;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
#[derive(Debug)]
pub struct Memory<'a> {
	/// The `WGPUContext` that the memory manager uses to write to buffers.
	context: Rc<WGPUContext>,
	/// The UUID to use for the next allocated page.
	next_page_index: PageUUID,
	/// The layer to use for the next reserved texture.
	pub(crate) next_texture_layer: u32,
	/// The pages allocated by this memory manager.
	pages: HashMap<PageUUID, Page>,
	/// Data that the memory manager will write to buffers the next renderer tick.
	queued_writes: Vec<(Vec<u8>, PageUUID, Node)>,
	/// The staging belt used for uploading data to the GPU.
	staging_belt: Option<wgpu::util::StagingBelt>,
	/// The texture array stored on the GPU.
	texture: wgpu::Texture,
	/// The descriptor for the texture.
	texture_descriptor: wgpu::TextureDescriptor<'a>,
	/// The texture view for the memory's texture.
	texture_view: wgpu::TextureView,
}

impl<'a> Memory<'a> {
	/// Create a new memory manager that uses the supplied queue to write to buffers.
	pub fn new(context: Rc<WGPUContext>) -> Self {
		let texture_descriptor = wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			label: None,
			mip_level_count: 1,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: 20,
				height: 256,
				width: 256,
			},
			view_formats: &[],
		};
		let texture = context.device.create_texture(&texture_descriptor);

		Memory {
			texture_view: texture.create_view(&wgpu::TextureViewDescriptor::default()),

			context,
			next_page_index: 0,
			next_texture_layer: 0,
			pages: HashMap::new(),
			queued_writes: Vec::new(),
			staging_belt: Some(wgpu::util::StagingBelt::new(16_000_000)),
			texture,
			texture_descriptor,
		}
	}

	/// Creates a mew page, which allocates a `wgpu` buffer with the specified size.
	pub fn new_page(&mut self, size: u64, usage: wgpu::BufferUsages) -> PageUUID {
		let mut page = Page::new(size, usage, self.context.clone());
		page.set_uuid(self.next_page_index);
		self.pages.insert(self.next_page_index, page);
		self.next_page_index += 1;

		return self.next_page_index - 1;
	}

	/// Finds the page associated with the supplied UUID.
	pub fn get_page(&self, index: PageUUID) -> Option<&Page> {
		self.pages.get(&index)
	}

	/// Finds the page associated with the supplied UUID.
	pub fn get_page_mut(&mut self, index: PageUUID) -> Option<&mut Page> {
		self.pages.get_mut(&index)
	}

	/// Schedules a buffer write for the next frame.
	pub fn write_buffer(&mut self, page: PageUUID, node: &Node, data: Vec<u8>) {
		self.queued_writes.push((data, page, node.clone()));
	}

	/// Returns reference to the memory's texture.
	pub fn get_texture(&self) -> &wgpu::Texture {
		&self.texture
	}

	/// Returns reference to the memory's texture view.
	pub fn get_texture_view(&self) -> &wgpu::TextureView {
		&self.texture_view
	}

	/// Returns a reference to the memory's texture descriptor.
	pub fn get_texture_descriptor(&self) -> &'a wgpu::TextureDescriptor {
		&self.texture_descriptor
	}

	/// Invoked by the renderer at the start of every tick, and writes all queued data to buffers.
	pub(crate) fn complete_write_buffers(&mut self, encoder: &mut wgpu::CommandEncoder) {
		// steal the staging belt
		let mut staging_belt = std::mem::take(&mut self.staging_belt);

		for (data, page, node) in self.queued_writes.iter() {
			let page = self.get_page(*page).unwrap();

			let mut view = staging_belt.as_mut().unwrap().write_buffer(
				encoder,
				page.get_buffer(),
				node.offset,
				NonZeroU64::new(data.len() as u64).unwrap(),
				&self.context.device
			);

			view.copy_from_slice(&data);
		}

		self.staging_belt = staging_belt;

		self.staging_belt.as_mut().unwrap().finish();

		self.queued_writes.clear();
	}

	/// Invoked by the renderer after the `complete_write_buffers` commands has been submitted into the queue.
	pub(crate) fn recall(&mut self) {
		self.staging_belt.as_mut().unwrap().recall();
	}
}
