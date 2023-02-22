use std::rc::Rc;

use crate::textures;

#[derive(Debug)]
pub enum TextureCellChild {
	TopLeft = 0,
	TopRight = 1,
	BottomRight = 2,
	BottomLeft = 3,
}

#[derive(Debug, PartialEq)]
pub enum TextureCellKind {
	/// Reference to the texture being stored in this cell.
	Allocated(Rc<textures::Texture>),
	/// The four leaves of the cell.
	Parent([usize; 4]),
	Unallocated,
}

#[derive(Debug)]
pub struct TextureCell {
	/// Size of the cell.
	size: u16,
	kind: TextureCellKind,
}

#[derive(Debug)]
pub struct TextureRoot {
	cells: Vec<TextureCell>,
	/// The size of the largest `TextureCell`.
	maximum_size: u16,
}

impl TextureRoot {
	pub fn new(maximum_size: u16) -> Self {
		TextureRoot {
			cells: vec![TextureCell {
				size: maximum_size,
				kind: TextureCellKind::Unallocated,
			}],
			maximum_size,
		}
	}

	/// Searches for an unallocated cell of the specified size.
	pub fn find_empty_cell(&mut self, size: u16) -> Option<usize> {
		if size > self.maximum_size {
			return None;
		}

		let mut split_target = None;
		let mut smallest_split_size = self.maximum_size;

		for i in 0..self.cells.len() {
			let cell = &self.cells[i];

			if cell.kind != TextureCellKind::Unallocated {
				continue;
			}

			// if the cell is the correct size and also empty, then return it
			if cell.size == size {
				return Some(i);
			}

			// figure out if this cell can be split if we don't find a cell that is the size we want
			if cell.size >= size * 2 && cell.size <= smallest_split_size {
				split_target = Some(i);
				smallest_split_size = cell.size;
			}
		}

		// split a cell if we can
		if let Some(split_target) = split_target {
			let split_size = self.cells[split_target].size / 2;

			// allocate children for the cell
			let mut children = [0; 4];
			for i in 0..4 {
				children[i] = self.create_cell(split_size);
			}

			// update the cell kind
			self.cells[split_target].kind = TextureCellKind::Parent(children);

			// if we found the correct size, then return the first new cell
			if split_size == size {
				return Some(children[0]);
			} else {
				self.find_empty_cell(size)
			}
		} else {
			None
		}
	}

	/// Returns the percentage of cells that have a texture allocated to them.
	pub fn usage(&self) -> f32 {
		let mut total = 0;
		let mut allocated_total = 0;
		for cell in self.cells.iter() {
			match cell.kind {
    		TextureCellKind::Allocated(_) => {
					allocated_total += 1;
					total += 1;
				},
    		TextureCellKind::Parent(_) => {},
    		TextureCellKind::Unallocated => {
					total += 1;
				}
			}
		}

		return allocated_total as f32 / total as f32;
	}

	/// Create a new unallocated cell.
	fn create_cell(&mut self, size: u16) -> usize {
		self.cells.push(TextureCell {
			size,
			kind: TextureCellKind::Unallocated,
		});

		return self.cells.len() - 1;
	}
}
