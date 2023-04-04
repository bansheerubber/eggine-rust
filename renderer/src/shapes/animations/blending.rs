/// Describes how an animation contributes to the pose of a model.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blending {
	/// Describes whether or not an animation should play over others. An animation with a higher priority will stop an
	/// animation with lower priority from playing.
	pub priority: u64,
	/// Weight used to blend animations on the same priority level together. The weight of the animation describes how
	/// much a single animation contributes to the overall pose, but does not influence the weight of contributions for
	/// any other animations.
	pub weight: f32,
}
