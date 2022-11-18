use std::fmt::Debug;
use std::string::FromUtf8Error;

#[derive(Debug)]
pub enum CartonError {
	FileError(std::io::Error),
	FromUtf8(FromUtf8Error),
	InvalidCompression,
	InvalidMagicNumber,
	InvalidTOMLType(u8),
	InvalidVersion,
	NoFile,
	NotInStringTable(u64),
	UnexpectedFileName,
	UnexpectedTable,
}

pub type Error = Box<dyn Debug + 'static>;
