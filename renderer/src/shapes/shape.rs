use glam::Vec3;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Mutex;

use crate::shapes;

lazy_static! {
	static ref NEXT_SHAPE_GUID: Mutex<u64> = Mutex::new(0);
}

#[derive(Debug)]
pub struct Shape {
	/// The animations playing on the shape.
	active_animations: Vec<shapes::AnimationContext>,
	blueprint: Rc<shapes::blueprint::Blueprint>,
	id: u64,
	pub position: Vec3,
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
			position: Vec3::default(),
		}
	}

	pub fn get_blueprint(&self) -> Rc<shapes::blueprint::Blueprint> {
		self.blueprint.clone()
	}

	/// Plays an animation based on the given animation context.
	pub fn play_animation(&mut self, animation: shapes::AnimationContext) {
		// do not play animations that would never play to begin with
		if animation.looping_behavior == shapes::AnimationIteration::Count(0) {
			return;
		}

		self.active_animations.push(animation);

		// sort animations by priority
		self.active_animations.sort_by(|a, b| b.blending.priority.cmp(&a.blending.priority));
	}

	/// Increments the timers for all playing animations.
	pub fn update_animation_timer(&mut self, deltatime: f32) {
		for animation in self.active_animations.iter_mut() {
			animation.update_timer(deltatime);
		}
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
}
