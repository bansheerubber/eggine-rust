use std::net::SocketAddr;
use std::time::Instant;

use crate::payload::{ AcknowledgeMask, Packet, };

/// Server representation of a connected client.
#[derive(Debug)]
pub struct ClientConnection {
	/// Server-side acknowledge mask for this client.
	pub(crate) acknowledge_mask: AcknowledgeMask,
	/// Address of the client.
	pub(crate) address: SocketAddr,
	/// The highest sequence number that the client said it had acknowledged. This is initialized as `None`, since the
	/// client starts off having acknowledged nothing.
	pub(crate) highest_acknowledge_received: Option<u32>,
	/// The last time we received information from the client.
	pub(crate) last_activity: Instant,
	/// The last time we went a ping to the client.
	pub(crate) last_ping_time: Instant,
	/// The last sequence we received from the client.
	pub(crate) last_sequence_received: Option<u32>,
	/// The NTP id that the client uses to talk to us.
	pub(crate) ntp_id_client: u32,
	/// Each client has a new packet assigned to them every server tick. All information that needs to be sent that tick
	/// should be encoded into the client's outgoing packet. Clients can only be sent one packet per tick.
	pub(crate) outgoing_packet: Packet,
	/// The server-side sequence number. Server -> client packets will be identified using this sequence number.
	pub(crate) sequence: u32,
}
