use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use anyhow::Context;
use carton::Carton;
use glam::{ Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;

use crate::memory_subsystem::{ Memory, Node, NodeKind, PageError, };
use crate::shape::triangulator::triangulator;

/// Controls how a `ShapeBlueprint` allocates memory and helps with calculating `first_index` and `vertex_offset`
/// properties for `Mesh`s
pub trait ShapeBlueprintState {
	/// Calculates the first index of the mesh, which a `Mesh` uses to index into the index buffer.
	///
	/// * `num_indices` - The amount of index values written in the index buffer. Additional disambiguation: index values
	/// are repeated in the index buffer, and `num_indices` includes all the repeats since it is the total number of
	/// indices written into the index buffer.
	fn calc_first_index(&mut self, num_indices: u32) -> u32;

	/// Calculates the vertex offset of the mesh, which a 'Mesh' uses to calculate the base vertex index used to index
	/// into the vertex buffer.
	///
	/// * `last_highest_index` - The highest index value from the mesh that the `vertex_offset` will be assigned to. If an
	/// index buffer uses numbers within the range 0..=32, then the `highest_index` passed to this function should be
	/// `32`.
	fn calc_vertex_offset(&mut self, highest_index: i32) -> i32;

	/// Prepares memory for the next mesh.
	fn prepare_mesh_pages(&mut self);

	/// Gets the node that the blueprint will store information into.
	///
	/// * `name`      - A descriptor for the kind of data stored in the node. Vec3 vertex information would be stored in a
	/// separate node from Vec4 color information, with the correct node specified by `name`. The `name` does not describe
	/// the GLSL type of data stored, so additional parameters are necessary for node allocation.
	/// * `size`      - The expected size of the returned node.
	/// * `align`     - The expected alignment of the returned node.
	/// * `node_kind` - The expected kind of the returned node.
	///
	/// # Return value
	/// Since an implementation of this function may allocate new nodes, a `PageError` is returned so the blueprint can
	/// handle them gracefully. The onus of pretty-printing memory error debug is on callers.
	/// If the unwrapped result is `None`, then the `ShapeBlueprintState` implementation does not support storing the kind
	/// of data described by `name`, and `ShapeBlueprint` should throw away any such data it loaded.
	fn get_named_node(
		&self,
		name: ShapeBlueprintDataKind,
		size: u64,
		align: u64,
		node_kind: NodeKind,
	) -> Result<Option<Node>, PageError>;

	/// Wrapper function for writing data into the specified node.
	fn write_node(&mut self, node: &Node, buffer: Vec<u8>);
}

#[derive(Debug)]
pub enum ShapeBlueprintError {
	CartonError(carton::Error),
	FBXParsingError(fbxcel_dom::any::Error),
	FailedTriangulation(String),
	NoPolygonVertices(String),
	UnsupportedVersion,
}

/// Represents buffer data associated with a particular mesh.
#[derive(Debug)]
pub struct Mesh {
	/// Used for indirect rendering.
	pub first_index: u32,
	/// Points to the mesh's vertex indices. Indices are u16s.
	indices: Option<Node>,
	/// Points to the mesh's vertex vec3 data. Vertices are f32s.
	vertices: Option<Node>,
	/// The amount of vertices in the mesh.
	pub vertex_count: u32,
	/// Used for indirect rendering.
	pub vertex_offset: i32,
}

/// Specifies the kind of data that was loaded from a shape file. Used to communicate what data `ShapeBlueprint` wants
/// to store using the `ShapeBlueprintState` trait.
#[derive(Debug)]
pub enum ShapeBlueprintDataKind {
	Esoteric(String),
	Index,
	Vertex,
}

/// A collection of meshes loaded from a single FBX file.
#[derive(Debug)]
pub struct ShapeBlueprint {
	/// The meshes decoded from the FBX.
	meshes: Vec<Mesh>,
}

impl ShapeBlueprint {
	/// Load a FBX file from a carton.
	pub fn load(
		file_name: &str, carton: &mut Carton, state: &mut dyn ShapeBlueprintState
	) -> Result<Rc<ShapeBlueprint>, ShapeBlueprintError> {
		// load the FBX up from the carton
		let fbx_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(ShapeBlueprintError::CartonError(error)),
			Ok(fbx_stream) => fbx_stream,
		};

		// use fbx library to parse the fbx
		let document = match AnyDocument::from_seekable_reader(fbx_stream) {
			Err(error) => return Err(ShapeBlueprintError::FBXParsingError(error)),
			Ok(document) => document,
		};

		let (_, fbx_dom) = match document {
			AnyDocument::V7400(fbx_ver, fbx_dom) => { // note: this is the only FBX version supported
				(fbx_ver, fbx_dom)
			},
			_ => panic!("Unsupported FBX version"),
		};

		// look for mesh data and construct data vectors we can then put into wgpu buffers
		let mut meshes = Vec::new();
		for object in fbx_dom.objects() {
			if let TypedObjectHandle::Model(TypedModelHandle::Mesh(mesh)) = object.get_typed() {
				let geometry = mesh.geometry().unwrap();

				// get the vertices from the mesh and triangulate them
				let triangulated_vertices = geometry
					.polygon_vertices()
					.context(format!("Could not get polygon vertices for mesh {:?}", mesh.name()))
					.unwrap()
					.triangulate_each(triangulator)
					.context(format!("Could not triangulate vertices for mesh {:?}", mesh.name()))
					.unwrap();

				// get the raw vertex data
				let mut vertices = Vec::new();
				for vertex in triangulated_vertices.polygon_vertices().raw_control_points().unwrap() {
					vertices.push(Vec3 {
						x: vertex.x as f32,
						y: vertex.y as f32,
						z: vertex.z as f32,
					});
				}

				// get the index vector
				let mut indices = Vec::new();
				for vertex_index in triangulated_vertices.iter_control_point_indices() {
					indices.push(vertex_index.unwrap().to_u32() as u32);
				}

				meshes.push((vertices, indices));
			}
		}

		// go through the mesh data and create nodes for it
		let mut mesh_representations = Vec::new();
		for (vertices, indices) in meshes.iter() {
			let vertex_count = indices.len() as u32; // amount of vertices to render

			// allocate node for `Vec3` vertices
			let vertices = state.get_named_node(
				ShapeBlueprintDataKind::Vertex,
				(vertices.len() * 3 * std::mem::size_of::<f32>()) as u64,
				(3 * std::mem::size_of::<f32>()) as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<Node>, ()> {
						eprintln!("Could not allocate node for {:?}", ShapeBlueprintDataKind::Vertex);
						Ok(None)
					}
				)
				.unwrap();

			// allocate node for `u32` Indices
			let indices = state.get_named_node(
				ShapeBlueprintDataKind::Index,
				(indices.len() * std::mem::size_of::<u32>()) as u64,
				std::mem::size_of::<u32>() as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<Node>, ()> {
						eprintln!("Could not allocate node for {:?}", ShapeBlueprintDataKind::Index);
						Ok(None)
					}
				)
				.unwrap();

			// push the mesh representation
			mesh_representations.push(Mesh {
				first_index: 0,
				indices,
				vertices,
				vertex_count,
				vertex_offset: 0,
			});
		}

		// schedule buffer writes
		let mut num_indices = 0;
		let mut highest_index = 0;
		for ((vertices, indices), mesh) in meshes.iter().zip(mesh_representations.iter_mut()) {
			// serialize vertices & write to buffer
			if let Some(vertices_node) = &mesh.vertices {
				let mut u8_vertices: Vec<u8> = Vec::new();
				for point in vertices {
					u8_vertices.extend_from_slice(bytemuck::bytes_of(point));
				}

				state.write_node(&vertices_node, u8_vertices);
			}

			// serialize indices & write to buffer
			if let Some(indices_node) = &mesh.indices {
				let mut u8_indices: Vec<u8> = Vec::new();
				for index in indices {
					highest_index = std::cmp::max(highest_index, *index);
					u8_indices.extend_from_slice(bytemuck::bytes_of(index));
				}

				state.write_node(&indices_node, u8_indices);
			}

			num_indices += indices.len();

			// take the first_index from the indices_written property, then accumulate
			// let first_index = shape_buffer.indices_written;
			// shape_buffer.indices_written += index_count as u32;

			// take the vertex_offset from the highest_vertex_offset property, then accumulate
			// let vertex_offset = shape_buffer.highest_vertex_offset;
			// shape_buffer.highest_vertex_offset += highest_index as i32;

			mesh.first_index = state.calc_first_index(num_indices as u32);
			mesh.vertex_offset = state.calc_vertex_offset(highest_index as i32);
		}

		Ok(Rc::new(ShapeBlueprint {
			meshes: mesh_representations,
		}))
	}

	pub fn get_meshes(&self) -> &Vec<Mesh> {
		&self.meshes
	}
}
