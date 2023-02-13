use std::collections::HashMap;
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::{ Node, Page, };
use super::page::PageUUID;

/// Keeps track of all allocated `Page`s, also helps `Page`s upload their data to wgpu buffers.
#[derive(Debug)]
pub struct Memory {
	/// The `WGPUContext` that the memory manager uses to write to buffers.
	context: Rc<WGPUContext>,
	/// The UUID to use for the next allocated page.
	next_page_index: PageUUID,
	/// The pages allocated by this memory manager.
	pages: HashMap<PageUUID, Page>,
	/// Data that the memory manager will write to buffers the next renderer tick.
	queued_writes: Vec<(Vec<u8>, PageUUID, Node)>,
}

impl Memory {
	/// Create a new memory manager that uses the supplied queue to write to buffers.
	pub fn new(context: Rc<WGPUContext>) -> Self {
		Memory {
			context,
			next_page_index: 0,
			pages: HashMap::new(),
			queued_writes: Vec::new(),
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

	/// Invoked by the renderer at the start of every tick, and writes all queued data to buffers.
	pub(crate) fn complete_write_buffers(&mut self) {
		for (data, page, node) in self.queued_writes.iter() {
			self.get_page(*page).unwrap().write_buffer(&node, data);
		}

		self.queued_writes.clear();
	}
}
