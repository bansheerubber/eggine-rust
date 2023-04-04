use lazy_static::lazy_static;
use std::sync::Mutex;

use crate::shapes::animations;

lazy_static! {
	static ref NEXT_ANIMATION_ID: Mutex<u64> = Mutex::new(0);
}

/// Describes the parameters of an active animation.
#[derive(Clone, Debug, Default)]
pub struct Context {
	/// Blending parameters for the animation.
	pub(crate) blending: animations::Blending,
	/// The ID assigned to this animation context. Allows for animation lookup.
	pub(crate) id: animations::Id,
	/// How many times to play the animation.
	pub(crate) looping_behavior: animations::PlayCount,
	/// The name of the animation.
	pub(crate) name: String,
	/// Whether or not the animation is playing.
	paused: bool,
	/// The point in time the animation is at.
	pub(crate) timer: f32,
	/// The speed of the animation playing.
	pub(crate) timescale: animations::Timescale,
}

impl Context {
	pub fn new(
		name: &str,
		blending: animations::Blending,
		looping_behavior: animations::PlayCount,
		timescale: animations::Timescale
	) -> Self {
		let mut next_animation_id = NEXT_ANIMATION_ID.lock().unwrap();

		let id = *next_animation_id;
		*next_animation_id += 1;

		Context {
			blending,
			id,
			looping_behavior,
			name: name.to_string(),
			paused: false,
			timer: 0.0,
			timescale,
		}
	}

	pub fn update_timer(&mut self, deltatime: f32) {
		if self.timescale == 0.0 || self.paused {
			return;
		}

		self.timer += deltatime * self.timescale;
	}

	pub fn set_timer(&mut self, time: f32) {
		self.timer = time;
	}

	pub fn get_timer(&self) -> f32 {
		self.timer
	}

	pub fn set_timescale(&mut self, timescale: animations::Timescale) {
		self.timescale = timescale;
	}

	pub fn get_timescale(&mut self) -> animations::Timescale {
		self.timescale
	}

	/// Stop the animation from playing.
	pub fn pause(&mut self) {
		self.paused = true;
	}

	/// Make the animation start playing.
	pub fn unpause(&mut self) {
		self.paused = false;
	}
}
