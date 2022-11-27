use rand::Rng;
use std::net::{ ToSocketAddrs, UdpSocket, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::error::NetworkStreamError;
use crate::handshake::{ Handshake, Version, };
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::ntp::{NtpServerError, NtpServer};
use crate::payload::{ AcknowledgeMask, DisconnectionReason, Packet, SubPayload, };
use crate::MAX_PACKET_SIZE;

#[derive(Debug)]
pub enum ClientError {
	/// Emitted if we could not talk to the server.
	ConnectionRefused,
	/// Emitted if we were disconnected by the server. Fatal.
	Disconnected(DisconnectionReason),
	/// Received an invalid handshake. We likely talked to a random UDP server. Fatal.
	Handshake,
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
	/// Wrapper for an error from the client NTP implementation
	NtpError(NtpServerError),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig,
	/// Emitted if we encountered an OS error during a socket operation.
	Socket(std::io::ErrorKind),
	/// Emitted if a socket call would block. With the non-blocking flag set, this indicates that we have consumed all
	/// available packets from the socket at the moment. Non-fatal.
	WouldBlock,
}

impl ClientError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match self {
			ClientError::ConnectionRefused => true,
			ClientError::Disconnected(_) => true,
			ClientError::Handshake => true,
			ClientError::NetworkStreamError(_) => false,
			ClientError::NtpError(error) => error.is_fatal(),
			ClientError::PacketTooBig => false,
			ClientError::Socket(_) => true,
			ClientError::WouldBlock => false,
		}
	}
}

impl From<NetworkStreamError> for ClientError {
	fn from(error: NetworkStreamError) -> Self {
		ClientError::NetworkStreamError(error)
	}
}

impl From<NtpServerError> for ClientError {
	fn from(error: NtpServerError) -> Self {
		ClientError::NtpError(error)
	}
}

impl From<std::io::Error> for ClientError {
	fn from(error: std::io::Error) -> Self {
		ClientError::Socket(error.kind())
	}
}

/// A client connection in the eggine network stack. Clients connect to servers, which are treated as a trusted source
/// of information. The two communicate using a packet format built upon the streams library.
#[derive(Debug)]
pub struct Client {
	/// Acknowledge mask for this client.
	acknowledge_mask: AcknowledgeMask,
	/// True if the server accepted our handshake and we're in a state where we are ready to exchange packets.
	connection_initialized: bool,
	/// The handshake we send to the server upon connection initialization.
	handshake: Handshake,
	/// The highest sequence number that the server said it had acknowledged. This is initialized as `None`, since the
	/// server starts off having acknowledged nothing.
	highest_acknowledge_received: Option<u32>,
	/// The last time we received data from the server.
	last_activity: Instant,
	/// The last sequence we received from the server.
	last_sequence_received: Option<u32>,
	log: Log,
	/// The NTP id that we use to talk to the server.
	ntp_id_client: u32,
	/// The NTP id that the server uses to talk to us.
	ntp_id_server: u32,
	ntp_server: Option<NtpServer>,
	/// We place all outgoing data into this packet.
	outgoing_packet: Packet,
	/// The buffer we write into when we receive data.
	receive_buffer: [u8; MAX_PACKET_SIZE + 1],
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	/// The stream we use to export data so we can sent it to a client.
	send_stream: NetworkWriteStream,
	/// The client-side sequence number. Client -> server packets will be identified using this sequence number.
	sequence: u32,
	/// The socket the client is connected to the server on.
	socket: UdpSocket,
}

impl Client {
	/// Initialize the a client socket bound to the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ClientError> {
		let socket = UdpSocket::bind(address)?;
		socket.set_nonblocking(true)?;

		let ntp_id_server = rand::thread_rng().gen::<u32>();

