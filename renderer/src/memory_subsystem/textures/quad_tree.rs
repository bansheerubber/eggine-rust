use glam::IVec2;
use std::rc::Rc;

use super::Texture;

#[derive(Clone, Debug)]
pub enum CellChildIndex {
	TopLeft = 0,
	TopRight = 1,
	BottomRight = 2,
	BottomLeft = 3,
}

impl From<usize> for CellChildIndex {
	fn from(value: usize) -> Self {
		match value {
			0 => CellChildIndex::TopLeft,
			1 => CellChildIndex::TopRight,
			2 => CellChildIndex::BottomRight,
			3 => CellChildIndex::BottomLeft,
			_ => CellChildIndex::TopLeft,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum CellKind {
	/// Reference to the texture being stored in this cell.
	Allocated(Rc<Texture>),
	/// The four leaves of the cell.
	Parent([usize; 4]),
	Unallocated,
}

#[derive(Clone, Debug)]
pub struct Cell {
	/// The tagged union containing the data the cell stores.
	kind: CellKind,
	/// Where the cell is located physically within the `TextureRoot`. The origin point of a cell is the upper-left corner
	/// of it.
	position: IVec2,
	/// Size of the cell.
	size: u16,
}

impl Cell {
	pub fn get_position(&self) -> &IVec2 {
		&self.position
	}

	pub fn get_size(&self) -> u16 {
		self.size
	}
}

#[derive(Clone, Debug)]
pub struct Tree {
	cells: Vec<Cell>,
	/// The size of the largest `TextureCell`.
	maximum_size: u16,
}

impl Tree {
	pub fn new(maximum_size: u16) -> Self {
		Tree {
			cells: vec![Cell {
				kind: CellKind::Unallocated,
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

			if cell.kind != CellKind::Unallocated {
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
				children[i] = self.create_cell(CellChildIndex::from(i), split_target, split_size);
			}

			// update the cell kind
			self.cells[split_target].kind = CellKind::Parent(children);

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
	pub fn allocate_texture(&mut self, texture: Rc<Texture>) -> Option<usize> {
		let Some(cell_index) = self.find_empty_cell(texture.get_size().0) else {
			return None;
		};

		self.cells[cell_index].kind = CellKind::Allocated(texture);

		Some(cell_index)
	}

	/// Returns the percentage of cells that have a texture allocated to them.
	pub fn usage(&self) -> f32 {
		let mut total = 0;
		let mut allocated_total = 0;
		for cell in self.cells.iter() {
			match cell.kind {
    		CellKind::Allocated(_) => {
					allocated_total += 1;
					total += 1;
				},
    		CellKind::Parent(_) => {},
    		CellKind::Unallocated => {
					total += 1;
				}
			}
		}

		return allocated_total as f32 / total as f32;
	}

	pub fn get_cell(&self, index: usize) -> &Cell {
		&self.cells[index]
	}

	/// Create a new unallocated cell.
	fn create_cell(&mut self, corner: CellChildIndex, parent: usize, size: u16) -> usize {
		let parent = &self.cells[parent];
		let position = match corner {
			CellChildIndex::TopLeft => parent.position,
			CellChildIndex::TopRight => IVec2::new(size as i32, 0) + parent.position,
			CellChildIndex::BottomRight => IVec2::new(size as i32, size as i32) + parent.position,
			CellChildIndex::BottomLeft => IVec2::new(0, size as i32) + parent.position,
		};

		self.cells.push(Cell {
			kind: CellKind::Unallocated,
			position,
			size,
		});

		return self.cells.len() - 1;
	}
}
