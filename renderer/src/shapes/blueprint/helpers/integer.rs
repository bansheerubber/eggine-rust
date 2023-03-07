/// Converts between two sizes of little-endian serialized integer. Zero-extends if the destination size is larger,
/// truncates if the destination size is smaller.
pub fn convert_integer(buffer: &[u8], out: &mut [u8], source_size: usize, destination_size: usize) -> u64 {
	let end = std::cmp::min(source_size, destination_size);
	out[0..end].clone_from_slice(&buffer[0..end]); // copy to output
	read_integer(out, end) // return what we just put in the array
}

/// Reads an integer from a buffer.
pub fn read_integer(buffer: &[u8], source_size: usize) -> u64 {
	match source_size {
		1 => buffer[0] as u64,
		2 => (buffer[1] as u64) << 8 | buffer[0] as u64,
		3 => (buffer[2] as u64) << 16 | (buffer[1] as u64) << 8 | buffer[0] as u64,
		4 => (buffer[3] as u64) << 24 | (buffer[2] as u64) << 16 | (buffer[1] as u64) << 8 | buffer[0] as u64,
		_ => panic!("size not supported"),
	}
}
