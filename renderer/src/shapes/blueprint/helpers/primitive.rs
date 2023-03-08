use crate::memory_subsystem::{ Node, NodeKind, };
use crate::shapes::blueprint::{ DataKind, State, helpers, };

/// Takes in a GLTF primitive attribute and loads the data into the eggine's memory subsystem. Performs error checking
/// to ensure that the attribute can be transcoded into the eggine's representation, and also provides functionality for
/// re-mapping data as its read from GLTF buffers.
///
/// If `semantic` is `None`, then assume we are loading index data.
pub fn load_attribute<T: State>(
	semantic: Option<gltf::Semantic>,
	accessor: gltf::Accessor,
	ir: &mut helpers::temp_ir::TempIR,
	state: &mut Box<T>,
	blob: &Vec<u8>
) -> Option<(DataKind, Node)> {
	// translate GLTF data type into memory system data type
	let kind = match semantic {
		Some(gltf::Semantic::Colors(0)) => DataKind::Color, // TODO support other color indices? what do they even mean?
		Some(gltf::Semantic::Joints(0)) => DataKind::BoneIndex, // TODO support different skin indices
		Some(gltf::Semantic::Normals) => DataKind::Normal,
		Some(gltf::Semantic::Positions) => DataKind::Position,
		Some(gltf::Semantic::TexCoords(0)) => DataKind::UV, // TODO support other texture coordinates
		Some(gltf::Semantic::Weights(0)) => DataKind::BoneWeight, // TODO support different skin indices
		None => DataKind::Index,
		kind => {
			eprintln!("GLTF semantic {:?} not yet supported", kind);
			return None;
		}
	};

	if accessor.normalized() { // TODO support integer normalization?
		eprintln!("Accessor normalization not supported");
		return None;
	}

	// check if the `accessor::DataType` and `DataKind` are compatible
	if !kind.is_compatible(&accessor) {
		eprintln!(
			"Accessor with parameters '{:?}<{:?}>' are not compatible with 'DataKind::{:?}'",
			accessor.dimensions(),
			accessor.data_type(),
			kind
		);
		return None;
	}

	// allocate the node using state
	let node = state.get_named_node(
		kind,
		(accessor.count() * kind.element_size() * kind.element_count()) as u64,
		kind.element_size() as u64,
		NodeKind::Buffer
	)
		.or_else(
			|_| -> Result<Option<Node>, ()> {
				eprintln!("Could not allocate node for {:?}", kind);
				Ok(None)
			}
		)
		.unwrap();

	// if the `DataKind` is not supported by the state, then print an error
	let Some(node) = node else {
		eprintln!("Node kind {:?} not supported by blueprint state", kind);
		return None;
	};

	// construct indexed eggine buffers. `temp` fills up with a certain amount of data and flushed to GPU VRAM
	let mut temp = Vec::new();
	temp.resize(kind.element_size() * kind.element_count() * accessor.count(), 0);

	let mut write_index = 0;

	let view = accessor.view().unwrap();

	// stride defaults to the size of elements in the accessor
	let stride = if let Some(stride) = view.stride() {
		stride
	} else {
		accessor.size()
	};

	// emit a warning b/c idk if the type conversion works 100% yet
	if accessor.data_type().size() != kind.element_size() {
		eprintln!("GLTF {:?} size does not match eggine {:?} size, doing type conversion...", semantic, kind);
	}

	let start_index = view.offset() + accessor.offset();

	if let Some(mapping) = ir.get_attribute_map(kind) { // perform data mapping
		for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
			mapping(
				ir,
				accessor.data_type(),
				accessor.dimensions(),
				&blob[buffer_index..buffer_index + accessor.size()],
				&mut temp[write_index..]
			);

			write_index += kind.element_size() * kind.element_count();
		}
	} else { // if no mapping, then do default writing behavior
		if kind.is_float() { // copy entire vector at once
			for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
				let buffer = &blob[buffer_index..buffer_index + accessor.size()];
				let write_length = &blob[buffer_index..buffer_index + accessor.size()].len();
				temp[write_index..write_index + write_length].copy_from_slice(buffer);

				write_index += write_length;
			}
		} else { // step through each element of the vector for type conversion
			let element_size = accessor.data_type().size();
			for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
				for i in 0..accessor.dimensions().multiplicity() {
					let buffer = &blob[buffer_index + i * element_size..buffer_index + (i + 1) * element_size];

					helpers::integer::convert_integer(buffer, &mut temp[write_index..], element_size, kind.element_size());
					write_index += kind.element_size();
				}
			}
		}
	}

	state.write_node(kind, &node, temp);

	Some((kind, node))
}

/// Allocates zeros for the specified `DataKind`.
pub fn allocate_empty<T: State>(kind: DataKind, element_count: usize, state: &mut Box<T>) {
	// allocate the node using state
	let node = state.get_named_node(
		kind,
		(element_count * kind.element_size() * kind.element_count()) as u64,
		kind.element_size() as u64,
		NodeKind::Buffer
	)
		.or_else(
			|_| -> Result<Option<Node>, ()> {
				eprintln!("Could not allocate node for {:?}", kind);
				Ok(None)
			}
		)
		.unwrap();

	// if the `DataKind` is not supported by the state, then print an error
	let Some(node) = node else {
		eprintln!("Node kind {:?} not supported by blueprint state", kind);
		return;
	};

	state.write_node(kind, &node, vec![0; element_count * kind.element_size() * kind.element_count()]);
}
