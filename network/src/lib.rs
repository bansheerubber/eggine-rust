pub mod client;
pub mod handshake;
pub mod network_stream;
pub mod payload;
pub mod server;

/// Maximum eggine packet size.
const MAX_PACKET_SIZE: usize = (2 as usize).pow(12);

pub use client::client::Client;
pub use server::server::Server;
