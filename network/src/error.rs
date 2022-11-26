use std::any::Any;

pub trait NetworkStreamErrorTrait: std::fmt::Debug {
	fn as_any(&self) -> &dyn Any;
}

/// Network stream error, for use in generics. In order to interact with the network stream subsystem, stream trait
/// implementations must use this error generic.
pub type NetworkStreamError = Box<dyn NetworkStreamErrorTrait + 'static>;
