pub mod server;
pub mod times;
pub mod times_shift_register;

pub use times::Times;
pub use times_shift_register::TimesShiftRegister;

pub const MAX_NTP_PACKET_SIZE: usize = 53;
pub const NTP_MAGIC_NUMBER: &str = "EGGINENTP";
