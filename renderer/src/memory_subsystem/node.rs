#[derive(Debug, Eq, PartialEq)]
pub enum NodeKind {
	Buffer,
	Unused,
}

/// Describes a suballocated piece of the buffer from a page.
#[derive(Debug)]
pub struct Node {
	/// The alignment of the piece of the buffer.
	pub align: u64,
	/// The kind of data being stored at this piece of the buffer.
	pub kind: NodeKind,
	/// This piece's offset relative to the start of the buffer.
	pub offset: u64,
	/// The size of this piece of the buffer.
	pub size: u64,
}

/// Rounds `number` up to the closest multiple of `align`.
pub fn align_to(number: u64, align: u64) -> u64 {
	return (number + align - 1) & !(align.overflowing_sub(1).0);
}
