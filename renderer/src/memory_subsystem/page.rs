use byte_unit::Byte;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::boss::WGPUContext;

use super::node::{ Node, NodeKind, align_to, };

#[derive(Debug)]
pub enum PageError {
	NodeNotFound,
	NoFreeSpace,
}

pub type PageUUID = u64;

#[derive(Debug)]
pub struct Page {
	buffer: wgpu::Buffer,
	// The page uses context properties to write and manage memory.
	context: Rc<WGPUContext>,
	/// UUID for the `Memory` that this page belongs to.
	index: PageUUID,
	/// Name of the page.
	name: String,
	/// UUID for the next allocated node.
	next_node_index: u64,
	nodes: VecDeque<Node>,
	/// The size of the wgpu buffer that the page allocated on its creation.
	size: u64,
}

impl Page {
	/// Creates a page and allocates a `wgpu` buffer with the specified size.
	pub(crate) fn new(
		size: u64,
		usage: wgpu::BufferUsages,
		name: &str,
		mapped_at_creation: bool,
		context: Rc<WGPUContext>
	) -> Self {
		Page {
			buffer: context.device.create_buffer(&wgpu::BufferDescriptor {
				label: Some(name),
				mapped_at_creation, // TODO get this working, requires some extra stuff according to the docs
				size,
				usage,
			}),
			context,
			index: 0,
			name: name.to_string(),
			next_node_index: 1,
			nodes: vec![Node { // initialize with empty node with thet size of the entire page
				align: 1,
				index: 0,
				kind: NodeKind::Unused,
				offset: 0,
				size: size,
			}].into(),
			size,
		}
	}

	/// Sets the UUID of the `Page`.
	pub fn set_uuid(&mut self, index: u64) {
		self.index = index;
	}

	/// Allocates a node into the page by allocating a node from unused nodes. Alignment must be non-zero. If a space for
	/// a node is found but its offset is not aligned, then an padding node will be allocated before the beginning to
	/// ensure that the node's offset is aligned.
	pub fn allocate_node(&mut self, size: u64, align: u64, kind: NodeKind) -> Result<Node, PageError> {
		let size = align_to(size, align);

		// find an unused node that can fit the new node into it
		let mut found_node = None;
		for i in 0..self.nodes.len() {
			let node = &self.nodes[i];
			// the amount of padding that we need to ensure alignment
			let unused_space = align_to(self.nodes[i].offset, align) - self.nodes[i].offset;

			if size + unused_space <= node.size && node.kind == NodeKind::Unused {
				found_node = Some(i);
				break;
			}
		}

		// if we couldn't find a spot for the node then assume that there wasn't enough space to allocate it
		let Some(mut found_node) = found_node else {
			return Err(PageError::NoFreeSpace);
		};

		// figure out how much padding we need to ensure node alignment
		let padding = align_to(self.nodes[found_node].offset, align) - self.nodes[found_node].offset;

		// split the node that we found
		if self.nodes[found_node].size != size + padding
			|| padding != 0
		{
			// insert unused space before the element to ensure alignment
			if padding != 0 {
				self.nodes.insert(found_node, Node {
					align: 1,
					index: self.next_node_index,
					kind: NodeKind::Unused,
					offset: self.nodes[found_node].offset,
					size: padding,
				});

				self.next_node_index += 1;
				found_node += 1;
			}

			let node = Node {
				align,
				index: self.next_node_index,
				kind,
				offset: self.nodes[found_node].offset + padding,
				size,
			};

			self.nodes.insert(found_node, node.clone());

			self.next_node_index += 1;
			found_node += 1;

			// assign new offset & size to the node we just stole from
			self.nodes[found_node].offset = self.nodes[found_node].offset + size + padding;
			self.nodes[found_node].size -= size + padding;
			self.defragment(found_node + 1);

			Ok(node)
		} else { // if the unused node is the exact size we want with correctly aligned offset, just steal it
			self.nodes[found_node].kind = kind;
			self.nodes[found_node].align = align;

			Ok(self.nodes[found_node].clone())
		}
	}

	/// Marks the node as unused and performs a defragment.
	pub fn deallocate_node(&mut self, node: Node) -> Result<(), PageError> {
		// find the node
		let mut index = None;
		for i in 0..self.nodes.len() {
			if self.nodes[i] == node {
				index = Some(i);
			}
		}

		let Some(index) = index else {
			return Err(PageError::NodeNotFound);
		};

		self.nodes[index].kind = NodeKind::Unused;
		self.nodes[index].align = 1;
		self.defragment(index + 1);

		Ok(())
	}

