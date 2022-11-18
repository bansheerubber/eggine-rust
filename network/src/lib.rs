pub mod client;
pub mod handshake;
pub mod log;
pub mod network_stream;
pub mod payload;
pub mod server;

/// Maximum size of a normal packet. Packets can be extended up to 2**16 bytes, but require special header flags to be
/// set.
const MAX_PACKET_SIZE: usize = (2 as usize).pow(12);

/// Maximum size of a mega packet.
const MAX_MEGA_PACKET_SIZE: usize = (2 as usize).pow(16);

pub use client::client::Client;
pub use server::server::Server;
