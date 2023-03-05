use std::collections::{ HashMap, HashSet, };
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use carton::Carton;

use crate::memory_subsystem::{ Memory, Node as MemoryNode, NodeKind, textures, };
use crate::shape;

use super::{ DataKind, Error, Material, Mesh, MeshPrimitive, MeshPrimitiveKind, State, };

#[derive(Debug)]
struct Node {
	children: Vec<Rc<Node>>,
	data: NodeData,
	parent: Option<Rc<Node>>,
	transform: glam::Mat4,
}

#[derive(Debug)]
enum NodeData {
	Empty,
	Mesh(Rc<Mesh>),
}

/// A collection of meshes loaded from a single GLTF file.
#[derive(Debug)]
pub struct Blueprint {
	/// The GLTF file we loaded the blueprint from.
	file_name: String,
	/// The meshes decoded from the GLTF.
	meshes: Vec<Rc<Mesh>>,
	/// All nodes decoded from the GLTF.
	nodes: Vec<Rc<Node>>,
	/// The textures `Mesh`s are dependent on.
	textures: HashSet<Rc<textures::Texture>>,
}

impl Hash for Blueprint {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.file_name.hash(state);
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
			nodes: Vec::new(),
			textures: HashSet::new(),
		};

		// build the `Blueprint` tree
		for scene in gltf.scenes() {
			for node in scene.nodes() {
				Self::parse_tree(&mut blueprint, &gltf, &node, state, memory.clone(), carton).unwrap();
			}
		}

		Ok(Rc::new(blueprint))
	}

	/// Gets the textures the `Mesh`s are dependent on.
	pub fn get_textures(&self) -> &HashSet<Rc<textures::Texture>> {
		&self.textures
	}

	/// Get the meshes owned by the `Blueprint`.
	pub fn get_meshes(&self) -> &Vec<Rc<Mesh>> {
		&self.meshes
	}

	/// Recursively parses the GLTF tree and adds loaded structures into the `Blueprint`.
	fn parse_tree<T: State>(
		blueprint: &mut Blueprint,
		gltf: &gltf::Gltf,
		node: &gltf::Node,
		state: &mut Box<T>,
		memory: Arc<RwLock<Memory>>,
		carton: &mut Carton
	) -> Result<Option<Rc<Node>>, Error> {
		let data = if node.mesh().is_some() {
			let mesh = Self::parse_mesh(blueprint, gltf, node, state, memory.clone(), carton)?.unwrap();
			blueprint.meshes.push(mesh.clone());
			NodeData::Mesh(mesh)
		} else {
			NodeData::Empty
		};

		let mut children = Vec::new();

		// parse the rest of the children
		for child in node.children() {
			match Self::parse_tree(blueprint, gltf, &child, state, memory.clone(), carton) {
				Ok(child) => { // add children meshes
					if let Some(child) = child {
						children.push(child);
					}
				},
				Err(error) => return Err(error),
			}
		}

		// create the node
		let node = Rc::new(Node {
			children,
			data,
			transform: glam::Mat4::IDENTITY, // TODO load transform
			parent: None, // TODO get parent stuff working
		});

		blueprint.nodes.push(node.clone());

		Ok(Some(node))
	}

	/// Parses a mesh object and loads all vertex data into memory.
	fn parse_mesh<T: State>(
		blueprint: &mut Blueprint,
		gltf: &gltf::Gltf,
		node: &gltf::Node,
		state: &mut Box<T>,
		memory: Arc<RwLock<Memory>>,
		carton: &mut Carton
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

			let blob = gltf.blob.as_ref().unwrap(); // get reference to binary data

			// associate `DataKind`s to different parts of GLTF and eggine state
			let mut kind_to_node = HashMap::new();

			state.prepare_mesh_pages();

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

				if accessor.normalized() { // TODO support integer normalization?
					eprintln!("Accessor normalization not supported");
					continue;
				}

				// check if the `accessor::DataType` and `DataKind` are compatible
				if !kind.is_compatible(&accessor) {
					eprintln!(
						"Accessor with parameters '{:?}<{:?}>' are not compatible with 'DataKind::{:?}'",
						accessor.dimensions(),
						accessor.data_type(),
						kind
					);
					continue;
				}

				// allocate the node using state
				let node = state.get_named_node(
					kind,
					(accessor.count() * kind.element_size() * kind.element_count()) as u64,
					kind.element_size() as u64,
					NodeKind::Buffer
				)
					.or_else(
						|_| -> Result<Option<MemoryNode>, ()> {
							eprintln!("Could not allocate node for {:?}", kind);
							Ok(None)
						}
					)
					.unwrap();

				// if the `DataKind` is not supported by the state, then print an error
				let Some(node) = node else {
					eprintln!("Node kind {:?} not supported by blueprint state", kind);
					continue;
				};

				// construct indexed eggine buffers. `temp` fills up with a certain amount of data and flushed to GPU VRAM
				let mut temp = Vec::new();
				let view = accessor.view().unwrap();

				// stride defaults to the size of elements in the accessor
				let stride = if let Some(stride) = view.stride() {
					stride
				} else {
					accessor.size()
				};

				let start_index = view.offset() + accessor.offset();

				for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
					// copy binary data straight into the buffer (TODO support type conversion?)
					temp.extend_from_slice(&blob[buffer_index..buffer_index + accessor.size()]);
				}

				state.write_node(kind, &node, temp);

				// store in `kind_to_node` so the `MeshPrimitive` can extract the data
				kind_to_node.insert(kind, node);
			}

			// allocate index node separately
			let index_node = state.get_named_node(
				DataKind::Index,
				(indices.count() * DataKind::Index.element_size() * DataKind::Index.element_count()) as u64,
				DataKind::Index.element_size() as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<MemoryNode>, ()> {
						eprintln!("Could not allocate node for {:?}", DataKind::Index);
						Ok(None)
					}
				)
				.unwrap()
				.unwrap();

			// load the indices into VRAM
			let mut highest_index = 0;
			{
				// statically evaluate this to hopefully influence some compiler optimization magic in the below for loops
				static INDEX_SIZE: usize = std::mem::size_of::<shape::IndexType>();

				let mut temp = Vec::new();
				let view = indices.view().unwrap();

				let stride = if let Some(stride) = view.stride() {
					stride
				} else {
					indices.size()
				};

				let start_index = view.offset() + indices.offset();

				if indices.size() != INDEX_SIZE {
					// emit a warning b/c idk if the type conversion works 100% yet
					eprintln!("GLTF index size do not match eggine index size, doing type conversion...");
				}

				// load the data w/ type conversion
				if indices.size() == 2 { // we're dealing with u16 data
					for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
						let buffer = &blob[buffer_index..buffer_index + indices.size()];

						let index = (buffer[1] as u32) << 8 | buffer[0] as u32;
						highest_index = std::cmp::max(highest_index, index as usize); // set the highest index

						if INDEX_SIZE == 4 { // pad u16 data to get us to a u32 size
							temp.extend_from_slice(buffer);
							temp.push(0);
							temp.push(0);
						} else { // no conversion needed
							temp.extend_from_slice(buffer);
						}
					}
				} else { // we're dealing with u32 data
					for buffer_index in (start_index..start_index + view.length()).step_by(stride) {
						let buffer = &blob[buffer_index..buffer_index + INDEX_SIZE];

						let index = if INDEX_SIZE == 4 { // no conversion needed
							(buffer[3] as u32) << 24 | (buffer[2] as u32) << 16 | (buffer[1] as u32) << 8 | buffer[0] as u32
						} else { // truncate upper half of u32 data
							(buffer[1] as u32) << 8 | buffer[0] as u32
						};

						highest_index = std::cmp::max(highest_index, index as usize); // set the highest index
						temp.extend_from_slice(buffer);
					}
				}

				state.write_node(DataKind::Index, &index_node, temp);
			}

			// load the material
			let material = primitive.material();

			// TODO blender exports a .glb that is all screwed up because it tries to mess with image embedding
			let texture = if let Some(texture) = material.pbr_metallic_roughness().base_color_texture() {
				let source = texture.texture().source().source();
				match source {
					gltf::image::Source::Uri {
						mime_type: _,
						uri: _,
					} => todo!("Blender doesn't export URIs so idk how this got here"),
					gltf::image::Source::View {
						mime_type: _,
						view: _,
					} => todo!("GLTF embedded textures not supported"),
    		}
			} else { // load texture based on material name
				if let Some(name) = material.name() {
					let mut memory = memory.write().unwrap();

					let texture_file_name = name.to_string() + ".qoi";
					let directory = std::path::Path::new(&blueprint.file_name).parent().unwrap().to_str().unwrap();
					let format = memory.get_texture_descriptor().format;

					Some(
						memory.get_pager_mut().load_qoi(&format!("{}/{}", directory, texture_file_name), format, carton).unwrap()
					)
				} else {
					None
				}
			};

			let texture = if let Some(texture) = texture {
				texture
			} else {
				state.get_none_texture()
			};

			blueprint.textures.insert(texture.clone());

			// construct the mesh primitive
			primitives.push(MeshPrimitive {
				first_index: state.calc_first_index(indices.count() as u32),
				indices: Some(index_node),
				kind: MeshPrimitiveKind::Triangle,
				material: Material {
					roughness: material.pbr_metallic_roughness().roughness_factor(),
					texture,
    		},
				normals: kind_to_node.remove(&DataKind::Normal),
				positions: kind_to_node.remove(&DataKind::Position),
				uvs: kind_to_node.remove(&DataKind::UV),
				vertex_count: indices.count() as u32,
				vertex_offset: state.calc_vertex_offset(highest_index as i32),
			});
		}

		Ok(Some(Rc::new(Mesh {
			primitives,
		})))
	}
}
