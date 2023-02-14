use glam::Vec3;
use std::rc::Rc;

use crate::shape;

#[derive(Debug)]
pub struct Shape {
	pub blueprint: Rc<shape::Blueprint>,
	pub position: Vec3
}

impl Shape {
	pub fn new(blueprint: Rc<shape::Blueprint>) -> Self {
		Shape {
			blueprint,
			position: Vec3::default(),
		}
	}
}
