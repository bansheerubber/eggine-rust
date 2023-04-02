use carton::metadata::{ FileMetadata, MetadataType, };

#[derive(Debug)]
pub struct AnimationMetadata {
	pub end: i64,
	pub fps: i64,
	pub name: String,
	pub start: i64,
}

pub fn decode_animation_table(gltf_metadata: &Option<FileMetadata>) -> Vec<AnimationMetadata> {
	let Some(gltf_metadata) = gltf_metadata else { // silently fail if there is no metadata
		return Vec::new();
	};

	let table = gltf_metadata.get_value().as_table().expect("Could not unwrap metadata table");
	let Some(animation_table) = table.get("animation") else { // silently fail if there is no animation table
		return Vec::new();
	};

	let Some(animation_table) = animation_table.as_table() else {
		eprintln!("Expected animation table, got different type");
		return Vec::new();
	};

	let mut output = Vec::new();
	for (key, value) in animation_table.iter() {
		let Some(value) = value.as_table() else {
			eprintln!("Expected animation table entry under key '{}', got different type", key);
			continue;
		};

		if !FileMetadata::check_types(
			value, &[("start", MetadataType::Integer), ("end", MetadataType::Integer), ("fps", MetadataType::Integer)]
		) {
			eprintln!("Improperly formed animation table entry '{}'", key);
			continue;
		}

		output.push(AnimationMetadata {
			end: value.get("end").unwrap().as_integer().unwrap(),
			fps: value.get("fps").unwrap().as_integer().unwrap(),
			name: key.to_string(),
			start: value.get("start").unwrap().as_integer().unwrap(),
		});
	}

	return output
}
