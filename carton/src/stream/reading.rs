pub fn read_u8(vector: &[u8]) -> (u8, &[u8]) {
	(vector[0], &vector[1..])
}

pub fn read_char(vector: &[u8]) -> (char, &[u8]) {
	(vector[0] as char, &vector[1..])
}

/// Reads 2 bytes in little-endian format
pub fn read_u16(vector: &[u8]) -> (u16, &[u8]) {
	let mut vector = vector;
	let mut number = 0;
	for i in 0..2 {
		let (byte, new_position) = read_u8(vector);
		vector = new_position;

		number |= (byte as u16) << (i * 8);
	}
	return (number, vector);
}

/// Reads 4 bytes in little-endian format
pub fn read_u32(vector: &[u8]) -> (u32, &[u8]) {
	let mut vector = vector;
	let mut number = 0;
	for i in 0..4 {
		let (byte, new_position) = read_u8(vector);
		vector = new_position;

		number |= (byte as u32) << (i * 8);
	}
	return (number, vector);
}

/// Reads 8 bytes in little-endian format
pub fn read_u64(vector: &[u8]) -> (u64, &[u8]) {
	let mut vector = vector;
	let mut number = 0;
	for i in 0..8 {
		let (byte, new_position) = read_u8(vector);
		vector = new_position;

		number |= (byte as u64) << (i * 8);
	}
	return (number, vector);
}

/// Reads a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
/// bits. 1 if there are, 0 if there aren't.
pub fn read_vlq(vector: &[u8]) -> (u64, &[u8]) {
	let mut vector = vector;
	let mut number = 0;
	for i in 0..4 {
		let (bytes, new_position) = read_u16(vector);
		vector = new_position;

		number |= (bytes as u64 & 0x7FFF) << (i * 15);

		if bytes & 0x8000 == 0 {
			break;
		}
	}
	return (number, vector);
}

/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
/// characters.
pub fn read_string(vector: &[u8]) -> (String, &[u8]) {
	let mut vector = vector;
	let (length, new_position) = read_vlq(vector);
	vector = new_position;

	let mut output = String::new();
	for _ in 0..length {
		let (character, new_position) = read_char(vector);
		vector = new_position;

		output.push(character);
	}
	return (output, vector);
}
