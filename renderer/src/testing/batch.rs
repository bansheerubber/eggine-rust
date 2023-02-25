use crate::memory_subsystem::textures;
use crate::shape;

#[derive(Debug)]
pub(crate) struct Batch<'a> {
	pub batch_parameters: Vec<&'a shape::BatchParameters>,
	pub texture_pager: textures::VirtualPager,
}
