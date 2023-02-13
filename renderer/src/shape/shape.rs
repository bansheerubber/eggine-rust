use anyhow::Context;
use carton::Carton;
use glam::{ Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;

use crate::memory_subsystem::{ Memory, Node, NodeKind, PageUUID, };
use crate::shape::triangulator::triangulator;

#[derive(Debug)]
pub enum ShapeError {
	CartonError(carton::Error),
	FBXParsingError(fbxcel_dom::any::Error),
	FailedTriangulation(String),
	NoPolygonVertices(String),
	UnsupportedVersion,
}

#[derive(Debug)]
struct Mesh {
	indices: Node,
	vertices: Node,
}

#[derive(Debug)]
pub struct Shape {
	buffer: PageUUID,
	meshes: Vec<Mesh>,
}

impl Shape {
	/// Load a shape from a carton.
	pub fn load(file_name: &str, carton: &mut Carton, device: &wgpu::Device, memory: &mut Memory) -> Result<Shape, ShapeError> {
		// load the FBX up from the carton
		let fbx_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(ShapeError::CartonError(error)),
			Ok(fbx_stream) => fbx_stream,
		};

		// use fbx library to parse the fbx
		let document = match AnyDocument::from_seekable_reader(fbx_stream) {
			Err(error) => return Err(ShapeError::FBXParsingError(error)),
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
					indices.push(vertex_index.unwrap().to_u32() as u16);
				}

				meshes.push((vertices, indices));
			}
		}

		// figure out the size of the page we need for the mesh buffer
		let mut size = 0;
		for (vertices, indices) in meshes.iter() {
			size += vertices.len() * 3 * std::mem::size_of::<f32>();
			size += indices.len() * std::mem::size_of::<u16>();
		}

		// allocate a page for all meshes in this FBX
		let page = memory.new_page(size as u64, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, device);

		// go through the mesh data and create nodes for it
		let mut mesh_representations = Vec::new();
		for (vertices, indices) in meshes.iter() {
			// allocate node for `Vec3` vertices
			let vertices = memory.get_page_mut(page).unwrap().allocate_node(
				(vertices.len() * 3 * std::mem::size_of::<f32>()) as u64,
				(3 * std::mem::size_of::<f32>()) as u64,
				NodeKind::Buffer
			).unwrap();

			// allocate node for `u16` indices
			let indices = memory.get_page_mut(page).unwrap().allocate_node(
				(indices.len() * std::mem::size_of::<u16>()) as u64,
				std::mem::size_of::<u16>() as u64,
				NodeKind::Buffer
			).unwrap();

			// push the mesh representation
			mesh_representations.push(Mesh {
				indices,
				vertices,
			});
		}

		// schedule buffer writes
		for ((vertices, indices), mesh) in meshes.iter().zip(mesh_representations.iter()) {
			// serialize vertices
			let mut u8_vertices: Vec<u8> = Vec::new();
			for point in vertices {
				u8_vertices.extend_from_slice(bytemuck::bytes_of(point));
			}

			memory.write_buffer(page, &mesh.vertices, u8_vertices);

			// serialize indices
			let mut u8_indices: Vec<u8> = Vec::new();
			for index in indices {
				u8_indices.extend_from_slice(bytemuck::bytes_of(index));
			}

			memory.write_buffer(page, &mesh.indices, u8_indices);
		}

		Ok(Shape {
			buffer: page,
			meshes: mesh_representations,
		})
	}
}
