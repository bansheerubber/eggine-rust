use crate::shapes;

/// Specifies the kind of data that was loaded from a shape file. Used to communicate what data `Blueprint` wants
/// to store using the `BlueprintState` trait.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataKind {
	BoneWeights,
	Color,
	Index,
	Normal,
	Position,
	UV,
}

impl DataKind {
	pub fn element_size(&self) -> usize {
		static FLOAT_SIZE: usize = std::mem::size_of::<shapes::FloatType>();
		static INDEX_SIZE: usize = std::mem::size_of::<shapes::IndexType>();

		match *self {
			DataKind::BoneWeights => FLOAT_SIZE,
			DataKind::Color => FLOAT_SIZE,
			DataKind::Index => INDEX_SIZE,
			DataKind::Normal => FLOAT_SIZE,
			DataKind::Position => FLOAT_SIZE,
			DataKind::UV => FLOAT_SIZE,
		}
	}

	pub fn element_count(&self) -> usize {
		match *self {
			DataKind::BoneWeights => 4,
			DataKind::Color => 4,
			DataKind::Index => 1,
			DataKind::Normal => 3,
			DataKind::Position => 3,
			DataKind::UV => 2,
		}
	}

	pub fn is_compatible(&self, accessor: &gltf::Accessor) -> bool {
		// unsigned integer type conversion for indices is supported, so do not return false if the integer width doesn't
		// match when we're checking index compatibility
		if accessor.data_type().size() != self.element_size() && self != &DataKind::Index {
			return false;
		}

		// check floatness/integerness/signedness of `gltf::accessor::DataType`
		match *self {
			DataKind::BoneWeights => { // do not allow integers
				if accessor.data_type() != gltf::accessor::DataType::F32 {
					return false;
				}
			},
			DataKind::Color => { // do not allow integers
				if accessor.data_type() != gltf::accessor::DataType::F32 {
					return false;
				}
			},
			DataKind::Index => { // do not allow signed integers
				if accessor.data_type() == gltf::accessor::DataType::I8
					|| accessor.data_type() == gltf::accessor::DataType::I16
				{
					return false;
				}
			},
			DataKind::Normal => { // do not allow integers
				if accessor.data_type() != gltf::accessor::DataType::F32 {
					return false;
				}
			},
			DataKind::Position => { // do not allow integers
				if accessor.data_type() != gltf::accessor::DataType::F32 {
					return false;
				}
			},
			DataKind::UV => { // do not allow integers
				if accessor.data_type() != gltf::accessor::DataType::F32 {
					return false;
				}
			},
		}

		match accessor.dimensions() {
			gltf::accessor::Dimensions::Scalar => {
				if self.element_count() != 1 {
					return false;
				}
			},
			gltf::accessor::Dimensions::Vec2 => {
				if self.element_count() != 2 {
					return false;
				}
			},
			gltf::accessor::Dimensions::Vec3 => {
				if self.element_count() != 3 {
					return false;
				}
			},
			gltf::accessor::Dimensions::Vec4 => {
				if self.element_count() != 4 {
					return false;
				}
			},
			gltf::accessor::Dimensions::Mat2 | gltf::accessor::Dimensions::Mat3 | gltf::accessor::Dimensions::Mat4 => {
				return false
			},
		}

		return true;
	}
}
