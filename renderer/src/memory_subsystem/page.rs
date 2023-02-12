use std::collections::VecDeque;

use super::node::{ Node, NodeKind, align_to, };

#[derive(Debug)]
pub struct Page {
	nodes: VecDeque<Node>,
	size: u64,
}

impl Page {
	pub fn new(size: u64) -> Self {
		Page {
			nodes: vec![Node { // initialize with empty node with thet size of the entire page
				align: 1,
				kind: NodeKind::Buffer,
				offset: 0,
				size: size,
			}].into(),
			size,
		}
	}

	/// Allocates a node into the page by allocating a node from unused nodes. Alignment must be non-zero.
	pub fn allocate_node(&mut self, size: u64, align: u64, kind: NodeKind) {
		let size = align_to(size, align);

		// find a node that can fit the new node into it
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

		let Some(mut found_node) = found_node else {
			return;
		};

		let padding = align_to(self.nodes[found_node].offset, align) - self.nodes[found_node].offset;

		// split the node that we found
		if self.nodes[found_node].size != size + padding
			|| padding != 0
		{
			// insert unused space before the element to ensure alignment
			if padding != 0 {
				self.nodes.insert(found_node, Node {
					align: 1,
					kind: NodeKind::Unused,
					offset: self.nodes[found_node].offset,
					size: padding,
				});

				found_node += 1;
			}

			self.nodes.insert(found_node, Node {
				align,
				kind,
				offset: self.nodes[found_node].offset + padding,
				size,
			});

			found_node += 1;

			// assign new offset & size to the node we just stole from
			self.nodes[found_node].offset = self.nodes[found_node].offset + size + padding;
			self.nodes[found_node].size -= size + padding;
			self.defragment(found_node + 1);
		} else { // if the unused node is the exact size we want with correctly aligned offset, just steal it
			self.nodes[found_node].kind = kind;
			self.nodes[found_node].align = align;
		}
	}

	/// Marks the node at the specified index as unused.
	pub fn deallocate_node(&mut self, index: usize) {
		self.nodes[index].kind = NodeKind::Unused;
		self.nodes[index].align = 1;
		self.defragment(index + 1);
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
		if self.nodes.len() == 0 {
			println!("{:#?}", self);
			panic!("Page has zero nodes left");
		}

		if self.nodes[0].offset != 0 {
			println!("{:#?}", self);
			panic!("Page begins with node that has non-zero offset '{}'", self.nodes[0].offset);
		}

		let mut last_size = 0;
		let mut last_offset = 0;
		let mut total_size = 0;
		for node in self.nodes.iter() {
			let correct_offset = last_offset + last_size;
			if node.offset != correct_offset {
				println!("{:#?}", self);
				panic!(
					"Page has node offset '{}' that is not equal to the sum of previous node's offset and size '{}'",
					node.offset,
					correct_offset,
				);
			}

			if node.kind == NodeKind::Unused && node.align != 1 {
				println!("{:#?}", self);
				panic!("Page has unused node that has non-one alignment of '{}'", node.align);
			}

			if node.align == 0 {
				println!("{:#?}", self);
				panic!("Page has node that has alignment of zero");
			}

			if node.size == 0 {
				println!("{:#?}", self);
				panic!("Page has node that has size of zero");
			}

			last_size = node.size;
			last_offset = node.offset;
			total_size += node.size;
		}

		if self.size != total_size {
			println!("{:#?}", self);
			panic!("Page expected size '{}' does not match node purported size '{}'", self.size, total_size);
		}
	}
}
