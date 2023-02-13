use std::collections::HashMap;
use std::rc::Rc;

use super::Page;
use super::page::PageUUID;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
#[derive(Debug)]
pub struct Memory {
	next_page_index: PageUUID,
	pages: HashMap<PageUUID, Page>,
	queue: Rc<wgpu::Queue>,
}

impl Memory {
	pub fn new(queue: Rc<wgpu::Queue>) -> Self {
		Memory {
			next_page_index: 0,
			pages: HashMap::new(),
			queue,
		}
	}

	pub fn new_page(&mut self, size: u64, usage: wgpu::BufferUsages, device: &wgpu::Device) -> PageUUID {
		let mut page = Page::new(size, usage, device);
		page.set_uuid(self.next_page_index);
		self.pages.insert(self.next_page_index, page);
		self.next_page_index += 1;

		return self.next_page_index - 1;
	}

	pub fn get_page(&mut self, index: PageUUID) -> Option<&Page> {
		self.pages.get(&index)
	}

	pub fn get_page_mut(&mut self, index: PageUUID) -> Option<&mut Page> {
		self.pages.get_mut(&index)
	}
}
