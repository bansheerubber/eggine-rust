use std::path::Path;

use crate::metadata::FileMetadata;

/// Represents a file in a carton.
#[derive(Debug)]
pub struct File {
	/// The filename taken from the input file structure during encoding.
	file_name: String,
	/// The metadata for this file.
	metadata: Option<FileMetadata>,
}

impl File {
	pub fn from_file(file_name: &str) -> File {
		let metadata = if Path::new(&format!("{}.toml", file_name)).exists() {
			Some(
				FileMetadata::from_file(&format!("{}.toml", file_name)).unwrap()
			)
		} else {
			None
		};

		File {
			file_name: String::from(file_name),
			metadata,
		}
	}
}
