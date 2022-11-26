pub mod server;

pub use server::NtpServer;
pub use server::NtpServerError;

pub const MAX_NTP_PACKET_SIZE: usize = 41;
pub const NTP_MAGIC_NUMBER: &str = "EGGINENTP";
