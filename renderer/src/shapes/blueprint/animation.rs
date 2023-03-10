use std::{collections::HashMap, fmt::Write};

/// Describes the interpolation algorithm to use in an animation.
#[derive(Debug)]
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

/// Describes the transform of a `Bone` at a specific time in the animation.
#[derive(Debug, Default)]
pub struct Knot {
	pub rotation: Option<glam::Quat>,
	pub rotation_interpolation: Option<Interpolation>,
	pub scale: Option<glam::Vec3>,
	pub scale_interpolation: Option<Interpolation>,
	pub translation: Option<glam::Vec3>,
	pub translation_interpolation: Option<Interpolation>,
}

impl std::fmt::Display for Knot {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(translation) = self.translation {
			formatter.write_fmt(format_args!(
				"t [{},{},{}] ({:?}), ", translation.x, translation.y, translation.z, self.translation_interpolation.as_ref().unwrap()
			))?;
		} else {
			formatter.write_str("t None, ")?;
		}

		if let Some(scale) = self.scale {
			formatter.write_fmt(format_args!(
				"s [{},{},{}] ({:?}), ", scale.x, scale.y, scale.z, self.scale_interpolation.as_ref().unwrap()
			))?
		} else {
			formatter.write_str("s None, ")?;
		}

		if let Some(rotation) = self.rotation {
			formatter.write_fmt(format_args!(
				"r [{},{},{},{}] ({:?})", rotation.x, rotation.y, rotation.z, rotation.w, self.rotation_interpolation.as_ref().unwrap()
			))?;
		} else {
			formatter.write_str("r None")?;
		}

		Ok(())
	}
}

#[derive(Debug, Default)]
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
/// The animation stores lookup tables that are used to set bone transforms.
#[derive(Debug, Default)]
pub struct Animation {
	/// Vector of keyframes in the animation sorted by the time they appear in the animation timeline.
	pub keyframes: Vec<Keyframe>,
	/// Name of the animation.
	pub name: String,
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
