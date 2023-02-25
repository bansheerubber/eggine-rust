use std::rc::Rc;

use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher, };

use super::{ Texture, Cell, Tree, Pager, };

/// Implementation of `Pager` that does not represent textures on the GPU, but instead textures on an imaginary GPU.
#[derive(Debug)]
pub struct VirtualPager {
	/// Accumulates the allocated textures.
	hasher: DefaultHasher,
	/// The amount of trees in the pager.
	layer_count: usize,
	/// The size of the textures.
	size: u16,
	/// The physical locations of the textures on the GPU.
	tree: Vec<Tree>,
}

impl Pager for VirtualPager {
	/// Create a new texture pager.
	fn new(layer_count: usize, size: u16) -> Self {
		VirtualPager {
			hasher: DefaultHasher::new(),
			layer_count,
			size,
			tree: vec![Tree::new(size); layer_count as usize],
		}
	}

	/// Wrapper for allocating a texture onto the quad tree. Returns the position within.
	fn allocate_texture(&mut self, texture: &Rc<Texture>) -> Option<wgpu::Origin3d> {
		// figure out where to put the texture
		let mut cell = None;
		let mut layer = 0;
		for i in 0..self.tree.len() {
			cell = self.tree[i].allocate_texture(texture);
			if cell.is_some() {
				layer = i;
				break;
			}
		}

		let Some(cell) = cell else {
			return None;
		};

		let position = wgpu::Origin3d {
			x: cell.get_position().x as u32,
			y: cell.get_position().y as u32,
			z: layer as u32,
		};

		texture.get_id().hash(&mut self.hasher);

		return Some(position);
	}

	/// Returns whether or not a texture has been allocated in the GPU.
	fn is_allocated(&self, _: &Rc<Texture>) -> bool {
		return false;
	}

	/// Returns the cell a texture belongs to.
	fn get_cell(&self, _: &Rc<Texture>) -> Option<&Cell> {
		return None;
	}

	/// Gets the layer count and the texture size.
	fn get_parameters(&self) -> (usize, u16) {
		(self.layer_count, self.size)
	}

	/// Gets the hash which is accumulated with the ID's of every texture allocated to the pager.
	fn get_hash(&self) -> u64 {
		self.hasher.finish()
	}
}

impl<T: Pager> PartialEq<T> for VirtualPager {
	fn eq(&self, other: &T) -> bool {
		self.get_hash() == other.get_hash()
	}
}
