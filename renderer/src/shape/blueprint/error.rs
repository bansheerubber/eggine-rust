#[derive(Debug)]
pub enum Error {
	/// Returned if a an error occured while attempting a carton operation.
	CartonError(carton::Error),
	/// Returned if a primitive has no indices.
	NoIndices,
}
