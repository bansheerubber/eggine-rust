use std::collections::HashMap;
use std::rc::Rc;

use crate::shapes::blueprint::{ DataKind, helpers, };

type VertexAttributeMap = Rc<dyn Fn(&mut TempIR, gltf::accessor::DataType, gltf::accessor::Dimensions, &[u8], &mut [u8])>;

/// Intermediate representation, used for storing temporary information that is shared between functions.
pub struct TempIR {
	/// Functions that transcode data into the form the eggine requires for its vertex attribute buffers.
	pub attribute_default_mappings: HashMap<DataKind, VertexAttributeMap>,
	/// Highest index found in one primitive. The primitive loading function uses this as an additional return value.
	pub highest_index: i32,
	/// The eggine wants to send a tightly packed bone matrix array to shaders. GLTF stores the skeleton as nodes and
	/// uses those node indices in the vertex's joint attribute data. GLTF does not guarantee that the skeleton node
	/// indices are tightly packed, so we can't pass the raw joint attribute data to shaders unless if wasted array space
	/// for non-skeleton nodes is acceptable (it is not acceptable). This table translates node indices into bone indices
	/// that can be used to look up bone matrices in the shader's bone matrix array storage buffer object. The bone
	/// indices stored here are local to the GLTF being loaded, and the absolute bone indices required by the GPU are
	/// calculated in-shader using a process outside of the scope of `TempIR`.
	///
	/// `node_to_bone` is populated by the default `DataKind::Joint` attribute mapping.
	pub node_to_bone: HashMap<usize, u16>,
	/// Loading the skeleton is done out-of-order. First, the mesh is loaded (where the inverse bind matrices are stored),
	/// and then later the skeleton nodes are loaded once they are reached by `Blueprint::parse_tree`. I want to be able
	/// to associate inverses bind matrices with the skeleton nodes, but due to out-of-order loading the association has
	/// to happen once all GLTF nodes have finished parsing. This map performs that function.
	///
	/// TODO are inverse bind matrices the same between meshes? probably not, right? should the key be a joint index, mesh
	/// index pair
	pub node_to_inverse_bind_matrix: HashMap<usize, glam::Mat4>,
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

		// bone index mapping (constructs blueprint's `node_to_bone` table)
		attribute_default_mappings.insert(
			DataKind::BoneIndex,
			Rc::new(|ir, data_kind, dimensions, data, output| { // this is passed a GLTF encoded vector
				for i in 0..dimensions.multiplicity() {
					let node_index = helpers::integer::read_integer(
						&data[i * data_kind.size()..(i + 1) * data_kind.size()], data_kind.size()
					);

					// translate node index into bone index
					let bone_index = if let Some(bone_index) =  ir.node_to_bone.get(&(node_index as usize)) {
						*bone_index
					} else {
						let bone_index = ir.node_to_bone.len();
						ir.node_to_bone.insert(node_index as usize, bone_index as u16);
						bone_index as u16
					};

					// write the bone index to the buffer
					for j in 0..DataKind::BoneIndex.element_size() {
						output[i * DataKind::BoneIndex.element_size() + j] =
							((bone_index >> (j * 8)) & 0xFF) as u8;
					}

					// TODO remove this assert at some point
					assert!(bone_index as u64 == helpers::integer::read_integer(
						&output[i * DataKind::BoneIndex.element_size()..(i + 1) * DataKind::BoneIndex.element_size()],
						DataKind::BoneIndex.element_size()
					));
				}
			})
		);

		TempIR {
			attribute_default_mappings,
			highest_index: 0,
			node_to_bone: HashMap::new(),
			node_to_inverse_bind_matrix: HashMap::new(),
		}
	}
}

impl TempIR {
	pub fn get_attribute_map(&self, kind: DataKind) -> Option<VertexAttributeMap> {
		self.attribute_default_mappings.get(&kind).cloned()
	}
}
