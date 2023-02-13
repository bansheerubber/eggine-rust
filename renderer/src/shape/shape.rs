use glam::Vec3;
use std::rc::Rc;

use crate::ShapeBlueprint;

pub struct Shape {
	blueprint: Rc<ShapeBlueprint>,
	position: Vec3
}

impl Shape {
	pub fn render() {

	}
}
