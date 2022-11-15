mod file_table;
mod string_table;

#[derive(Eq, PartialEq)]
pub(crate) enum TableID {
	Invalid 			= 0,
	FileTable 		= 1,
	StringTable 	= 2,
}

impl From<u8> for TableID {
	fn from(table_id: u8) -> Self {
		match table_id {
			1 => TableID::FileTable,
			2 => TableID::StringTable,
			_ => TableID::Invalid,
		}
	}
}

pub(crate) use file_table::FileTable;
pub(crate) use string_table::StringTable;
