use std::net::SocketAddr;
use std::time::Instant;

use crate::payload::Packet;

/// Server representation of a connected client.
#[derive(Debug)]
pub struct ClientConnection {
	pub(crate) address: SocketAddr,
	/// The last time we received information from the client.
	pub(crate) last_activity: Instant,
	/// The last time we went a ping to the client.
	pub(crate) last_ping_time: Instant,
	/// Each client has a new packet assigned to them every server tick. All information that needs to be sent that tick
	/// should be encoded into the client's outgoing packet. Clients can only be sent one packet per tick.
	pub(crate) outgoing_packet: Packet,
}
