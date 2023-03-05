use crate::memory_subsystem::textures;
use crate::shapes;

#[derive(Debug)]
pub(crate) struct Batch<'a> {
	pub batch_parameters: Vec<&'a shapes::BatchParameters>,
	pub texture_pager: textures::VirtualPager,
}
