use super::{ Blending, PlayCount, Timescale, };

/// Describes the parameters of an active animation.
#[derive(Clone, Debug, Default)]
pub struct Context {
	/// Blending parameters for the animation.
	pub blending: Blending,
	/// How many times to play the animation.
	pub looping_behavior: PlayCount,
	/// The name of the animation.
	pub name: String,
	/// The point in time the animation is at.
	pub timer: f32,
	/// The speed of the animation playing.
	pub timescale: Timescale,
}

impl Context {
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
