use std::cell::RefCell;
use std::collections::{ HashMap, HashSet, };
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use carton::Carton;

use crate::memory_subsystem::{ Memory, textures, };

use super::{
	DataKind,
	Error,
	Material,
	Mesh,
	MeshPrimitive,
	MeshPrimitiveKind,
	Node,
	NodeData,
	State,
	animation,
	helpers,
};

/// A collection of meshes loaded from a single GLTF file.
#[derive(Debug)]
pub struct Blueprint {
	animations: Vec<animation::Animation>,
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
			animations: Vec::new(),
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

		// put together animations
		let blob = gltf.blob.as_ref().unwrap(); // get reference to binary data
		for animation in gltf.animations() {
			let mut time_to_keyframe: HashMap<u32, animation::Keyframe> = HashMap::new();

			// ensures a knot is present in a keyframe from `time_to_keyframe`
			let insert_knot = |time: f32, time_to_keyframe: &mut HashMap<u32, animation::Keyframe>, node_index: usize| {
				if !time_to_keyframe.contains_key(&time.to_bits()) {
					time_to_keyframe.insert(
						time.to_bits(),
						animation::Keyframe {
							bone_to_knot: HashMap::new(),
							time,
						}
					);
				}

				let keyframe = time_to_keyframe.get_mut(&time.to_bits()).unwrap();
				if !keyframe.bone_to_knot.contains_key(&node_index) {
					keyframe.bone_to_knot.insert(node_index, animation::Knot::default());
				}
			};

			// write knots to keyframes
			for channel in animation.channels() {
				let reader = channel.reader(|_| Some(blob.as_slice()));
				let inputs = reader.read_inputs().unwrap();
				let node_index = channel.target().node().index();
				let interpolation = channel.sampler().interpolation();

				// insert knots
				match reader.read_outputs().unwrap() {
        	gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => println!("not implemented"),
					gltf::animation::util::ReadOutputs::Rotations(iterator) => {
						for (time, rotation) in inputs.zip(iterator.into_f32()) {
							insert_knot(time, &mut time_to_keyframe, node_index);

							let knot = time_to_keyframe.get_mut(&time.to_bits())
								.unwrap()
								.bone_to_knot.get_mut(&channel.target().node().index())
								.unwrap();

							knot.transformation[animation::Transform::Rotate as usize] =
								Some((glam::Vec4::from_array(rotation), interpolation.into()));
						}
					},
        	gltf::animation::util::ReadOutputs::Scales(iterator) => {
						for (time, scale) in inputs.zip(iterator) {
							insert_knot(time, &mut time_to_keyframe, node_index);

							let knot = time_to_keyframe.get_mut(&time.to_bits())
								.unwrap()
								.bone_to_knot.get_mut(&channel.target().node().index())
								.unwrap();

							knot.transformation[animation::Transform::Scale as usize] =
								Some((glam::Vec4::from_array([scale[0], scale[1], scale[2], 0.0]), interpolation.into()));
						}
					},
					gltf::animation::util::ReadOutputs::Translations(iterator) => {
						for (time, translation) in inputs.zip(iterator) {
							insert_knot(time, &mut time_to_keyframe, node_index);

							let knot = time_to_keyframe.get_mut(&time.to_bits())
								.unwrap()
								.bone_to_knot.get_mut(&channel.target().node().index())
								.unwrap();

							knot.transformation[animation::Transform::Translate as usize] =
								Some((glam::Vec4::from_array([translation[0], translation[1], translation[2], 0.0]), interpolation.into()));
						}
					},
    		}
			}

			let mut keyframes = time_to_keyframe.into_values().collect::<Vec<animation::Keyframe>>();
			keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

			blueprint.animations.push(animation::Animation::new(keyframes, animation.name().unwrap()));
		}

		// go through stuff that we know are bones and assign a bone object to their node
		for node_index in ir.joint_ids.iter() {
			let node = blueprint.index_to_node.get_mut(node_index).unwrap();
			node.borrow_mut().data = NodeData::Bone;
		}

		// go through mesh intermediate representation and assign them
		for (node_index, primitives, joints) in ir.meshes {
			let mut bones = Vec::new();
			for (bone_index, inverse_bind_matrix) in joints {
				bones.push((blueprint.index_to_node[&bone_index].clone(), inverse_bind_matrix));
			}

			let mesh = Rc::new(Mesh {
				bones,
				primitives,
			});

			let node = blueprint.index_to_node.get_mut(&node_index).unwrap();
			node.borrow_mut().data = NodeData::Mesh(mesh.clone());

			blueprint.meshes.push(mesh.clone());
			blueprint.mesh_nodes.push((node.clone(), mesh));
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

	/// Get an animation by index.
	pub fn get_animation(&self, index: usize) -> Option<&animation::Animation> {
		if index >= self.animations.len() {
			None
		} else {
			Some(&self.animations[index])
		}
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
		let local_transform = helpers::matrix::transform_to_mat4(&node.transform());

		// create the node
		let new_node = Rc::new(RefCell::new(
			Node {
				children: Vec::new(),
				data: NodeData::Empty,
				gltf_id: node.index(),
				local_transform,
				transform: if let Some(parent) = parent.clone() {
					parent.borrow().transform.mul_mat4(&local_transform)
				} else {
					local_transform
				},
				parent,
			}
		));

		if node.mesh().is_some() {
			let (primitives, joints) = Self::parse_mesh(blueprint, ir, gltf, node, state, memory.clone(), carton)?.unwrap();
			ir.meshes.push((node.index(), primitives, joints))
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
	) -> Result<Option<(Vec<MeshPrimitive>, Vec<(usize, glam::Mat4)>)>, Error> {
		let Some(mesh) = node.mesh() else {
			return Ok(None);
		};

		let blob = gltf.blob.as_ref().unwrap(); // get reference to binary data

		// l	oad the primitives
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

			// store in data `kind_to_node` so the `MeshPrimitive` can extract the data
			let mut element_count = 0;
			for (semantic, accessor) in primitive.attributes() {
				if accessor.count() != element_count && element_count != 0 {
					panic!("Expected vertex attribute accessors to have the same amount of elements as each other.");
				}

				element_count = accessor.count();

				if let Some((kind, node)) = helpers::primitive::load_attribute(Some(semantic), accessor, ir, state, blob) {
					kind_to_node.insert(kind, node);
				}
			}

			let default_kinds = state.required_attributes();
			let mut missing_kinds = default_kinds.iter().filter(|kind| !kind_to_node.contains_key(&kind)).collect::<Vec<&DataKind>>();
			if missing_kinds.contains(&&DataKind::Index) {
				eprintln!("Required attributes cannot contain `{:?}`", DataKind::Index);
				missing_kinds.swap_remove(missing_kinds.iter().position(|x| x == &&DataKind::Index).unwrap());
			}

			// allocate zeros in all missing vertex attributes
			for kind in missing_kinds {
				helpers::primitive::allocate_empty(*kind, element_count, state);
			}

			// load indices
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
		let mut joints = Vec::new();
		if let Some(skin) = node.skin() {
			let reader = skin.reader(|_| Some(blob.as_slice()));

			for (joint, matrix) in skin.joints().zip(reader.read_inverse_bind_matrices().unwrap()) {
				joints.push((joint.index(), glam::Mat4::from_cols_array_2d(&matrix)));

				ir.joint_ids.insert(joint.index());
			}
		}

		Ok(Some((primitives, joints)))
	}
}
