use glam::Vec4Swizzles;
use std::collections::HashMap;
use std::fmt::Write;

/// Describes the interpolation algorithm to use in an animation.
#[derive(Clone, Copy, Debug)]
pub enum Interpolation {
	CubicSpline,
	Linear,
	Step,
}

impl From<gltf::animation::Interpolation> for Interpolation {
	fn from(value: gltf::animation::Interpolation) -> Self {
		match value {
			gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
			gltf::animation::Interpolation::Linear => Interpolation::Linear,
			gltf::animation::Interpolation::Step => Interpolation::Step,
		}
	}
}

/// Array indices for intermediate calculations.
pub enum Transform {
	Translate = 0,
	Scale,
	Rotate,
}

/// Describes the transform of a `Bone` at a specific time in the animation.
#[derive(Clone, Debug, Default)]
pub struct Knot {
	pub transformation: [Option<(glam::Vec4, Interpolation)>; 3],
}

impl std::fmt::Display for Knot {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some((translation, interpolation)) = self.transformation[Transform::Translate as usize].as_ref() {
			formatter.write_fmt(format_args!(
				"t [{},{},{}] ({:?}), ", translation.x, translation.y, translation.z, interpolation
			))?;
		} else {
			formatter.write_str("t None, ")?;
		}

		if let Some((scale, interpolation)) = self.transformation[Transform::Scale as usize].as_ref() {
			formatter.write_fmt(format_args!(
				"s [{},{},{}] ({:?}), ", scale.x, scale.y, scale.z, interpolation
			))?
		} else {
			formatter.write_str("s None, ")?;
		}

		if let Some((rotation, interpolation)) = self.transformation[Transform::Rotate as usize].as_ref() {
			formatter.write_fmt(format_args!(
				"r [{},{},{},{}] ({:?})", rotation.x, rotation.y, rotation.z, rotation.w, interpolation
			))?;
		} else {
			formatter.write_str("r None")?;
		}

		Ok(())
	}
}

#[derive(Clone, Debug, Default)]
pub struct Keyframe {
	/// Lookup table for translating `Bone` node IDs to their transform at this keyframe.
	pub bone_to_knot: HashMap<usize, Knot>,
	/// Time the keyframe appears in the animation, starting at 0.
	pub time: f32,
}

impl std::fmt::Display for Keyframe {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let index = formatter.width(); // use width to specify a single node index to print

		formatter.write_fmt(format_args!("{}s:", self.time))?;

		for (bone, knot) in self.bone_to_knot.iter() {
			if let Some(index) = index {
				if bone != &index {
					continue;
				}
			}

			formatter.write_fmt(format_args!("\n    #{} => {}", bone, knot))?;
		}

		Ok(())
	}
}

/// Describes a section of an animation timeline. Meshes can have multiple animations, like a walk cycle, a jump, etc.
/// The animation stores lookup tables that are used to set bone transforms. Currently, only one animation can play at
/// a time.
#[derive(Debug)]
pub struct Animation {
	/// Vector of keyframes in the animation sorted by the time they appear in the animation timeline.
	keyframes: Vec<Keyframe>,
	/// Name of the animation.
	name: String,
}

impl std::fmt::Display for Animation {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_fmt(format_args!("animation '{}'", self.name))?;

		for keyframe in self.keyframes.iter() {
			formatter.write_char('\n')?;
			keyframe.fmt(formatter)?;
		}

		Ok(())
	}
}

impl Animation {
	pub fn new(keyframes: Vec<Keyframe>, name: &str) -> Self {
		Animation {
			keyframes,
			name: name.to_string(),
		}
	}

	pub fn get_name(&self) -> &str {
		&self.name
	}

	/// Gets the length of the animation by subtracting the first keyframe time from the last keyframe time
	pub fn get_length(&self) -> f32 {
		self.keyframes[self.keyframes.len() - 1].time - self.keyframes[0].time
	}

	/// Calculates a bone's local transformation matrix based on the supplied animation time. Handles interpolation
	/// keyframes when necessary.
	pub fn transform_bone(&self, bone: usize, time: f32) -> glam::Mat4 {
		let mut start_transformation: [Option<((glam::Vec4, Interpolation), f32)>; 3] = [None, None, None];
		let mut end_transformation: [Option<((glam::Vec4, Interpolation), f32)>; 3] = [None, None, None];

		let mut transformations_written = 0;

		let min_time = self.keyframes[0].time;
		let max_time = self.keyframes[self.keyframes.len() - 1].time;
		let duration = max_time - min_time;
		let translated_time = time - min_time; // translate time into animation timespace

		// wrap time into the range [min_time, max_time)
		let time = if time < min_time {
			translated_time + duration * (translated_time.abs() / duration).ceil() + min_time
		} else if time >= max_time {
			translated_time - duration * (translated_time.abs() / duration).floor() + min_time
		} else {
			time
		};

		// shouldn't need to do this, but floating point error accumulates and can screw up the below code
		let time = f32::clamp(time, min_time, max_time);

		for i in 0..self.keyframes.len() {
			let keyframe = &self.keyframes[i];
			let Some(knot) = keyframe.bone_to_knot.get(&bone) else {
				continue;
			};

			if keyframe.time <= time {
				for i in 0..3 { // assign the start transformation, as long as we haven't found the ending knot
					if end_transformation[i].is_none() && knot.transformation[i].is_some() {
						start_transformation[i] = Some((knot.transformation[i].unwrap(), keyframe.time));
					}
				}
			}

			if keyframe.time > time { // assign the end transformation
				for i in 0..3 {
					if end_transformation[i].is_none() && knot.transformation[i].is_some() {
						end_transformation[i] = Some((knot.transformation[i].unwrap(), keyframe.time));
						transformations_written += 1;
					}
				}
			}

			if transformations_written == 3 {
				break;
			}
		}

		// TODO handle interpolation
		glam::Mat4::from_scale_rotation_translation(
			// handle scale
			start_transformation[Transform::Scale as usize].unwrap().0.0.lerp(
				end_transformation[Transform::Scale as usize].unwrap().0.0,
				(time - start_transformation[Transform::Scale as usize].unwrap().1)
					/ (end_transformation[Transform::Scale as usize].unwrap().1 - start_transformation[Transform::Scale as usize].unwrap().1)
			).xyz(),
			// handle rotation
			glam::Quat::from_vec4(
				start_transformation[Transform::Rotate as usize].unwrap().0.0
			).slerp(
				glam::Quat::from_vec4(end_transformation[Transform::Rotate as usize].unwrap().0.0),
				(time - start_transformation[Transform::Rotate as usize].unwrap().1)
					/ (end_transformation[Transform::Rotate as usize].unwrap().1 - start_transformation[Transform::Rotate as usize].unwrap().1)
			),
			// handle translation
			start_transformation[Transform::Translate as usize].unwrap().0.0.lerp(
				end_transformation[Transform::Translate as usize].unwrap().0.0,
				(time - start_transformation[Transform::Translate as usize].unwrap().1)
					/ (end_transformation[Transform::Translate as usize].unwrap().1 - start_transformation[Transform::Translate as usize].unwrap().1)
			).xyz()
		)
	}
}
