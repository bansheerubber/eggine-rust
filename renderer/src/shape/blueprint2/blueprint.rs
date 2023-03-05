use std::collections::{ HashMap, HashSet, };
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use carton::Carton;

use crate::memory_subsystem::{ Memory, Node, NodeKind, textures, };
use crate::shape;

use super::{ Error, Mesh, MeshPrimitive, MeshPrimitiveKind, State, };

/// Specifies the kind of data that was loaded from a shape file. Used to communicate what data `Blueprint` wants
/// to store using the `BlueprintState` trait.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DataKind {
	Color,
	Index,
	Normal,
	Position,
	UV,
}

impl DataKind {
	pub fn element_size(&self) -> usize {
		static FLOAT_SIZE: usize = std::mem::size_of::<shape::FloatType>();
		static INDEX_SIZE: usize = std::mem::size_of::<shape::IndexType>();

		match *self {
			DataKind::Color => FLOAT_SIZE,
			DataKind::Index => INDEX_SIZE,
			DataKind::Normal => FLOAT_SIZE,
			DataKind::Position => FLOAT_SIZE,
			DataKind::UV => FLOAT_SIZE,
		}
	}

	pub fn element_count(&self) -> usize {
		match *self {
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

/// A collection of meshes loaded from a single FBX file.
#[derive(Debug)]
pub struct Blueprint {
	/// The GLTF file we loaded the blueprint from.
	file_name: String,
	/// The meshes decoded from the GLTF.
	meshes: Vec<Rc<Mesh>>,
	/// The textures `Mesh`s are dependent on.
	textures: Vec<Rc<textures::Texture>>,
}

impl Hash for Blueprint {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.file_name.hash(state);
	}
}

/// Used for constructing the indexed vertex buffers.
#[derive(Clone, Debug)]
struct Vertex {
	normal: glam::Vec3,
	position: glam::Vec3,
	uv: glam::Vec2,
}

impl Eq for Vertex {}

impl PartialEq for Vertex {
	fn eq(&self, other: &Self) -> bool {
		self.normal == other.normal && self.position == other.position && self.uv == other.uv
	}
}

impl Hash for Vertex {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.normal.x.to_bits().hash(state);
		self.normal.y.to_bits().hash(state);
		self.normal.z.to_bits().hash(state);

		self.position.x.to_bits().hash(state);
		self.position.y.to_bits().hash(state);
		self.position.z.to_bits().hash(state);

		self.uv.x.to_bits().hash(state);
		self.uv.y.to_bits().hash(state);
	}
}

impl Blueprint {
	/// Load a GLTF file from a carton.
	pub fn load<T: State>(
		file_name: &str, carton: &mut Carton, state: &mut Box<T>, memory: Arc<RwLock<Memory>>
	) -> Result<Rc<Blueprint>, Error> {
		// load the GLTF up from the carton
		let gltf_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(Error::CartonError(error)),
			Ok(gltf_stream) => gltf_stream,
		};

		let gltf = gltf::Gltf::from_reader(gltf_stream).unwrap();

		let mut blueprint = Blueprint {
			file_name: file_name.to_string(),
			meshes: Vec::new(),
			textures: vec![state.get_none_texture()], // TODO do not always reference the none texture
		};

		// build the `Blueprint` tree
		for scene in gltf.scenes() {
			for node in scene.nodes() {
				Self::parse_tree(&mut blueprint, &gltf, &node, state, memory.clone()).unwrap();
			}
		}

		Ok(Rc::new(blueprint))
	}

	/// Gets the textures the `Mesh`s are dependent on.
	pub fn get_textures(&self) -> &Vec<Rc<textures::Texture>> {
		&self.textures
	}

	/// Get the meshes owned by the `Blueprint`.
	pub fn get_meshes(&self) -> &Vec<Rc<Mesh>> {
		&self.meshes
	}

