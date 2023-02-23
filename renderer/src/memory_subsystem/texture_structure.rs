use glam::IVec2;
use std::rc::Rc;

use crate::textures;

#[derive(Clone, Debug)]
pub enum TextureCellChild {
	TopLeft = 0,
	TopRight = 1,
	BottomRight = 2,
	BottomLeft = 3,
}

impl From<usize> for TextureCellChild {
	fn from(value: usize) -> Self {
		match value {
			0 => TextureCellChild::TopLeft,
			1 => TextureCellChild::TopRight,
			2 => TextureCellChild::BottomRight,
			3 => TextureCellChild::BottomLeft,
			_ => TextureCellChild::TopLeft,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextureCellKind {
	/// Reference to the texture being stored in this cell.
	Allocated(Rc<textures::Texture>),
	/// The four leaves of the cell.
	Parent([usize; 4]),
	Unallocated,
}

#[derive(Clone, Debug)]
pub struct TextureCell {
	/// The tagged union containing the data the cell stores.
	kind: TextureCellKind,
	/// Where the cell is located physically within the `TextureRoot`. The origin point of a cell is the upper-left corner
	/// of it.
	position: IVec2,
	/// Size of the cell.
	size: u16,
}

impl TextureCell {
	pub fn get_position(&self) -> &IVec2 {
		&self.position
	}

	pub fn get_size(&self) -> u16 {
		self.size
	}
}

#[derive(Clone, Debug)]
pub struct TextureRoot {
	cells: Vec<TextureCell>,
	/// The size of the largest `TextureCell`.
	maximum_size: u16,
}

impl TextureRoot {
	pub fn new(maximum_size: u16) -> Self {
		TextureRoot {
			cells: vec![TextureCell {
				kind: TextureCellKind::Unallocated,
				position: IVec2::new(0, 0),
				size: maximum_size,
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
				children[i] = self.create_cell(TextureCellChild::from(i), split_target, split_size);
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

	/// Finds an empty cell and allocates the texture to it.
	pub fn allocate_texture(&mut self, texture: Rc<textures::Texture>) -> Option<usize> {
		let Some(cell_index) = self.find_empty_cell(texture.get_size().0) else {
			return None;
		};

		self.cells[cell_index].kind = TextureCellKind::Allocated(texture);

		Some(cell_index)
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

	pub fn get_cell(&self, index: usize) -> &TextureCell {
		&self.cells[index]
	}

	/// Create a new unallocated cell.
	fn create_cell(&mut self, corner: TextureCellChild, parent: usize, size: u16) -> usize {
		let parent = &self.cells[parent];
		let position = match corner {
			TextureCellChild::TopLeft => parent.position,
			TextureCellChild::TopRight => IVec2::new(size as i32, 0) + parent.position,
			TextureCellChild::BottomRight => IVec2::new(size as i32, size as i32) + parent.position,
			TextureCellChild::BottomLeft => IVec2::new(0, size as i32) + parent.position,
		};

		self.cells.push(TextureCell {
			kind: TextureCellKind::Unallocated,
			position,
			size,
		});

		return self.cells.len() - 1;
	}
}
