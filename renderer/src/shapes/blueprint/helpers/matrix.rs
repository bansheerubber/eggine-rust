/// Converts a gltf transform into a `glam::Mat4`.
pub fn transform_to_mat4(transform: &gltf::scene::Transform) -> glam::Mat4 {
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
