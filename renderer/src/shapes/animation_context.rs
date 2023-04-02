/// Describes the looping behavior of an animation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnimationIteration {
	/// Play the animation the specified number of times in a row. If zero, the animation does not play at all.
	Count(u64),
	/// Play the animation forever.
	#[default]
	Infinite,
}

/// Describes how an animation contributes to the pose of a model.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnimationBlending {
	/// Describes whether or not an animation should play over others. An animation with a higher priority will stop an
	/// animation with lower priority from playing.
	pub priority: u64,
	/// Weight used to blend animations on the same priority level together. The weight of the animation describes how
	/// much a single animation contributes to the overall pose, but does not influence the weight of contributions for
	/// any other animations.
	pub weight: f32,
}

/// Describes how quickly the animation should play. If zero, then the animation's timer is not incremented at all and
/// is expected to be controlled by the animation's creater.
pub type AnimationTimescale = f32;

/// Describes the parameters of an active animation.
#[derive(Clone, Debug, Default)]
pub struct AnimationContext {
	/// Blending parameters for the animation.
	pub blending: AnimationBlending,
	/// How many times to play the animation.
	pub looping_behavior: AnimationIteration,
	/// The name of the animation.
	pub name: String,
	/// The point in time the animation is at.
	pub timer: f32,
	/// The speed of the animation playing.
	pub timescale: AnimationTimescale,
}

impl AnimationContext {
	pub fn update_timer(&mut self, deltatime: f32) {
		if self.timescale == 0.0 {
			return;
		}

		self.timer += deltatime * self.timescale;
	}

	pub fn get_timer(&self) -> f32 {
		self.timer
	}
}
