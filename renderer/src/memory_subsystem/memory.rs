use super::Page;
use super::page::PageUUID;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
pub struct Memory<'a> {
	next_page_index: PageUUID,
	pages: Vec<Page>,
	queue: &'a wgpu::Queue,
}

impl<'q> Memory<'q> {
	pub fn new(queue: &'q wgpu::Queue) -> Self {
		Memory {
			next_page_index: 0,
			pages: Vec::new(),
			queue,
		}
	}

	pub fn new_page(&mut self, size: u64, usage: wgpu::BufferUsages, device: &wgpu::Device) -> PageUUID {
		let mut page = Page::new(size, usage, device);
		page.set_uuid(self.next_page_index);
		self.pages.push(page);
		self.next_page_index += 1;

		return self.next_page_index - 1;
	}
}
