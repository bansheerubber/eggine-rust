use std::collections::HashMap;
use std::num::{ NonZeroU32, NonZeroU64,  };
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::{ Node, Page, textures, };
use super::page::PageUUID;
use super::textures::Pager;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
#[derive(Debug)]
pub struct Memory<'a> {
	/// The `WGPUContext` that the memory manager uses to write to buffers.
	context: Rc<WGPUContext>,
	/// The UUID to use for the next allocated page.
	next_page_index: PageUUID,
	/// Placeholder texture for when a mesh does not have a loaded texture.
	none_texture: Option<Rc<textures::Texture>>,
	/// The pages allocated by this memory manager.
	pages: HashMap<PageUUID, Page>,
	/// Data that the memory manager will write to buffers the next renderer tick.
	queued_writes: Vec<(Vec<u8>, PageUUID, Node)>,
	/// The staging belt used for uploading data to the GPU.
	staging_belt: Option<wgpu::util::StagingBelt>,
	/// The texture array stored on the GPU.
	texture_array: wgpu::Texture,
	/// The descriptor for the texture.
	texture_array_descriptor: wgpu::TextureDescriptor<'a>,
	/// The texture view for the memory's texture.
	texture_array_view: wgpu::TextureView,
	/// Controls the physical locations of the textures on the GPU.
	pub texture_pager: textures::GPUPager,
}

impl<'a> Memory<'a> {
	/// Create a new memory manager that uses the supplied queue to write to buffers.
	pub fn new(context: Rc<WGPUContext>) -> Self {
		// TODO dynamically figure this out from GPU configuration
		let layer_count = 20;
		let texture_size = 4096;

		let texture_descriptor = wgpu::TextureDescriptor {
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			label: None,
			mip_level_count: 1,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			sample_count: 1,
			size: wgpu::Extent3d {
				depth_or_array_layers: layer_count,
				height: texture_size,
				width: texture_size,
			},
			view_formats: &[],
		};

		let texture = context.device.create_texture(&texture_descriptor);

		Memory {
			texture_array_view: texture.create_view(&wgpu::TextureViewDescriptor::default()),

			context,
			next_page_index: 0,
			none_texture: None,
			pages: HashMap::new(),
			queued_writes: Vec::new(),
			staging_belt: Some(wgpu::util::StagingBelt::new(16_000_000)),
			texture_array: texture,
			texture_array_descriptor: texture_descriptor,
			texture_pager: textures::GPUPager::new(layer_count as usize, texture_size as u16),
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
		&self.texture_array
	}

	/// Returns reference to the memory's texture view.
	pub fn get_texture_view(&self) -> &wgpu::TextureView {
		&self.texture_array_view
	}

	/// Returns a reference to the memory's texture descriptor.
	pub fn get_texture_descriptor(&self) -> &'a wgpu::TextureDescriptor {
		&self.texture_array_descriptor
	}

	/// Finds a spot for the texture and uploads it to the GPU.
	pub fn upload_texture(&mut self, texture: &Rc<textures::Texture>) {
		if self.texture_pager.is_allocated(texture) { // skip upload if already uploaded
			return;
		}

		if let Some(position) = self.texture_pager.allocate_texture(texture) {
			let data = match self.texture_array_descriptor.format {
				wgpu::TextureFormat::Astc { block: _, channel: _, } => {
					if let textures::TextureData::Astc(data, _) = texture.get_data() {
						data
					} else {
						panic!("Expected Astc texture format for {}", texture.get_file_name())
					}
				},
				wgpu::TextureFormat::Bc3RgbaUnorm | wgpu::TextureFormat::Bc3RgbaUnormSrgb => {
					if let textures::TextureData::Bc3(data) = texture.get_data() {
						data
					} else {
						panic!("Expected Bc3 texture format for {}", texture.get_file_name())
					}
				},
				wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
					if let textures::TextureData::Raw(data) = texture.get_data() {
						data
					} else {
						panic!("Expected raw texture format for {}", texture.get_file_name())
					}
				},
				_ => todo!(),
			};

			// do the upload
			self.context.queue.write_texture(
				wgpu::ImageCopyTexture {
					aspect: wgpu::TextureAspect::All,
					mip_level: 0,
					origin: position,
					texture: &self.texture_array,
				},
				&data,
				wgpu::ImageDataLayout {
					bytes_per_row: NonZeroU32::new(texture.get_size().0 as u32 * 4),
					offset: 0,
					rows_per_image: None, // required if there's multiple images
				},
				wgpu::Extent3d {
					depth_or_array_layers: 1,
					height: texture.get_size().1 as u32,
					width: texture.get_size().0 as u32,
				}
			)
		}
	}

	/// Sets the none texture.
	pub fn set_none_texture(&mut self, texture: Rc<textures::Texture>) {
		self.none_texture = Some(texture);
	}

	/// Gets the none texture.
	pub fn get_none_texture(&self) -> Option<Rc<textures::Texture>> {
		self.none_texture.clone()
	}

	/// Checks if the provided texture tree contains the same textures as our tree.
	pub fn is_same_pager<T: textures::Pager>(&self, pager: &T) -> bool {
		&self.texture_pager == pager
	}

	/// Reset the texture pager.
	pub fn reset_pager(&mut self) {
		let (layer_count, texture_size) = self.texture_pager.get_parameters();
		self.texture_pager = textures::GPUPager::new(layer_count as usize, texture_size as u16);
	}

	/// Get a mutable reference to the texture pager. TODO probably shouldn't expose this
	pub fn get_pager_mut(&mut self) -> &mut textures::GPUPager {
		&mut self.texture_pager
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
