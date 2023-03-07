/// Converts between two sizes of little-endian serialized integer. Zero-extends if the destination size is larger,
/// truncates if the destination size is smaller.
pub fn convert_integer(buffer: &[u8], out: &mut Vec<u8>, source_size: usize, destination_size: usize) -> u64 {
	out.extend_from_slice(&buffer[0..std::cmp::min(source_size, destination_size)]); // copy to output

	if destination_size > source_size { // zero-extend
		for _ in 0..destination_size - source_size {
			out.push(0);
		}
	}

	// return what we just put in the array
	let number = match source_size {
		1 => buffer[0] as u64,
		2 => (buffer[1] as u64) << 8 | buffer[0] as u64,
		3 => (buffer[2] as u64) << 16 | (buffer[1] as u64) << 8 | buffer[0] as u64,
		4 => (buffer[3] as u64) << 24 | (buffer[2] as u64) << 16 | (buffer[1] as u64) << 8 | buffer[0] as u64,
		_ => panic!("size not supported"),
	};

	return number & (0xFF_FF_FF_FF >> ((4 - destination_size) * 8));
}
