pub mod client;
pub mod error;
pub mod handshake;
pub mod log;
pub mod network_stream;
pub mod payload;
pub mod server;

/// Maximum size of a normal packet. Has to stay below 1500 bytes because MTUs are low on the internet. Set to 1400
/// since headers count towards the 1500 byte limit. Others seem to pick a number closer to 1450 bytes, but I'm picking
/// 1400 bytes to stay safe. If we send packets larger than 1400 bytes, then our packets will be fragmented and possibly
/// dropped.
const MAX_PACKET_SIZE: usize = 1400;

pub use client::client::Client;
pub use server::server::Server;
