use std::net::SocketAddr;
use std::time::Instant;

/// Server representation of a connected client.
#[derive(Debug)]
pub struct ClientConnection {
	pub(crate) address: SocketAddr,
	/// The last time we received information from the client.
	pub(crate) last_activity: Instant,
}