	/// Returns the page's buffer.
	pub fn get_buffer(&self) -> &wgpu::Buffer {
		return &self.buffer;
	}

	/// Gets a buffer slice from the specified node.
	pub fn get_slice(&self, node: &Node) -> wgpu::BufferSlice {
		return self.buffer.slice(node.offset..node.offset + node.size);
	}

	/// Writes a node into the page's buffer. Does not write the data immediately.
	pub fn write_buffer(&self, node: &Node, data: &Vec<u8>) {
		self.context.queue.write_buffer(&self.buffer, node.offset, data);
	}

	/// Writes a node into the page's buffer. Does not write the data immediately.
	pub fn write_buffer_with_offset(&self, node: &Node, offset: u64, data: &[u8]) {
		self.context.queue.write_buffer(&self.buffer, node.offset + offset, data);
	}

	/// Writes a node into the page's buffer. Does not write the data immediately.
	pub fn write_slice(&self, node: &Node, data: &[u8]) {
		self.context.queue.write_buffer(&self.buffer, node.offset, data);
	}

	/// Writes a node into the page's buffer. Does not write the data immediately.
	pub fn write_slice_with_offset(&self, node: &Node, offset: u64, data: &[u8]) {
		self.context.queue.write_buffer(&self.buffer, node.offset + offset, data);
	}

	/// Get the size of the page.
	pub fn get_size(&self) -> u64 {
		self.size
	}

	/// Combines adjacent unused nodes into single nodes.
	fn defragment(&mut self, index: usize) {
		// defragment unused nodes
		let mut last_node_was_unused = false;
		for i in (0..=std::cmp::min(self.nodes.len() - 1, index)).rev() {
			// remove the last node and grow the current node
			if last_node_was_unused && self.nodes[i].kind == NodeKind::Unused {
				let size = self.nodes[i + 1].size;
				self.nodes.remove(i + 1);
				self.nodes[i].size += size;
			}

			// keep track if last node was unused or not
			if self.nodes[i].kind == NodeKind::Unused {
				last_node_was_unused = true;
			} else {
				last_node_was_unused = false;
			}
		}
	}

	/// Verifies the node structure by checking the following properties:
	/// 1. there should always be at least one node
	/// 2. the first node should begin at offset 0
	/// 3. the following node's offset should be equal to the previous node's offset plus its size
	/// 4. the sum of node sizes should equal the page's size parameter
	/// 5. unused nodes should have an alignment of 1
	/// 6. nodes should have non-zero alignment and size
	pub fn verify(&self) {
		// check #1
		if self.nodes.len() == 0 {
			println!("{:#?}", self);
			panic!("Page has zero nodes left");
		}

		// check #2
		if self.nodes[0].offset != 0 {
			println!("{:#?}", self);
			panic!("Page begins with node that has non-zero offset '{}'", self.nodes[0].offset);
		}

		let mut last_size = 0;
		let mut last_offset = 0;
		let mut total_size = 0;
		for node in self.nodes.iter() {
			let correct_offset = last_offset + last_size;
			if node.offset != correct_offset { // check #3
				println!("{:#?}", self);
				panic!(
					"Page has node offset '{}' that is not equal to the sum of previous node's offset and size '{}'",
					node.offset,
					correct_offset,
				);
			}

			if node.kind == NodeKind::Unused && node.align != 1 { // check #5
				println!("{:#?}", self);
				panic!("Page has unused node that has non-one alignment of '{}'", node.align);
			}

			if node.align == 0 { // check #6
				println!("{:#?}", self);
				panic!("Page has node that has alignment of zero");
			}

			if node.size == 0 { // check #6
				println!("{:#?}", self);
				panic!("Page has node that has size of zero");
			}

			last_size = node.size;
			last_offset = node.offset;
			total_size += node.size;
		}

		if self.size != total_size { // #check 4
			println!("{:#?}", self);
			panic!("Page expected size '{}' does not match node purported size '{}'", self.size, total_size);
		}
	}
}

impl std::fmt::Display for Page {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let bytes = Byte::from_bytes(self.size.into());
		formatter.write_fmt(format_args!("Page '{}' ({})", self.name, bytes.get_appropriate_unit(false).to_string()))
	}
}
