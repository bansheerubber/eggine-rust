/// Stores data about a uniform, used for generating descriptor sets
#[derive(Debug)]
pub struct Uniform {
	pub binding: u32,
	pub kind: String,
	pub name: String,
	pub set: u32,
}
