use anyhow::Context;
use carton::Carton;
use glam::{ Vec2, Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::data::mesh::layer::TypedLayerElementHandle;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;
use std::collections::{ HashMap, HashSet, };
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use crate::memory_subsystem::{ Memory, Node, NodeKind, textures, };
use crate::shape;

use super::{ BlueprintState, Mesh, };

#[derive(Debug)]
pub enum BlueprintError {
	CartonError(carton::Error),
	FBXParsingError(fbxcel_dom::any::Error),
	FailedTriangulation(String),
	NoPolygonVertices(String),
	UnsupportedVersion,
}

/// Specifies the kind of data that was loaded from a shape file. Used to communicate what data `Blueprint` wants
/// to store using the `BlueprintState` trait.
#[derive(Debug)]
pub enum BlueprintDataKind {
	Color,
	Esoteric(String),
	Index,
	Normal,
	Position,
	UV,
}

/// A collection of meshes loaded from a single FBX file.
#[derive(Debug)]
pub struct Blueprint {
	/// The FBX we loaded the blueprint from.
	file_name: String,
	/// The meshes decoded from the FBX.
	meshes: Vec<Mesh>,
	/// The textures that the meshes in this blueprint use.
	textures: HashSet<Option<Rc<textures::Texture>>>,
}

impl Hash for Blueprint {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.file_name.hash(state);
	}
}

/// Used for constructing the indexed vertex buffers.
#[derive(Clone, Debug)]
struct Vertex {
	normal: Vec3,
	position: Vec3,
	uv: Vec2,
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
	/// Load a FBX file from a carton.
	pub fn load<T: BlueprintState>(
		file_name: &str, carton: &mut Carton, state: &mut Box<T>, memory: Arc<RwLock<Memory>>
	) -> Result<Rc<Blueprint>, BlueprintError> {
		// load the FBX up from the carton
		let fbx_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(BlueprintError::CartonError(error)),
			Ok(fbx_stream) => fbx_stream,
		};

		// use fbx library to parse the fbx
		// TODO this parser is really slow
		let document = match AnyDocument::from_seekable_reader(fbx_stream) {
			Err(error) => return Err(BlueprintError::FBXParsingError(error)),
			Ok(document) => document,
		};

		let (_, fbx_dom) = match document {
			AnyDocument::V7400(fbx_ver, fbx_dom) => { // note: this is the only FBX version supported
				(fbx_ver, fbx_dom)
			},
			_ => panic!("Unsupported FBX version"),
		};

		let mut meshes = Vec::new();
		let mut textures = HashSet::new();
		let mut num_indices = 0;
		let mut highest_index = 0;

		// look for mesh data and construct data vectors we can then put into wgpu buffers
		for object in fbx_dom.objects() {
			let TypedObjectHandle::Model(TypedModelHandle::Mesh(mesh)) = object.get_typed() else {
				continue;
			};

			let geometry = mesh.geometry().unwrap();
			let layer = geometry.layers().next().unwrap();

			let (texture_file_name, roughness) = Blueprint::load_material(&mesh);
			let (positions, normals, uvs) = Blueprint::load_vertex_data(&mesh, &geometry, &layer);
			let (indices, positions, normals, uvs) = Blueprint::create_indexable_vertices(positions, normals, uvs);

			// begin preparing the memory subsystem for mesh data upload
			state.prepare_mesh_pages();

			// allocate nodes
			let (indices_node, positions_node, normals_node, uvs_node) = Blueprint::allocate_nodes(
				&indices, &positions, &normals, &uvs, state
			);

			// load the texture, if any
			let texture = if let Some(texture_file_name) = texture_file_name {
				let directory = std::path::Path::new(file_name).parent().unwrap().to_str().unwrap();
				let texture_file_name = texture_file_name.replace(".png", ".qoi");

				let mut memory = memory.write().unwrap();
				let format = memory.get_texture_descriptor().format;
				Some(memory.get_pager_mut().load_qoi(&format!("{}/{}", directory, texture_file_name), format, carton).unwrap())
			} else {
				None
			};

			textures.insert(texture.clone()); // put texture into textures set

			// create the mesh representation
			let mut mesh = Mesh {
				first_index: 0,
				indices: indices_node,
				normals: normals_node,
				positions: positions_node,
				roughness,
				texture,
				uvs: uvs_node,
				vertex_count: indices.len() as u32,
				vertex_offset: 0,
			};

			// write vertex information to memory
			(num_indices, highest_index) = Blueprint::write_nodes(
				&indices, &positions, &normals, &uvs, state, &mesh, num_indices, highest_index
			);

			// update indices/offsets for mesh
			mesh.first_index = state.calc_first_index(num_indices as u32);
			mesh.vertex_offset = state.calc_vertex_offset(highest_index as i32);

			meshes.push(mesh); // push to final output
		}

