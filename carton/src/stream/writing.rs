pub fn write_u8(byte: u8, vector: &mut Vec<u8>) {
	vector.push(byte);
}

pub fn write_char(byte: char, vector: &mut Vec<u8>) {
	vector.push(byte as u8);
}

/// Writes 2 bytes in little-endian format
pub fn write_u16(number: u16, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..2 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes 4 bytes in little-endian format
pub fn write_u32(number: u32, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..4 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes 8 bytes in little-endian format
pub fn write_u64(number: u64, vector: &mut Vec<u8>) {
	let mut shift = number;
	for _ in 0..8 {
		write_u8((shift & 0xFF) as u8, vector);
		shift >>= 8;
	}
}

/// Writes a variable length quantity integer. The 16th bit in a 2 byte pair represents if the number has another two
/// bits. 1 if there are, 0 if there aren't.
pub fn write_vlq(id: u64, vector: &mut Vec<u8>) {
	let mut shift = id;
	for _ in 0..4 {
		let number = if shift >> 15 != 0 {
			(shift as u16 & 0x7FFF) | 0x8000
		} else {
			shift as u16 & 0x7FFF
		};

		write_u16(number, vector);

		shift >>= 15;

		if shift == 0 {
			break;
		}
	}
}

/// Strings are length encoded, with a variable length integer representing the length. Strings can have up to 2**60
/// characters.
pub fn write_string(string: &str, vector: &mut Vec<u8>) {
	write_vlq(string.len() as u64, vector);

	for character in string.chars() {
		vector.push(character as u8);
	}
}
