use std::rc::Rc;

use anyhow::Context;
use carton::Carton;
use glam::{ Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;

use crate::memory_subsystem::{ Node, NodeKind, };
use crate::shape::triangulator::triangulator;

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
	Esoteric(String),
	Index,
	Vertex,
}

/// A collection of meshes loaded from a single FBX file.
#[derive(Debug)]
pub struct Blueprint {
	/// The meshes decoded from the FBX.
	meshes: Vec<Mesh>,
}

impl Blueprint {
	/// Load a FBX file from a carton.
	pub fn load(
		file_name: &str, carton: &mut Carton, state: &mut dyn BlueprintState
	) -> Result<Rc<Blueprint>, BlueprintError> {
		// load the FBX up from the carton
		let fbx_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(BlueprintError::CartonError(error)),
			Ok(fbx_stream) => fbx_stream,
		};

		// use fbx library to parse the fbx
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
			state.prepare_mesh_pages();

			let vertex_count = indices.len() as u32; // amount of vertices to render

			// allocate node for `Vec3` vertices
			let vertices = state.get_named_node(
				BlueprintDataKind::Vertex,
				(vertices.len() * 3 * std::mem::size_of::<f32>()) as u64,
				(3 * std::mem::size_of::<f32>()) as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<Node>, ()> {
						eprintln!("Could not allocate node for {:?}", BlueprintDataKind::Vertex);
						Ok(None)
					}
				)
				.unwrap();

			// allocate node for `u32` Indices
			let indices = state.get_named_node(
				BlueprintDataKind::Index,
				(indices.len() * std::mem::size_of::<u32>()) as u64,
				std::mem::size_of::<u32>() as u64,
				NodeKind::Buffer
			)
				.or_else(
					|_| -> Result<Option<Node>, ()> {
						eprintln!("Could not allocate node for {:?}", BlueprintDataKind::Index);
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

				state.write_node(BlueprintDataKind::Vertex, &vertices_node, u8_vertices);
			}

			// serialize indices & write to buffer
			if let Some(indices_node) = &mesh.indices {
				let mut u8_indices: Vec<u8> = Vec::new();
				for index in indices {
					highest_index = std::cmp::max(highest_index, *index);
					u8_indices.extend_from_slice(bytemuck::bytes_of(index));
				}

				state.write_node(BlueprintDataKind::Index, &indices_node, u8_indices);
			}

			num_indices += indices.len();

			mesh.first_index = state.calc_first_index(num_indices as u32);
			mesh.vertex_offset = state.calc_vertex_offset(highest_index as i32);
		}

		Ok(Rc::new(Blueprint {
			meshes: mesh_representations,
		}))
	}

	pub fn get_meshes(&self) -> &Vec<Mesh> {
		&self.meshes
	}
}