	/// Parses the GLTF tree and adds loaded structures into the `Blueprint`.
	fn parse_tree<T: State>(
		blueprint: &mut Blueprint, gltf: &gltf::Gltf, node: &gltf::Node, state: &mut Box<T>, memory: Arc<RwLock<Memory>>
	) -> Result<Option<Rc<Mesh>>, Error> {
		let Some(mesh) = node.mesh() else {
			return Ok(None);
		};

		// load the primitives
		let mut primitives = Vec::new();
		for primitive in mesh.primitives() {
			if primitive.mode() != gltf::mesh::Mode::Triangles {
				eprintln!("Unsupported primitive mode");
				continue;
			}

			// the reader is used to read indices and that is it. i want to have granular control over the stuff that i'm
			// reading for memory system reasons, so we do the rest of the data fetching using accessors
			// TODO the reader only supports embedded binary data right now, maybe support URIs?
			let reader = primitive.reader(|_| { Some(&gltf.blob.as_ref().unwrap()) });

			let Some(indices) = primitive.indices() else {
				return Err(Error::NoIndices);
			};

			// make sure indices are compatible
			if !DataKind::Index.is_compatible(&indices) {
				eprintln!(
					"Index accessor with parameters '{:?}<{:?}>' are not compatible with 'DataKind::{:?}'",
					indices.dimensions(),
					indices.data_type(),
					DataKind::Index
				);

				return Err(Error::NoIndices);
			}

			// associate `DataKind`s to different parts of GLTF and eggine state
			let mut kind_to_accessor = HashMap::new();
			let mut kind_to_node = HashMap::new();

			// populate `kind_to_accessor` for future data fetching
			for (semantic, accessor) in primitive.attributes() {
				// translate GLTF data type into memory system data type
				let kind = match semantic {
					gltf::Semantic::Colors(0) => { // TODO support other color indices? what do they even mean?
						DataKind::Color
					},
					gltf::Semantic::Normals => DataKind::Normal,
					gltf::Semantic::Positions => DataKind::Position,
					gltf::Semantic::TexCoords(0) => DataKind::UV, // TODO support other texture coordinates
					kind => {
						eprintln!("GLTF semantic {:?} not yet supported", kind);
						continue;
					}
    		};

				// check if the `accessor::DataType` and `DataKind` are compatible
				if !kind.is_compatible(&accessor) {
					eprintln!(
						"Accessor with parameters '{:?}<{:?}>' are not compatible with 'DataKind::{:?}'",
						accessor.dimensions(),
						accessor.data_type(),
						kind
					);
				} else {
					kind_to_accessor.insert(kind, accessor);
				}
			}

			state.prepare_mesh_pages();

			// allocate nodes for the `MeshPrimitive`
			for (kind, accessor) in kind_to_accessor.iter() {
				let node = state.get_named_node(
					*kind,
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
				if node.is_none() {
					eprintln!("Node kind {:?} not supported by blueprint state", kind);
				} else {
					kind_to_node.insert(kind, node.unwrap());
				}
			}

			// allocate index node separately
			let index_node = state.get_named_node(
				DataKind::Index,
				(indices.count() * DataKind::Index.element_size() * DataKind::Index.element_count()) as u64,
				DataKind::Index.element_size() as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<Node>, ()> {
						eprintln!("Could not allocate node for {:?}", DataKind::Index);
						Ok(None)
					}
				)
				.unwrap()
				.unwrap();

			// construct indexed eggine buffers. `temp` fills up with a certain amount of data and flushed to GPU VRAM
			let blob = gltf.blob.as_ref().unwrap();
			let mut temp = Vec::new();
			let mut highest_index = 0;
			for (kind, accessor) in kind_to_accessor.iter() {
				if accessor.normalized() { // TODO support integer normalization?
					panic!("Accessor normalization not supported");
				}

				let view = accessor.view().unwrap();

				// stride defaults to the size of elements in the accessor
				let stride = if let Some(stride) = view.stride() {
					stride
				} else {
					accessor.size()
				};

				for index in reader.read_indices().unwrap().into_u32() {
					highest_index = std::cmp::max(highest_index, index as usize); // set the highest index

					let start_buffer_index = index as usize * stride + view.offset() + accessor.offset();

					// bounds check
					assert!(start_buffer_index < view.offset() + accessor.offset() + view.length());

					// copy binary data straight into the buffer (TODO support type conversion?)
					temp.extend_from_slice(&blob[start_buffer_index..start_buffer_index + accessor.size()]);
				}

				state.write_node(*kind, &kind_to_node[kind], temp);
				temp = Vec::new(); // reallocate temp
			}

			// load the indices into VRAM
			{
				// statically evaluate this to hopefully influence some compiler optimization magic in the below for loops
				static INDEX_SIZE: usize = std::mem::size_of::<shape::IndexType>();

				let view = indices.view().unwrap();

				let stride = if let Some(stride) = view.stride() {
					stride
				} else {
					indices.size()
				};

				let start_index = view.offset() + indices.offset();

				if indices.size() != INDEX_SIZE { // we're dealing with u16 data
					for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
						if INDEX_SIZE == 4 { // pad u16 data to get us to a u32 size
							temp.extend_from_slice(&blob[buffer_index..buffer_index + indices.size()]);
							temp.push(0);
							temp.push(0);
						} else { // no conversion needed
							temp.extend_from_slice(&blob[buffer_index..buffer_index + indices.size()]);
						}
					}
				} else { // we're dealing with u32 data
					for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
						if INDEX_SIZE == 4 { // no conversion needed
							temp.extend_from_slice(&blob[buffer_index..buffer_index + indices.size()]);
						} else { // truncate upper half of u32 data
							temp.extend_from_slice(&blob[buffer_index..buffer_index + 2]);
						}
					}
				}

				state.write_node(DataKind::Index, &index_node, temp);
			}

			// construct the mesh primitive
			primitives.push(MeshPrimitive {
				first_index: state.calc_first_index(indices.count() as u32),
				indices: Some(index_node),
				kind: MeshPrimitiveKind::Triangle,
				normals: kind_to_node.remove(&DataKind::Normal),
				positions: kind_to_node.remove(&DataKind::Position),
				uvs: kind_to_node.remove(&DataKind::UV),
				vertex_count: indices.count() as u32,
				vertex_offset: state.calc_vertex_offset(highest_index as i32),
			});
		}

		let mut mesh = Mesh {
			children: Vec::new(),
			primitives,
			transform: glam::Mat4::IDENTITY,
		};

		// parse the rest of the children
		for child in node.children() {
			match Self::parse_tree(blueprint, gltf, &child, state, memory.clone()) {
				Ok(child) => { // add children meshes
					if let Some(child) = child {
						mesh.children.push(child);
					}
				},
				Err(error) => return Err(error),
			}
		}

		// make blueprint own mesh and return a copy of the mesh (so other meshes can add the mesh as a child)
		let mesh = Rc::new(mesh);
		let output = mesh.clone();
		blueprint.meshes.push(mesh);

		Ok(Some(output))
	}
}
