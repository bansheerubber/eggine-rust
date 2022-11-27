pub mod server;
pub mod times;
pub mod ntp_statistics;

pub use server::NtpServer;
pub use server::NtpServerError;
pub use times::Times;
pub use ntp_statistics::NtpStatistics;

pub const MAX_NTP_PACKET_SIZE: usize = 57;
pub const NTP_MAGIC_NUMBER: &str = "EGGINENTP";
