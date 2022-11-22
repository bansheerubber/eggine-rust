use std::fmt::{ Debug, Write, };
use streams::{ Decode, Encode, Endable, ReadStream, StreamPosition, WriteStream, };
use streams::u8_io::{ U8ReadStream, U8ReadStringSafeStream, U8WriteStream, };

use crate::error::NetworkStreamError;

const ACKNOWLEDGE_MASK_SIZE: usize = 2;

#[derive(Clone, Copy, Default)]
pub struct AcknowledgeMask {
	mask: [u64; ACKNOWLEDGE_MASK_SIZE],
}

impl AcknowledgeMask {
	/// Shifts the acknowledge mask by the specified amount
	pub fn shift(&mut self, amount: u32) {
		if amount == 0 {
			return;
		}

		let capped_amount = if amount >= 63 {
			63
		} else {
			amount
		};

		let mut carry = 0;
		for i in 0..ACKNOWLEDGE_MASK_SIZE {
			let temp_carry = self.mask[i] & (0xFFFF_FFFF_FFFF_FFFF << (64 - capped_amount));
			self.mask[i] <<= capped_amount;
			self.mask[i] |= carry >> (64 - capped_amount);
			carry = temp_carry;
		}

		// it becomes non-trivial to do shifts with an amount > 63. good thing we can split amount > 63 shifts into
		// multiple shift calls, all with a range of amount <= 63
		self.shift(amount - capped_amount);
	}

	/// Sets the first bit to 1.
	pub fn set_first(&mut self) {
		self.mask[0] |= 1;
	}

	/// Tests a bit.
	pub fn test(&self, bit: u32) -> Option<bool> {
		if bit as usize >= ACKNOWLEDGE_MASK_SIZE * 64 {
			None
		} else {
			Some((self.mask[(bit / 64) as usize] & (1 << (bit % 64))) != 0)
		}
	}
}

impl Debug for AcknowledgeMask {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// f.debug_struct("AcknowledgeMask").field("mask", &self.mask).finish()

		f.write_char('[')?;
		for i in (0..ACKNOWLEDGE_MASK_SIZE).rev() {
			for bit in (0..64).rev() {
				let character = if (self.mask[i] & (1 << bit)) != 0 {
					'1'
				} else {
					'0'
				};

				f.write_char(character)?;
			}
		}

		f.write_char(']')
	}
}

impl<T> Encode<u8, T, NetworkStreamError> for AcknowledgeMask
where
	T: WriteStream<u8, NetworkStreamError> + U8WriteStream<NetworkStreamError>
{
	fn encode(&self, stream: &mut T) -> Result<(), NetworkStreamError> {
		for part in self.mask {
			stream.write_u64(part)?;
		}

		Ok(())
	}
}

impl<T> Decode<u8, T, NetworkStreamError> for AcknowledgeMask
where
	T: ReadStream<u8, NetworkStreamError> + U8ReadStream<NetworkStreamError> + U8ReadStringSafeStream<NetworkStreamError> + Endable<NetworkStreamError>
{
	fn decode(stream: &mut T) -> Result<(Self, StreamPosition), NetworkStreamError> {
		let mut mask = [0; ACKNOWLEDGE_MASK_SIZE];

		let mut position = 0;
		for i in 0..ACKNOWLEDGE_MASK_SIZE {
			let (number, new_position) = stream.read_u64()?;
			position = new_position;
			mask[i] = number;
		}

		Ok((AcknowledgeMask {
			mask,
		}, position))
	}
}
