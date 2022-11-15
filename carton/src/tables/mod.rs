mod file_table;
mod string_table;

#[derive(Eq, PartialEq)]
pub(crate) enum TableID {
	FileTable 		= 1,
	StringTable 	= 2,
}

pub(crate) use file_table::FileTable;
pub(crate) use string_table::StringTable;
