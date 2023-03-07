use glam::Vec3;
use lazy_static::lazy_static;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Mutex;

use crate::shapes;

lazy_static! {
	static ref NEXT_SHAPE_GUID: Mutex<u64> = Mutex::new(0);
}

#[derive(Debug)]
pub struct Shape {
	pub bones: Vec<glam::Mat4>,
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

		let mut bones = Vec::new();
		for bone in blueprint.get_bones().iter() {
			bones.push(bone.transform);
		}

		Shape {
			bones,
			blueprint,
			id,
			position: Vec3::default(),
		}
	}

	pub fn get_blueprint(&self) -> Rc<shapes::blueprint::Blueprint> {
		self.blueprint.clone()
	}

	pub fn update_bones(&self) {

	}
}
