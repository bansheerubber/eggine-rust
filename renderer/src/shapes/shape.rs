use glam::Vec3;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Mutex;

use crate::shapes;

lazy_static! {
	static ref NEXT_SHAPE_GUID: Mutex<u64> = Mutex::new(0);
}

#[derive(Debug)]
pub struct Shape {
	/// Index of the animtaion being played on the shape.
	active_animation: Option<usize>,
	/// The current animation time in seconds.
	animation_timer: f32,
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
			active_animation: None,
			animation_timer: 0.0,
			blueprint,
			id,
			position: Vec3::default(),
		}
	}

	pub fn get_blueprint(&self) -> Rc<shapes::blueprint::Blueprint> {
		self.blueprint.clone()
	}

	/// Looks up an animation by the specified name and then plays it.
	pub fn play_animation_by_name(&mut self, name: &str) {
		let mut index = 0;
		for animation in self.blueprint.get_animations() {
			if animation.get_name() == name {
				self.active_animation = Some(index);
				self.animation_timer = 0.0;
				break;
			}

			index += 1;
		}
	}

	/// Increments the shape's animation timer.
	pub fn update_animation_timer(&mut self, increment: f32) {
		self.animation_timer += increment;
	}

	/// Calculates a bone's global (relative to bone's root parent) transformation matrix based on the shape's animation
	/// state.
	pub fn get_bone_matrix(
		&self,
		bone: &Rc<RefCell<shapes::blueprint::Node>>,
		inverse_bind_matrix: &glam::Mat4,
		inverse_transform: &glam::Mat4
	) -> glam::Mat4 {
		let bone = bone.borrow();

		if let Some(animation_index) = self.active_animation {
			let Some(animation) = self.blueprint.get_animation(animation_index) else {
				return inverse_transform.mul_mat4(&bone.transform.mul_mat4(inverse_bind_matrix));
			};

			// animations store location transformations, so we need to figure out the global transformation by accumulating
			// together the bone's parent transforms
			let mut accumulator = animation.transform_bone(bone.gltf_id, self.animation_timer);
			let mut next = bone.parent.clone();
			loop {
				if let Some(temp) = next {
					let parent_transform = animation.transform_bone(temp.borrow().gltf_id, self.animation_timer);
					accumulator = parent_transform.mul_mat4(&accumulator);
					next = temp.borrow().parent.clone();
				} else {
					break;
				}
			}

			inverse_transform.mul_mat4(&&accumulator.mul_mat4(inverse_bind_matrix))
		} else {
			inverse_transform.mul_mat4(&bone.transform.mul_mat4(inverse_bind_matrix))
		}
	}
}
