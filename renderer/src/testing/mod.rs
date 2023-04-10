/// Implements some components of the renderer for testing purposes as the renderer crate is slowly fleshed out.
mod batch;
mod indirect_pass;
mod indirect_pass2;
mod uniforms;

pub(crate) use batch::Batch;
pub use indirect_pass::IndirectPass;
pub use uniforms::GlobalUniform;
