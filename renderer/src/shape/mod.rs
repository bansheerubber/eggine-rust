mod batch_parameters;
mod blueprint;
pub mod blueprint2;
mod shape;
mod triangulator;

pub type FloatType = f32;
pub type IndexType = u32;

pub(crate) use batch_parameters::BatchParameters;
pub(crate) use batch_parameters::BatchParametersKey;
pub use blueprint::Blueprint;
pub use blueprint::BlueprintDataKind;
pub use blueprint::BlueprintError;
pub use blueprint::BlueprintState;
pub use blueprint::Mesh;
pub use shape::Shape;
pub use triangulator::triangulator;
