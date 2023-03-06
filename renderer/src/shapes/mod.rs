mod batch_parameters;
pub mod blueprint;
mod shape;
mod triangulator;

pub type BoneIndexType = u16;
pub type FloatType = f32;
pub type IndexType = u32;

pub(crate) use batch_parameters::BatchParameters;
pub(crate) use batch_parameters::BatchParametersKey;
pub use shape::Shape;
pub use triangulator::triangulator;
