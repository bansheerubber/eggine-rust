/// Structs without implementations that are used to compose the `IndirectPass` object.
mod allocated_memory;
mod bind_groups;
mod programs;
mod render_textures;

pub(crate) use allocated_memory::AllocatedMemory;
pub(crate) use bind_groups::BindGroups;
pub(crate) use programs::Programs;
pub(crate) use render_textures::RenderTextures;
