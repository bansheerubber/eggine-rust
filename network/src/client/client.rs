use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::error::{ BoxedNetworkError, NetworkError, };
use crate::handshake::{ Handshake, Version, };
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ AcknowledgeMask, DisconnectionReason, Packet, SubPayload, };
use crate::MAX_PACKET_SIZE;

use super::ntp_client::NtpClient;

#[derive(Debug)]
pub enum ClientError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
	/// Emitted if we encountered a problem connecting to a server socket. Fatal.
	Connect(std::io::Error),
	/// Emitted if we were disconnected by the server. Fatal.
	Disconnected(DisconnectionReason),
	/// Received an invalid handshake. We likely talked to a random UDP server. Fatal.
	Handshake,
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig,
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(std::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(std::io::Error),
	/// Emitted if a socket call would block. With the non-blocking flag set, this indicates that we have consumed all
	/// available packets from the socket at the moment. Non-fatal.
	WouldBlock,
}

impl ClientError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			ClientError::Create(_) => true,
			ClientError::Connect(_) => true,
			ClientError::Disconnected(_) => true,
			ClientError::Handshake => true,
			ClientError::PacketTooBig => false,
			ClientError::Receive(_) => true,
			ClientError::Send(_) => true,
			ClientError::WouldBlock => false,
		}
	}
}

impl NetworkError for ClientError {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	fn as_debug(&self) -> &dyn std::fmt::Debug {
		self
	}
}

impl From<ClientError> for BoxedNetworkError {
	fn from(error: ClientError) -> Self {
		Box::new(error)
	}
}

/// A client connection in the eggine network stack. Clients connect to servers, which are treated as a trusted source
/// of information. The two communicate using a packet format built upon the streams library.
#[derive(Debug)]
pub struct Client {
	/// Acknowledge mask for this client.
	acknowledge_mask: AcknowledgeMask,
	address: SocketAddr,
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
	ntp_client: Option<NtpClient>,
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
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, BoxedNetworkError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(ClientError::Create(error).into()),
		};

		if let Err(error) = socket.set_nonblocking(true) {
			return Err(ClientError::Create(error).into());
		}

		Ok(Client {
			acknowledge_mask: AcknowledgeMask::default(),
			address: socket.local_addr().unwrap(),
			connection_initialized: false,
			handshake: Handshake {
				checksum: [0; 16],
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
			ntp_client: None,
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
	pub fn tick(&mut self) -> Result<(), BoxedNetworkError> {
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

		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					if let Some(error2) = error.as_any().downcast_ref::<ClientError>() {
						if error2.is_fatal() {
							return Err(error);
						} else if let ClientError::WouldBlock = error2 {
							break;
						}
					} else {
						return Err(error);
					}
				},
			}
		}

		Ok(())
	}

	/// Initializes a connection with the specified server. Done by sending a handshake, and receiving a sequence ID pair
	/// used for exchanging packets.
	pub fn initialize_connection<T: ToSocketAddrs>(&mut self, address: T) -> Result<(), BoxedNetworkError> {
		if let Err(error) = self.socket.connect(address) {
			return Err(ClientError::Connect(error).into());
		}

		self.log.print(LogLevel::Info, format!("establishing connection to {:?}...", self.socket.peer_addr().unwrap()), 0);
		self.send_stream.encode(&self.handshake)?;
		let bytes = self.send_stream.export()?;

		self.send_bytes(&bytes)?;

		let mut bind_address = self.socket.local_addr().unwrap();
		bind_address.set_port(bind_address.port() + 1);

		let mut host_address = self.socket.peer_addr().unwrap();
		host_address.set_port(host_address.port() + 1);

		self.ntp_client = Some(NtpClient::new(bind_address, host_address)?);

		Ok(())
	}

	pub fn is_connection_valid(&self) -> bool {
		self.connection_initialized
	}

	/// Ping the server.
	pub fn ping(&mut self) -> Result<(), BoxedNetworkError> {
		self.ntp_client.as_mut().unwrap().sync_time()?;

		if !self.is_connection_valid() {
			return Ok(());
		}

		self.outgoing_packet.add_sub_payload(SubPayload::Ping(
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
		));

		Ok(())
	}

	/// Attempt to receive data from the socket.
	fn recv(&mut self) -> Result<(), BoxedNetworkError> {
		let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				if error.raw_os_error().unwrap() == 11 {
					return Err(ClientError::WouldBlock.into());
				} else {
					return Err(ClientError::Receive(error).into());
				}
			},
		};

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet"), 0);
			return Err(ClientError::PacketTooBig.into());
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
				return Err(ClientError::Handshake.into());
			}

			// set our sequence numbers
			self.last_sequence_received = Some(handshake.sequences.0);
			self.sequence = handshake.sequences.1;

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
					return Err(ClientError::Disconnected(*reason).into());
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
	fn send_bytes(&mut self, bytes: &Vec<u8>) -> Result<(), BoxedNetworkError> {
		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send(&bytes) {
			return Err(ClientError::Send(error).into());
		}

		Ok(())
	}
}
