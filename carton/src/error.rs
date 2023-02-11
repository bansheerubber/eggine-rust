use std::fmt::Debug;
use std::string::FromUtf8Error;

#[derive(Debug)]
pub enum CartonError {
	DecodedFileNotFound,
	FileError(std::io::Error),
	FileNotOpen,
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
