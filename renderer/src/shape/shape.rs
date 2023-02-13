use anyhow::Context;
use carton::Carton;
use glam::{ Vec3, };
use fbxcel_dom::any::AnyDocument;
use fbxcel_dom::v7400::object::TypedObjectHandle;
use fbxcel_dom::v7400::object::model::TypedModelHandle;

use crate::shape::triangulator::triangulator;

pub enum ShapeError {
	CartonError(carton::Error),
	FBXParsingError(fbxcel_dom::any::Error),
	FailedTriangulation(String),
	NoPolygonVertices(String),
	UnsupportedVersion,
}

pub struct Shape {

}

impl Shape {
	/// Load a shape from a carton.
	pub fn load(file_name: &str, carton: &mut Carton) -> Result<Shape, ShapeError> {
		// load the FBX up from the carton
		let fbx_stream = match carton.get_file_data(file_name) {
			Err(error) => return Err(ShapeError::CartonError(error)),
			Ok(fbx_stream) => fbx_stream,
		};

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
				let mut points = Vec::new();
				for vertex in triangulated_vertices.polygon_vertices().raw_control_points().unwrap() {
					points.push(Vec3 {
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

				meshes.push((points, indices));
			}
		}

		Ok(Shape {

		})
	}
}
