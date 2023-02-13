use std::rc::Rc;
use std::sync::{ Arc, RwLock, };

use anyhow::Context;
use carton::Carton;
use glam::{ Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;

use crate::memory_subsystem::{ Memory, Node, NodeKind, };
use crate::shape::triangulator::triangulator;
use super::shape_buffer::ShapeBuffer;

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
	indices: Node,
	/// Points to the mesh's vertex vec3 data. Vertices are f32s.
	vertices: Node,
	/// The amount of vertices in the mesh.
	pub vertex_count: u32,
	/// Used for indirect rendering.
	pub vertex_offset: i32,
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
		file_name: &str, carton: &mut Carton, memory: Arc<RwLock<Memory>>, shape_buffer: &mut ShapeBuffer,
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

		let mut memory = memory.write().unwrap();

		// go through the mesh data and create nodes for it
		let mut mesh_representations = Vec::new();
		for (vertices, indices) in meshes.iter() {
			let vertex_count = indices.len() as u32; // amount of vertices to render

			// allocate node for `Vec3` vertices
			let vertices = memory.get_page_mut(shape_buffer.vertex_page).unwrap().allocate_node(
				(vertices.len() * 3 * std::mem::size_of::<f32>()) as u64,
				(3 * std::mem::size_of::<f32>()) as u64,
				NodeKind::Buffer
			).unwrap();

			// allocate node for `u32` indices
			let indices = memory.get_page_mut(shape_buffer.index_page).unwrap().allocate_node(
				(indices.len() * std::mem::size_of::<u32>()) as u64,
				std::mem::size_of::<u32>() as u64,
				NodeKind::Buffer
			).unwrap();

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
		let mut index_count = 0;
		let mut highest_index = 0;
		for ((vertices, indices), mesh) in meshes.iter().zip(mesh_representations.iter_mut()) {
			// serialize vertices & write to buffer
			let mut u8_vertices: Vec<u8> = Vec::new();
			for point in vertices {
				u8_vertices.extend_from_slice(bytemuck::bytes_of(point));
			}

			memory.write_buffer(shape_buffer.vertex_page, &mesh.vertices, u8_vertices);

			// serialize indices & write to buffer
			let mut u8_indices: Vec<u8> = Vec::new();
			for index in indices {
				highest_index = std::cmp::max(highest_index, *index);
				u8_indices.extend_from_slice(bytemuck::bytes_of(index));
			}

			index_count += indices.len();

			// take the first_index from the indices_written property, then accumulate
			let first_index = shape_buffer.indices_written;
			shape_buffer.indices_written += index_count as u32;

			// take the vertex_offset from the highest_vertex_offset property, then accumulate
			let vertex_offset = shape_buffer.highest_vertex_offset;
			shape_buffer.highest_vertex_offset += highest_index as i32;

			mesh.first_index = first_index;
			mesh.vertex_offset = vertex_offset;

			memory.write_buffer(shape_buffer.index_page, &mesh.indices, u8_indices);
		}

		Ok(Rc::new(ShapeBlueprint {
			meshes: mesh_representations,
		}))
	}

	pub fn get_meshes(&self) -> &Vec<Mesh> {
		&self.meshes
	}
}
