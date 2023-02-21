use glam::Vec3;
use std::rc::Rc;

use crate::{ shape, textures, };

#[derive(Debug)]
pub struct Shape {
	pub blueprint: Rc<shape::Blueprint>,
	pub position: Vec3,
	texture: Option<Rc<textures::Texture>>,
}

impl Shape {
	pub fn new(blueprint: Rc<shape::Blueprint>) -> Self {
		Shape {
			blueprint,
			position: Vec3::default(),
			texture: None,
		}
	}

	pub fn set_texture(&mut self, texture: Option<Rc<textures::Texture>>) {
		self.texture = texture;
	}
}
