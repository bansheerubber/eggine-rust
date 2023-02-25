use std::rc::Rc;

use super::{ Texture, Cell, };

pub trait Pager: PartialEq {
	fn new(layer_count: usize, size: u16) -> Self;

	/// Allocates a texture onto the pager's quad tree. Returns the position of the allocated cell. If no allocation is
	/// possible, then `None` is returned.
	fn allocate_texture(&mut self, texture: &Rc<Texture>) -> Option<wgpu::Origin3d>;

	/// Whether or not the texture has been allocated onto the quad tree.
	fn is_allocated(&self, texture: &Rc<Texture>) -> bool;

	/// Returns the cell associated with the provided texture.
	fn get_cell(&self, texture: &Rc<Texture>) -> Option<&Cell>;

	/// Returns the layer count, and the maximum size of cells in the quad tree.
	fn get_parameters(&self) -> (usize, u16);

	/// Gets the hash which is accumulated with the ID's of every texture allocated to the pager.
	fn get_hash(&self) -> u64;
}

