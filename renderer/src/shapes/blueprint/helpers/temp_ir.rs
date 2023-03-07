use std::collections::HashMap;
use std::rc::Rc;

use crate::shapes::blueprint::DataKind;

type VertexAttributeMap = Rc<dyn Fn(&mut TempIR, gltf::accessor::DataType, gltf::accessor::Dimensions, &[u8], &mut [u8])>;

/// Intermediate representation, used for storing temporary information that is shared between functions.
#[derive(Default)]
pub struct TempIR {
	pub attribute_default_mappings: HashMap<DataKind, VertexAttributeMap>,
	/// Highest index found in one primitive. The primitive loading function uses this as an additional return value.
	pub highest_index: i32,
}

impl TempIR {
	pub fn get_attribute_map(&self, kind: DataKind) -> Option<VertexAttributeMap> {
		self.attribute_default_mappings.get(&kind).cloned()
	}
}
