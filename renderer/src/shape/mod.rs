mod blueprint;
mod shape;
mod triangulator;

pub type IndexType = u32;

pub use blueprint::Blueprint;
pub use blueprint::BlueprintDataKind;
pub use blueprint::BlueprintError;
pub use blueprint::BlueprintState;
pub use blueprint::Mesh;
pub use shape::Shape;
pub use triangulator::triangulator;
