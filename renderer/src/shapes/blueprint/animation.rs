use std::collections::HashMap;

/// Describes the interpolation algorithm to use in an animation.
pub enum Interpolation {
	CubicSpline,
	Linear,
	Step,
}

/// Describes the transform of a `Bone` at a specific time in the animation.
pub struct Knot {
	interpolation: Interpolation,
	rotation: Option<glam::Quat>,
	scale: Option<glam::Vec3>,
	translation: Option<glam::Vec3>,
}

pub struct Keyframe {
	/// Lookup table for translating `Bone` node IDs to their transform at this keyframe.
	bone_to_knot: HashMap<usize, Knot>,
	/// Time the keyframe appears in the animation, starting at 0.
	time: f32,
}

/// Describes a section of an animation timeline. Meshes can have multiple animations, like a walk cycle, a jump, etc.
/// The animation stores lookup tables that are used to set bone transforms.
pub struct Animation {
	/// Vector of keyframes in the animation sorted by the time they appear in the animation timeline.
	pub keyframes: Vec<Keyframe>,
	/// Name of the animation.
	pub name: String,
}
