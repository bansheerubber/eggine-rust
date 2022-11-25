pub mod client;
pub mod times;
pub mod times_shift_register;

pub use client::NtpClient;
pub use client::NtpClientError;
pub use times::Times;
pub use times_shift_register::TimesShiftRegister;
