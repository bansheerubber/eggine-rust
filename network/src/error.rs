use std::any::Any;
use std::fmt::Debug;

pub trait NetworkError {
	fn as_any(&self) -> &dyn Any;
	fn as_debug(&self) -> &dyn Debug;
}

impl Debug for Box<dyn NetworkError> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let casted = self.as_debug();
		casted.fmt(f)
	}
}

/// Network stream error, for use in generics. In order to interact with the network stream subsystem, stream trait
/// implementations must use this error generic.
pub type BoxedNetworkError = Box<dyn NetworkError + 'static>;
