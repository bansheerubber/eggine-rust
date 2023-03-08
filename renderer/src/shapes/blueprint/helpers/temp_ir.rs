use std::collections::{ HashMap, HashSet, };
use std::rc::Rc;

use crate::shapes::blueprint::{ DataKind, MeshPrimitive, helpers, };

type VertexAttributeMap = Rc<dyn Fn(&mut TempIR, gltf::accessor::DataType, gltf::accessor::Dimensions, &[u8], &mut [u8])>;

/// Intermediate representation, used for storing temporary information that is shared between functions.
pub struct TempIR {
	/// Functions that transcode data into the form the eggine requires for its vertex attribute buffers.
	pub attribute_default_mappings: HashMap<DataKind, VertexAttributeMap>,
	/// Highest index found in one primitive. The primitive loading function uses this as an additional return value.
	pub highest_index: i32,
	/// The indices of joints that skins defined.
	pub joint_ids: HashSet<usize>,
	/// Intermediate representation of meshes. Bones are added to meshes after the entire GLTF tree has been loaded.
	pub meshes: Vec<(usize, Vec<MeshPrimitive>, Vec<(usize, glam::Mat4)>)>,
}

impl Default for TempIR {
	fn default() -> TempIR {
		let mut attribute_default_mappings: HashMap<DataKind, VertexAttributeMap> = HashMap::new();

		// index mapping (finds highest index used in primitive)
		attribute_default_mappings.insert(
			DataKind::Index,
			Rc::new(|ir, _, _, data, output| {
				let index = helpers::integer::convert_integer(data, output, data.len(), output.len());
				ir.highest_index = std::cmp::max(ir.highest_index, index as i32);
			})
		);

		TempIR {
			attribute_default_mappings,
			highest_index: 0,
			joint_ids: HashSet::new(),
			meshes: Vec::new(),
		}
	}
}

impl TempIR {
	pub fn get_attribute_map(&self, kind: DataKind) -> Option<VertexAttributeMap> {
		self.attribute_default_mappings.get(&kind).cloned()
	}
}
