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
	Allocated,
	/// The four leaves of the cell.
	Parent,
	Unallocated,
}

#[derive(Clone, Debug)]
pub struct Cell {
	/// The tagged union containing the data the cell stores.
	kind: CellKind,
	/// Where the cell is located physically within the `Tree`. The origin point of a cell is the upper-left corner
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
	allocated_cells: Vec<Vec<Cell>>,
	/// The size of the largest `Cell`.
	maximum_size: u16,
	unallocated_cells: Vec<Vec<Cell>>,
}

impl Tree {
	pub fn new(maximum_size: u16) -> Self {
		let mut unallocated_cells = Vec::new();
		for _ in 0..maximum_size.trailing_zeros() - 1 {
			unallocated_cells.push(Vec::new());
		}

		unallocated_cells.push(vec![Cell {
			kind: CellKind::Unallocated,
			position: IVec2::new(0, 0),
			size: maximum_size,
		}]);

		let mut allocated_cells = Vec::new();
		for _ in 0..maximum_size.trailing_zeros() {
			allocated_cells.push(Vec::new());
		}

		Tree {
			allocated_cells,
			maximum_size,
			unallocated_cells,
		}
	}

	pub fn allocate_texture(&mut self, texture: Rc<Texture>) -> Option<Cell> {
		let size = texture.get_size().0;
		if size > self.maximum_size {
			return None;
		}

		let index = size.trailing_zeros() as usize - 1;

		// pick a cell to allocate to
		if self.unallocated_cells[index].len() > 0 {
			let mut cell = self.unallocated_cells[index].pop().unwrap();
			cell.kind = CellKind::Allocated;
			self.allocated_cells[index].push(cell.clone());
			return Some(cell);
		}

		// find the smallest possible cell we can allocate to
		let mut split_size: Option<u16> = None;
		for i in index + 1..self.maximum_size.trailing_zeros() as usize {
			if self.unallocated_cells[i].len() > 0 {
				split_size = Some(i as u16);
				break;
			}
		}

		let Some(split_size) = split_size else {
			return None;
		};

		// get ownership of the cell
		let mut allocation_cell = self.unallocated_cells[split_size as usize].pop().unwrap();

		let mut split_size = 2 << (split_size - 1);

		loop {
			allocation_cell = self.split_cell(allocation_cell, split_size);
			if split_size == size {
				break;
			}

			split_size >>= 1;
		}

		allocation_cell.kind = CellKind::Allocated;
		self.allocated_cells[index].push(allocation_cell.clone());

		Some(allocation_cell)
	}

	fn create_cell(&mut self, corner: CellChildIndex, parent: &Cell, size: u16) -> Cell {
		let position = match corner {
			CellChildIndex::TopLeft => parent.position,
			CellChildIndex::TopRight => IVec2::new(size as i32, 0) + parent.position,
			CellChildIndex::BottomRight => IVec2::new(size as i32, size as i32) + parent.position,
			CellChildIndex::BottomLeft => IVec2::new(0, size as i32) + parent.position,
		};

		Cell {
			kind: CellKind::Unallocated,
			position,
			size,
		}
	}

	fn split_cell(&mut self, mut cell: Cell, size: u16) -> Cell {
		let index = size.trailing_zeros() as usize - 1;

		for i in 1..4 {
			let cell = self.create_cell(CellChildIndex::from(i), &cell, size);
			self.unallocated_cells[index].push(cell);
		}

		let new_cell = self.create_cell(CellChildIndex::from(0), &cell, size);
		cell.kind = CellKind::Parent; // TODO redo how the indices work n stuff
		self.allocated_cells[cell.size.trailing_zeros() as usize - 1].push(cell);

		return new_cell;
	}
}
