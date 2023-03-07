use std::cell::RefCell;
use std::collections::{ HashMap, HashSet, };
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use carton::Carton;

use crate::memory_subsystem::{ Memory, textures, };

use super::{ Bone, DataKind, Error, Material, Mesh, Node, NodeData, MeshPrimitive, MeshPrimitiveKind, State, helpers, };

/// A collection of meshes loaded from a single GLTF file.
#[derive(Debug)]
pub struct Blueprint {
	bones: Vec<Bone>,
	/// The GLTF file we loaded the blueprint from.
	file_name: String,
	/// JSON index to node.
	index_to_node: HashMap<usize, Rc<RefCell<Node>>>,
	/// The meshes decoded from the GLTF.
	meshes: Vec<Rc<Mesh>>,
	/// The nodes that contain mesh data.
	mesh_nodes: Vec<(Rc<RefCell<Node>>, Rc<Mesh>)>,
	/// All nodes decoded from the GLTF.
	nodes: Vec<Rc<RefCell<Node>>>,
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
			bones: Vec::new(),
			file_name: file_name.to_string(),
			index_to_node: HashMap::new(),
			meshes: Vec::new(),
			mesh_nodes: Vec::new(),
			nodes: Vec::new(),
			textures: HashSet::new(),
		};

		let mut ir = helpers::temp_ir::TempIR::default();

		ir.attribute_default_mappings.insert(
			DataKind::Index,
			Rc::new(|ir, _, _, data, output| {
				let index = helpers::integer::convert_integer(data, output, data.len(), output.len());
				ir.highest_index = std::cmp::max(ir.highest_index, index as i32);
			})
		);

		// build the `Blueprint` tree
		for scene in gltf.scenes() {
			for node in scene.nodes() {
				Self::parse_tree(&mut blueprint, &mut ir, &gltf, &node, None, state, memory.clone(), carton).unwrap();
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

	/// Get the nodes with mesh data in `Blueprint`.
	pub fn get_mesh_nodes(&self) -> &Vec<(Rc<RefCell<Node>>, Rc<Mesh>)> {
		&self.mesh_nodes
	}

	/// Get the `Bone`s imported by the `Blueprint`.
	pub fn get_bones(&self) -> &Vec<Bone> {
		&self.bones
	}

	/// Recursively parses the GLTF tree and adds loaded structures into the `Blueprint`.
	fn parse_tree<T: State>(
		blueprint: &mut Blueprint,
		ir: &mut helpers::temp_ir::TempIR,
		gltf: &gltf::Gltf,
		node: &gltf::Node,
		parent: Option<Rc<RefCell<Node>>>,
		state: &mut Box<T>,
		memory: Arc<RwLock<Memory>>,
		carton: &mut Carton
	) -> Result<Option<Rc<RefCell<Node>>>, Error> {
		// load node transform
		let transform = Self::transform_to_mat4(&node.transform());

		// create the node
		let new_node = Rc::new(RefCell::new(
			Node {
				children: Vec::new(),
				data: NodeData::Empty,
				local_transform: transform,
				transform: Node::accumulate_transform(parent.clone(), transform),
				parent,
			}
		));

		if node.mesh().is_some() {
			let mesh = Self::parse_mesh(blueprint, ir, gltf, node, state, memory.clone(), carton)?.unwrap();
			blueprint.meshes.push(mesh.clone());
			new_node.borrow_mut().data = NodeData::Mesh(mesh.clone());

			blueprint.mesh_nodes.push((new_node.clone(), mesh));
		} else {
			new_node.borrow_mut().data = NodeData::Empty;
		};

		// parse the rest of the children
		let mut children = Vec::new();
		for child in node.children() {
			match Self::parse_tree(blueprint, ir, gltf, &child, Some(new_node.clone()), state, memory.clone(), carton) {
				Ok(child) => { // add children meshes
					if let Some(child) = child {
						children.push(child);
					}
				},
				Err(error) => return Err(error),
			}
		}

		new_node.borrow_mut().children = children;

		blueprint.index_to_node.insert(node.index(), new_node.clone());
		blueprint.nodes.push(new_node.clone());

		Ok(Some(new_node))
	}

	/// Parses a mesh object and loads all vertex data into memory.
	fn parse_mesh<T: State>(
		blueprint: &mut Blueprint,
		ir: &mut helpers::temp_ir::TempIR,
		gltf: &gltf::Gltf,
		node: &gltf::Node,
		state: &mut Box<T>,
		memory: Arc<RwLock<Memory>>,
		carton: &mut Carton
	) -> Result<Option<Rc<Mesh>>, Error> {
		let Some(mesh) = node.mesh() else {
			return Ok(None);
		};

		let blob = gltf.blob.as_ref().unwrap(); // get reference to binary data

		// load the primitives
		let mut primitives = Vec::new();
		for primitive in mesh.primitives() {
			if primitive.mode() != gltf::mesh::Mode::Triangles {
				eprintln!("Unsupported primitive mode");
				continue;
			}

			let Some(indices_accessor) = primitive.indices() else {
				return Err(Error::NoIndices);
			};

			// associate `DataKind`s to different parts of GLTF and eggine state
			let mut kind_to_node = HashMap::new();

			state.prepare_mesh_pages();

			// populate `kind_to_accessor` for future data fetching
			for (semantic, accessor) in primitive.attributes() {
				// store in `kind_to_node` so the `MeshPrimitive` can extract the data
				if let Some((kind, node)) = helpers::primitive::load_attribute(Some(semantic), accessor, ir, state, blob) {
					kind_to_node.insert(kind, node);
				}
			}

			let indices_count = indices_accessor.count();
			let Some((_, index_node)) = helpers::primitive::load_attribute(None, indices_accessor, ir, state, blob) else {
				return Err(Error::NoIndices);
			};

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
				first_index: state.calc_first_index(indices_count as u32),
				indices: Some(index_node),
				kind: MeshPrimitiveKind::Triangle,
				material: Material {
					roughness: material.pbr_metallic_roughness().roughness_factor(),
					texture,
    		},
				normals: kind_to_node.remove(&DataKind::Normal),
				positions: kind_to_node.remove(&DataKind::Position),
				uvs: kind_to_node.remove(&DataKind::UV),
				vertex_count: indices_count as u32,
				vertex_offset: state.calc_vertex_offset(ir.highest_index),
			});
		}

		// figure out the joints
		let bones = if let Some(skin) = node.skin() {
			let reader = skin.reader(|_| Some(blob.as_slice()));

			let mut inverse_bind_matrices = Vec::new();
			for matrix in reader.read_inverse_bind_matrices().unwrap() { // TODO accept `None` value
				inverse_bind_matrices.push(glam::Mat4::from_cols_array_2d(&matrix));
			}

			let mut output = Vec::new();
			for joint in skin.joints() {
				let bone = Bone {
					inverse_bind_matrix: inverse_bind_matrices[output.len()],
					local_transform: Self::transform_to_mat4(&joint.transform()),
					transform: glam::Mat4::IDENTITY,
				};

				output.push(bone.clone());
				blueprint.bones.push(bone);
			}

			output
		} else {
			Vec::new()
		};

		Ok(Some(Rc::new(Mesh {
			bones,
			primitives,
		})))
	}

	/// Converts a gltf transform into a `glam::Mat4`.
	fn transform_to_mat4(transform: &gltf::scene::Transform) -> glam::Mat4 {
		match transform {
			gltf::scene::Transform::Decomposed {
				rotation,
				scale,
				translation,
			} => {
				glam::Mat4::from_scale_rotation_translation(
					glam::Vec3::from_array(*scale),
					glam::Quat::from_array(*rotation),
					glam::Vec3::from_array(*translation),
				)
			},
			gltf::scene::Transform::Matrix {
				matrix,
			} => {
				glam::Mat4::from_cols_array_2d(&matrix)
			},
		}
	}
}