		Ok(Rc::new(Blueprint {
			file_name: file_name.to_string(),
			meshes,
			textures,
		}))
	}

	pub fn get_meshes(&self) -> &Vec<Mesh> {
		&self.meshes
	}

	pub fn get_textures(&self) -> &HashSet<Option<Rc<textures::Texture>>> {
		&self.textures
	}

	/// Helper function that extracts the texture from FBX data.
	fn load_material(mesh: &fbxcel_dom::v7400::object::model::MeshHandle) -> (Option<String>, f32) {
		// TODO support multiple materials per mesh
		let mut texture_file_name = None;
		let mut roughness = 0.5;
		for material in mesh.materials() {
			let Some(diffuse_texture) = material.diffuse_texture() else {
				continue;
			};

			let Some(video_clip) = diffuse_texture.video_clip() else {
				continue;
			};

			let Ok(file_name) = video_clip.relative_filename() else {
				continue;
			};

			texture_file_name = Some(file_name.to_string());

			let shininess = material.properties().shininess().unwrap().unwrap() as f32;
			if shininess >= 81.0 - f32::EPSILON { // cap this at 0.1 roughness
				roughness = 0.1;
			} else {
				roughness = (10.0 - shininess.sqrt() as f32) / 10.0;
			}
		}

		(texture_file_name, roughness)
	}

	/// Helper function that extracts vertex positions, normals, and UVs.
	fn load_vertex_data(
		mesh: &fbxcel_dom::v7400::object::model::MeshHandle,
		geometry: &fbxcel_dom::v7400::object::geometry::MeshHandle,
		layer: &fbxcel_dom::v7400::data::mesh::layer::LayerHandle
	) -> (Vec<Vec3>, Vec<Vec3>, Vec<Vec2>) {
		// get the vertices from the mesh and triangulate them
		let triangulated_vertices = geometry
			.polygon_vertices()
			.context(format!("Could not get polygon vertices for mesh {:?}", mesh.name()))
			.unwrap()
			.triangulate_each(shape::triangulator)
			.context(format!("Could not triangulate vertices for mesh {:?}", mesh.name()))
			.unwrap();

		// get the raw vertex data
		let mut positions: Vec<Vec3> = Vec::new();
		for index in triangulated_vertices.triangle_vertex_indices() {
			let position = triangulated_vertices.control_point(index).unwrap();
			positions.push(Vec3 {
					x: position.x as f32,
					y: position.y as f32,
					z: position.z as f32,
			});
		}

		// get the normals vector
		let mut normals = Vec::new();
		let raw_normals = layer
			.layer_element_entries()
			.find_map(|entry| match entry.typed_layer_element() {
					Ok(TypedLayerElementHandle::Normal(handle)) => Some(handle),
					_ => None,
			})
			.unwrap()
			.normals()
			.context(format!("Could not get normals for mesh {:?}", mesh.name()))
			.unwrap();

		for index in triangulated_vertices.triangle_vertex_indices() {
			let normal = raw_normals.normal(&triangulated_vertices, index).unwrap();
			normals.push(
				Vec3 {
					x: normal.x as f32,
					y: normal.y as f32,
					z: normal.z as f32,
				}
			);
		}

		// get the UVs vector
		let mut uvs = Vec::new();
		let raw_uvs = layer
			.layer_element_entries()
			.find_map(|entry| match entry.typed_layer_element() {
					Ok(TypedLayerElementHandle::Uv(handle)) => Some(handle),
					_ => None,
			})
			.unwrap()
			.uv()
			.context(format!("Could not get normals for mesh {:?}", mesh.name()))
			.unwrap();

		for index in triangulated_vertices.triangle_vertex_indices() {
			let uv = raw_uvs.uv(&triangulated_vertices, index).unwrap();
			uvs.push(
				Vec2 {
					x: uv.x as f32,
					y: uv.y as f32,
				}
			);
		}

		(positions, normals, uvs)
	}

	/// Helper function that iterates through the supplied vertex data and creates an index buffer for Vulkan rendering.
	/// The vertex data is deduplicated.
	fn create_indexable_vertices(
		positions: Vec<Vec3>, normals: Vec<Vec3>, uvs: Vec<Vec2>,
	) -> (Vec<shape::IndexType>, Vec<Vec3>, Vec<Vec3>, Vec<Vec2>) {
		// build the indexed buffers for positions + normals, indexed buffers are deduplicated on position/normal pairs.
		// we need to deduplicate vertex position + normals so we can abuse the memory savings we get from using indexed
		// draw calls. without deduplication, indexed draw calls make no sense
		let mut lookup: HashMap<Vertex, shape::IndexType> = HashMap::new();
		let mut deduplicated_positions: Vec<Vec3> = Vec::new();
		let mut deduplicated_normals: Vec<Vec3> = Vec::new();
		let mut deduplicated_uvs: Vec<Vec2> = Vec::new();
		let mut indices: Vec<shape::IndexType> = Vec::new();
		let mut next_index = 0;

		// go through positions/normals, build a hash map that is used for deduplication
		for i in 0..positions.len(){
			let position = positions[i];
			let normal = normals[i];
			let uv = uvs[i];

			let vertex = Vertex {
				normal: normal.clone(),
				position: position.clone(),
				uv: uv.clone(),
			};

			if lookup.contains_key(&vertex) {
				let index = lookup.get(&vertex).unwrap();
				indices.push(*index);
			} else { // if we have a unique vertex, then add its position/normal to the deduplicated output
				deduplicated_positions.push(position.clone());
				deduplicated_normals.push(normal.clone());
				deduplicated_uvs.push(uv.clone());

				indices.push(next_index);

				lookup.insert(vertex, next_index);

				next_index += 1
			}
		}

		(indices, deduplicated_positions, deduplicated_normals, deduplicated_uvs)
	}

	/// Helper function that allocates memory nodes for vertex data.
	fn allocate_nodes<T: BlueprintState>(
		indices: &Vec<shape::IndexType>,
		positions: &Vec<Vec3>,
		normals: &Vec<Vec3>,
		uvs: &Vec<Vec2>,
		state: &mut Box<T>
	) -> (Option<Node>, Option<Node>, Option<Node>, Option<Node>) {
		// allocate node for `u32` Indices
		let indices = state.get_named_node(
			BlueprintDataKind::Index,
			(indices.len() * std::mem::size_of::<shape::IndexType>()) as u64,
			std::mem::size_of::<shape::IndexType>() as u64,
			NodeKind::Buffer
		)
			.or_else(
				|_| -> Result<Option<Node>, ()> {
					eprintln!("Could not allocate node for {:?}", BlueprintDataKind::Index);
					Ok(None)
				}
			)
			.unwrap();

		// allocate node for `Vec3` positions
		let positions = state.get_named_node(
			BlueprintDataKind::Position,
			(positions.len() * 3 * std::mem::size_of::<f32>()) as u64,
			std::mem::size_of::<f32>() as u64,
			NodeKind::Buffer
		)
			.or_else(
				|_| -> Result<Option<Node>, ()> {
					eprintln!("Could not allocate node for {:?}", BlueprintDataKind::Position);
					Ok(None)
				}
			)
			.unwrap();

		// allocate node for `Vec3` normals
		let normals = state.get_named_node(
			BlueprintDataKind::Normal,
			(normals.len() * 3 * std::mem::size_of::<f32>()) as u64,
			std::mem::size_of::<f32>() as u64,
			NodeKind::Buffer
		)
			.or_else(
				|_| -> Result<Option<Node>, ()> {
					eprintln!("Could not allocate node for {:?}", BlueprintDataKind::Normal);
					Ok(None)
				}
			)
			.unwrap();

		// allocate node for `Vec2` uvs
		let uvs = state.get_named_node(
			BlueprintDataKind::UV,
			(uvs.len() * 2 * std::mem::size_of::<f32>()) as u64,
			std::mem::size_of::<f32>() as u64,
			NodeKind::Buffer
		)
			.or_else(
				|_| -> Result<Option<Node>, ()> {
					eprintln!("Could not allocate node for {:?}", BlueprintDataKind::UV);
					Ok(None)
				}
			)
			.unwrap();

		(indices, positions, normals, uvs)
	}

	/// Helper function that writes vertex information to memory.
	fn write_nodes<T: BlueprintState>(
		indices: &Vec<shape::IndexType>,
		positions: &Vec<Vec3>,
		normals: &Vec<Vec3>,
		uvs: &Vec<Vec2>,
		state: &mut Box<T>,
		mesh: &Mesh,
		mut num_indices: usize,
		mut highest_index: usize,
	) -> (usize, usize) {
		// serialize indices & write to buffer
		if let Some(indices_node) = &mesh.indices {
			let mut u8_indices: Vec<u8> = Vec::new();
			for index in indices {
				highest_index = std::cmp::max(highest_index, *index as usize);
				u8_indices.extend_from_slice(bytemuck::bytes_of(index));
			}

			state.write_node(BlueprintDataKind::Index, &indices_node, u8_indices);
		}

		// serialize positions & write to buffer
		if let Some(positions_node) = &mesh.positions {
			let mut u8_positions: Vec<u8> = Vec::new();
			for point in positions {
				u8_positions.extend_from_slice(bytemuck::bytes_of(point));
			}

			state.write_node(BlueprintDataKind::Position, &positions_node, u8_positions);
		}

		// serialize normals & write to buffer
		if let Some(normals_node) = &mesh.normals {
			let mut u8_normals: Vec<u8> = Vec::new();
			for normal in normals {
				u8_normals.extend_from_slice(bytemuck::bytes_of(normal));
			}

			state.write_node(BlueprintDataKind::Normal, &normals_node, u8_normals);
		}

		// serialize uvs & write to buffer
		if let Some(uvs_node) = &mesh.uvs {
			let mut u8_uvs: Vec<u8> = Vec::new();
			for uv in uvs {
				u8_uvs.extend_from_slice(bytemuck::bytes_of(uv));
			}

			state.write_node(BlueprintDataKind::UV, &uvs_node, u8_uvs);
		}

		num_indices += indices.len();

		(num_indices, highest_index)
	}
}
