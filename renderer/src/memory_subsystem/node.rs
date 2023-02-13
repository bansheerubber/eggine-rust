#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NodeKind {
	Buffer,
	#[default]
	Unused,
}

/// Describes a suballocated piece of the buffer from a page.
#[derive(Clone, Debug, Default)]
pub struct Node {
	/// The alignment of the piece of the buffer.
	pub(crate) align: u64,
	/// Index unique to this buffer within its page.
	pub(crate) index: u64,
	/// The kind of data being stored at this piece of the buffer.
	pub(crate) kind: NodeKind,
	/// This piece's offset relative to the start of the buffer. The offset does not change as long as the kind is not
	/// unused.
	pub offset: u64,
	/// The size of this piece of the buffer. The size does not change as long as the kind is not unused,
	pub size: u64,
}

impl PartialEq for Node {
	fn eq(&self, other: &Self) -> bool {
		self.index == other.index
	}
}

/// Rounds `number` up to the closest multiple of `align`.
pub fn align_to(number: u64, align: u64) -> u64 {
	return (number + align - 1) & !(align.overflowing_sub(1).0);
}
