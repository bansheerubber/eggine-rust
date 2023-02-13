use std::collections::HashMap;
use std::rc::Rc;

use super::{ Node, Page, };
use super::page::PageUUID;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
#[derive(Debug)]
pub struct Memory {
	next_page_index: PageUUID,
	pages: HashMap<PageUUID, Page>,
	queue: Rc<wgpu::Queue>,
	queued_writes: Vec<(Vec<u8>, PageUUID, Node)>,
}

impl Memory {
	pub fn new(queue: Rc<wgpu::Queue>) -> Self {
		Memory {
			next_page_index: 0,
			pages: HashMap::new(),
			queue,
			queued_writes: Vec::new(),
		}
	}

	pub fn new_page(&mut self, size: u64, usage: wgpu::BufferUsages, device: &wgpu::Device) -> PageUUID {
		let mut page = Page::new(size, usage, device);
		page.set_uuid(self.next_page_index);
		self.pages.insert(self.next_page_index, page);
		self.next_page_index += 1;

		return self.next_page_index - 1;
	}

	pub fn get_page(&self, index: PageUUID) -> Option<&Page> {
		self.pages.get(&index)
	}

	pub fn get_page_mut(&mut self, index: PageUUID) -> Option<&mut Page> {
		self.pages.get_mut(&index)
	}

	/// Schedules a buffer write for the next frame.
	pub fn write_buffer(&mut self, page: PageUUID, node: &Node, data: Vec<u8>) {
		self.queued_writes.push((data, page, node.clone()));
	}

	/// Invoked by the renderer at the start of every tick, and writes all queued data to buffers.
	pub(crate) fn complete_write_buffers(&mut self) {
		for (data, page, node) in self.queued_writes.iter() {
			self.get_page(*page).unwrap().write_buffer(&node, data, self.queue.clone());
		}

		self.queued_writes.clear();
	}
}
