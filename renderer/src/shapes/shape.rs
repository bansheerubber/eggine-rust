use glam::{ Mat4, Quat, Vec3, };
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Mutex;

use crate::shapes;

lazy_static! {
	static ref NEXT_SHAPE_GUID: Mutex<u64> = Mutex::new(0);
}

/// A blueprint that can be instantiated within the scene and take on its own state (position, animation, etc).
#[derive(Debug)]
pub struct Shape {
	/// The animations playing on the shape.
	active_animations: Vec<shapes::animations::Context>,
	blueprint: Rc<shapes::blueprint::Blueprint>,
	id: u64,
	position: Vec3,
	rotation: Quat,
	scale: Vec3,
	transformation: Mat4,
}

impl Hash for Shape {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl Eq for Shape {}

impl PartialEq for Shape {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl Shape {
	pub fn new(blueprint: Rc<shapes::blueprint::Blueprint>) -> Self {
		let mut next_shape_id = NEXT_SHAPE_GUID.lock().unwrap();

		let id = *next_shape_id;
		*next_shape_id += 1;

		Shape {
			active_animations: Vec::new(),
			blueprint,
			id,
			position: Vec3::new(0.0, 0.0, 0.0),
			rotation: Quat::default(),
			scale: Vec3::new(1.0, 1.0, 1.0),
			transformation: Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0)),
		}
	}

	pub fn get_blueprint(&self) -> Rc<shapes::blueprint::Blueprint> {
		self.blueprint.clone()
	}

	/// Plays an animation based on the given animation context.
	pub fn play_animation(&mut self, animation: shapes::animations::Context) -> shapes::animations::Id {
		// do not play animations that would never play to begin with
		if animation.looping_behavior == shapes::animations::PlayCount::Count(0) {
			return 0;
		}

		let id = animation.id;

		self.active_animations.push(animation);

		// sort animations by priority
		self.active_animations.sort_by(|a, b| b.blending.priority.cmp(&a.blending.priority));

		return id;
	}

	/// Gets an animation based on the animation's id.
	pub fn get_animation(&self, id: shapes::animations::Id) -> Option<&shapes::animations::Context> {
		self.active_animations.iter().find(|x| x.id == id)
	}

	/// Gets an animation based on the animation's id.
	pub fn get_animation_mut(&mut self, id: shapes::animations::Id) -> Option<&mut shapes::animations::Context> {
		self.active_animations.iter_mut().find(|x| x.id == id)
	}

	/// Increments the timers for all playing animations.
	pub fn update_animation_timer(&mut self, deltatime: f32) {
		for animation in self.active_animations.iter_mut() {
			animation.update_timer(deltatime);
		}

		// see if animations are done or not
		let mut removed = HashSet::new();
		for animation in self.active_animations.iter() {
			let Some(blueprint_animation) = self.blueprint.get_animation(&animation.name) else {
				break;
			};

			if let &shapes::animations::PlayCount::Count(count) = animation.get_play_count() {
				if f32::floor(animation.get_timer() / blueprint_animation.get_length()) as u64 >= count {
					removed.insert(animation.id);
				}
			}
		}

		self.active_animations.retain(|x| {
			!removed.contains(&x.id)
		});
	}

	/// Calculates a bone's global (relative to bone's root parent) transformation matrix based on the shape's animation
	/// state.
	pub fn get_bone_matrix(
		&mut self,
		bone: &Rc<RefCell<shapes::blueprint::Node>>,
		inverse_bind_matrix: &glam::Mat4,
		inverse_transform: &glam::Mat4
	) -> glam::Mat4 {
		let bone = bone.borrow();
		let unanimated_position = inverse_transform.mul_mat4(&bone.transform.mul_mat4(inverse_bind_matrix));
		if self.active_animations.len() == 0 {
			return unanimated_position;
		}

		// animations store location transformations, so we need to figure out the global transformation by accumulating
		// together the bone's parent transforms
		let mut accumulator = glam::Mat4::ZERO;
		let mut accumulator_set = false;

		let current_priority = self.active_animations[0].blending.priority;

		for animation in self.active_animations.iter() {
			if current_priority != animation.blending.priority {
				break;
			}

			let Some(blueprint_animation) = self.blueprint.get_animation(&animation.name) else {
				break;
			};

			let mut next = bone.parent.clone();
			let mut transform_accumulator = blueprint_animation.transform_bone(bone.gltf_id, animation.get_timer());
			loop {
				if let Some(temp) = next {
					let parent_transform = blueprint_animation.transform_bone(temp.borrow().gltf_id, animation.get_timer());
					transform_accumulator = parent_transform.mul_mat4(&transform_accumulator);
					next = temp.borrow().parent.clone();
				} else {
					break;
				}
			}

			let difference = inverse_transform.mul_mat4(&&transform_accumulator.mul_mat4(inverse_bind_matrix)) - unanimated_position;
			accumulator += difference * animation.blending.weight;

			accumulator_set = true;
		}

		if accumulator_set {
			unanimated_position + accumulator
		} else {
			unanimated_position
		}
	}

	/// Set the shape's position.
	pub fn set_position(&mut self, position: Vec3) {
		self.position = position;

		self.transformation = Mat4::from_scale_rotation_translation(
			self.scale, self.rotation, self.position
		)
	}

	/// Get the shape's position.
	pub fn get_position(&self) -> Vec3 {
		self.position
	}

	/// Set the shape's rotation.
	pub fn set_rotation(&mut self, rotation: Quat) {
		self.rotation = rotation;

		self.transformation = Mat4::from_scale_rotation_translation(
			self.scale, self.rotation, self.position
		)
	}

	/// Get the shape's rotation.
	pub fn get_rotation(&mut self) -> Quat {
		self.rotation
	}

	/// Set the shape's scale.
	pub fn set_scale(&mut self, scale: Vec3) {
		self.scale = scale;

		self.transformation = Mat4::from_scale_rotation_translation(
			self.scale, self.rotation, self.position
		)
	}

	/// Get the shape's scale.
	pub fn get_scale(&self) -> Vec3 {
		self.scale
	}

	/// Get the shape transformation matrix.
	pub fn get_transformation(&self) -> &Mat4 {
		&self.transformation
	}
}
