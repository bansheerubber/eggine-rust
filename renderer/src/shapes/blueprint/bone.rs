#[derive(Clone, Debug)]
pub struct Bone {
	/// The inverse bind matrix transforms stuff into local bone space.
	pub inverse_bind_matrix: glam::Mat4,
	/// Local transform of the bone.
	pub local_transform: glam::Mat4,
	/// Global transform of where the bone is located in the scene.
	pub transform: glam::Mat4,
}