		Ok(Client {
			acknowledge_mask: AcknowledgeMask::default(),
			connection_initialized: false,
			handshake: Handshake {
				checksum: [0; 16],
				ntp_id: ntp_id_server,
				sequences: (0, 0),
				version: Version {
					branch: String::from("master"),
					major: 0,
					minor: 0,
					revision: 0,
				}
			},
			highest_acknowledge_received: None,
			last_activity: Instant::now(),
			last_sequence_received: None,
			log: Log::default(),
			ntp_id_client: 0,
			ntp_id_server,
			ntp_server: None,
			outgoing_packet: Packet::new(0, 0),
			// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
			// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
			// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
			// `0..MAX_PACKET_SIZE`.
			receive_buffer: [0; MAX_PACKET_SIZE + 1],
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			sequence: 0,
			socket,
		})
	}

	/// Perform all necessary network functions for this tick. This includes receiving data, sending data, and figuring
	/// out our time-to-live.
	pub async fn tick(&mut self) -> Result<(), ClientError> {
		// send the packet we worked on constructing to the server, then reset it
		if self.outgoing_packet.get_sub_payloads().len() > 0 && self.is_connection_valid() {
			self.sequence += 1;
			self.outgoing_packet.prepare(
				self.acknowledge_mask,
				self.sequence,
				self.last_sequence_received.unwrap_or(0)
			);

			self.send_stream.encode(&self.outgoing_packet)?;

			let bytes = self.send_stream.export()?;
			self.send_bytes(&bytes)?;

			self.outgoing_packet.next();
		}

		// receive packets from the server
		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					if let ClientError::Socket(std::io::ErrorKind::WouldBlock) = error {
						break;
					} else if error.is_fatal() {
						return Err(error);
					}
				},
			}
		}

		// process NTP packets from the server
		if self.is_connection_valid() && self.ntp_server.is_some() {
			let ntp_server = self.ntp_server.as_mut().unwrap();
			ntp_server.sync_time(None).await?;
			ntp_server.process_all().await?;
		}

		Ok(())
	}

	/// Initializes a connection with the specified server. Done by sending a handshake, and receiving a sequence ID pair
	/// used for exchanging packets.
	pub async fn initialize_connection<T: ToSocketAddrs>(&mut self, address: T) -> Result<(), ClientError> {
		self.socket.connect(address)?;

		self.log.print(LogLevel::Info, format!("establishing connection to {:?}...", self.socket.peer_addr().unwrap()), 0);
		self.send_stream.encode(&self.handshake)?;
		let bytes = self.send_stream.export()?;

		self.send_bytes(&bytes)?;

		let mut bind_address = self.socket.local_addr().unwrap();
		bind_address.set_port(bind_address.port() + 1);

		let mut host_address = self.socket.peer_addr().unwrap();
		host_address.set_port(host_address.port() + 1);

		self.ntp_server = Some(NtpServer::new(bind_address, Some(host_address)).await?);
		self.ntp_server.as_mut().unwrap().address_to_id.insert(host_address, self.ntp_id_server);

		Ok(())
	}

	pub fn is_connection_valid(&self) -> bool {
		self.connection_initialized
	}

	/// Ping the server.
	pub fn ping(&mut self) -> Result<(), ClientError> {
		if !self.is_connection_valid() {
			return Ok(());
		}

		self.outgoing_packet.add_sub_payload(SubPayload::Ping(
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
		));

		Ok(())
	}

	/// Attempt to receive data from the socket.
	fn recv(&mut self) -> Result<(), ClientError> {
		let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				return Err(ClientError::Socket(error.kind()));
			},
		};

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet"), 0);
			return Err(ClientError::PacketTooBig);
		}

		self.last_activity = Instant::now();

		// import raw bytes into the receive stream
		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&self.receive_buffer[0..read_bytes]);
		self.receive_stream.import(buffer)?;

		// if the connection has not been initialized yet, we need to check if the server sent us back a handshake
		if !self.connection_initialized {
			// check handshake
			let handshake = self.receive_stream.decode::<Handshake>()?.0;
			if !self.handshake.is_compatible(&handshake) {
				self.log.print(
					LogLevel::Error, format!("invalid handshake, theirs: {:?}, our: {:?}", handshake, self.handshake), 1
				);
				return Err(ClientError::Handshake);
			}

			// set our sequence numbers
			self.last_sequence_received = Some(handshake.sequences.0);
			self.sequence = handshake.sequences.1;
			self.ntp_id_client = handshake.ntp_id;
			self.ntp_server.as_mut().unwrap().id_to_host_id.insert(self.ntp_id_server, self.ntp_id_client);

			self.log.print(LogLevel::Info, format!("connection established"), 0);
			self.connection_initialized = true;

			return Ok(());
		}

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<Packet>()?.0;

		let result = packet.handle_sequences(
			self.highest_acknowledge_received,
			self.last_sequence_received,
			self.acknowledge_mask
		);

		self.acknowledge_mask = result.new_acknowledge_mask;
		self.last_sequence_received = Some(result.remote_sequence);
		self.highest_acknowledge_received = Some(result.new_highest_acknowledged_sequence);

		for sub_payload in packet.get_sub_payloads() {
			match sub_payload {
				SubPayload::Disconnect(reason) => {
					self.log.print(LogLevel::Info, format!("server told us to disconnect with reason {:?}", reason), 0);
					return Err(ClientError::Disconnected(*reason));
				},
				SubPayload::Ping(time) => {
					self.log.print(LogLevel::Info, format!("got ping with time {}", time), 0);

					// send a pong to the server
					self.outgoing_packet.add_sub_payload(SubPayload::Pong(
						SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
					));
				},
				SubPayload::Pong(time) => {
					self.log.print(LogLevel::Info, format!("got pong with time {}", time), 0);
				}
			}
		}

		Ok(())
	}

	/// Send a byte vector to the server.
	fn send_bytes(&mut self, bytes: &Vec<u8>) -> Result<(), ClientError> {
		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		match self.socket.send(&bytes) {
			Ok(_) => Ok(()),
			Err(error) => {
				Err(ClientError::Socket(error.kind()))
			},
		}
	}
}
