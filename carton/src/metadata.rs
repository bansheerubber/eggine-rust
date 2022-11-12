use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use toml::Value;

/// Emitted when a `.toml` metadata file cannot be parsed.
#[derive(Debug)]
pub enum FileMetadataError {
	/// Could not load the metadata file
	FileError,
	/// Metadata file did not have the .toml extension
	IncorrectExtension,
	/// Metadata file TOML contents could not be deserialized
	ParseError(toml::de::Error),
}

/// Represents metadata for a file stored in a carton. All metadata has a corresponding file it describes. Metadata is
/// interpreted from a TOML file that has the same name as the file it describes with the `.toml` extension appended.
#[derive(Debug)]
pub struct FileMetadata {
	/// File that the `FileMetadata`'s metadata describes.
	file_name: String,
	/// Indexable metadata contents
	values: FileMetadataTOML,
}

impl FileMetadata {
	/// Parse a TOML metadata file into a `FileMetadata` struct. A subset of TOML is supported, specifically string
	/// key/value pairs inside of the `values` table.
	///
	/// If parsing fails, the `FileMetadataError` structure can be interrogated to determine the `toml::de::Error` that
	/// was returned.
	pub fn read_file(file_name: &str) -> Result<FileMetadata, FileMetadataError> {
		let length = file_name.len();
		if &file_name[length - 5..] == ".toml" {
			let Ok(contents) = std::fs::read_to_string(file_name) else {
				return Err(FileMetadataError::FileError);
			};

			let values = match toml::from_str::<FileMetadataTOML>(&contents) {
				Ok(values) => values,
				Err(error) => return Err(FileMetadataError::ParseError(error)),
			};

			Ok(FileMetadata {
				file_name: file_name[0..length - 5].to_string(),
				values,
			})
		} else {
			Err(FileMetadataError::IncorrectExtension)
		}
	}
}

/// Represents the data found in a `.toml` metadata file.
#[derive(Debug, Deserialize, Serialize)]
struct FileMetadataTOML {
	/// String key/value pairs from the `values` table.
	values: HashMap<String, Value>,
}
