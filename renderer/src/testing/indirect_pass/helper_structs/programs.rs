use std::collections::HashMap;
use std::rc::Rc;

use crate::shaders::{ ComputeProgram, Program, };
use crate::testing::indirect_pass::ObjectUniform;

/// Stores program related information used by the pass object.
#[derive(Debug)]
pub(crate) struct Programs {
	pub(crate) bone_uniforms: HashMap<u64, Vec<glam::Mat4>>,
	pub(crate) composite_program: Rc<Program>,
	pub(crate) depth_pyramid_bind_group_layout: wgpu::BindGroupLayout,
	pub(crate) depth_pyramid_pipeline_layout: wgpu::PipelineLayout,
	pub(crate) depth_pyramid_program: Rc<ComputeProgram>,
	pub(crate) g_buffer_program: Rc<Program>,
	pub(crate) object_uniforms: HashMap<u64, Vec<ObjectUniform>>,
	pub(crate) prepass_program: Rc<Program>,
}
