use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::MAX_PACKET_SIZE;
use crate::handshake::{ Handshake, Version, };
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ DisconnectionReason, Packet, SubPayload, };

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

#[derive(Debug)]
pub struct Client {
	address: SocketAddr,
	/// True if the server accepted our handshake and we're in a state where we are ready to exchange packets.
	connection_initialized: bool,
	handshake: Handshake,
	/// The last time we received data from the server.
	last_activity: Instant,
	log: Log,
	outgoing_packet: Packet,
	receive_buffer: [u8; MAX_PACKET_SIZE],
	receive_stream: NetworkReadStream,
	send_stream: NetworkWriteStream,
	socket: UdpSocket,
}

impl Client {
	/// Initialize the a client socket bound to the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ClientError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(ClientError::Create(error)),
		};

		if let Err(error) = socket.set_nonblocking(true) {
			return Err(ClientError::Create(error));
		}

		let mut receive_buffer = Vec::new();
		receive_buffer.resize(MAX_PACKET_SIZE + 1, 0);

		Ok(Client {
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
			last_activity: Instant::now(),
			log: Log::default(),
			outgoing_packet: Packet::new(0, 0),
			// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
			// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
			// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
			// `0..MAX_PACKET_SIZE`.
			receive_buffer: [0; MAX_PACKET_SIZE],
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			socket,
		})
	}

	/// Perform all necessary network functions for this tick. This includes receiving data, sending data, and figuring
	/// out our time-to-live.
	pub fn tick(&mut self) -> Result<(), ClientError> {
		// send the packet we worked on constructing to the server, then reset it
		if self.outgoing_packet.get_sub_payloads().len() > 0 {
			self.send_stream.encode(&self.outgoing_packet);

			let bytes = self.send_stream.export().unwrap();
			self.send_bytes(&bytes)?;

			self.outgoing_packet = Packet::new(0, 0); // TODO improve packet reset API
		}

		// read a packet from the server
		let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				if error.raw_os_error().unwrap() == 11 {
					return Err(ClientError::WouldBlock);
				} else {
					return Err(ClientError::Receive(error));
				}
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
		self.receive_stream.import(buffer).unwrap();

		// if the connection has not been initialized yet, we need to check if the server sent us back a handshake
		if !self.connection_initialized {
			// check handshake
			let handshake = self.receive_stream.decode::<Handshake>();
			if handshake != self.handshake {
				self.log.print(LogLevel::Error, format!("invalid handshake"), 0);
				return Err(ClientError::Handshake);
			}

			self.log.print(LogLevel::Info, format!("connection established"), 0);
			self.connection_initialized = true;

			return Ok(());
		}

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<Packet>();
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

	/// Initializes a connection with the specified server. Done by sending a handshake, and receiving a sequence ID pair
	/// used for exchanging packets.
	pub fn initialize_connection<T: ToSocketAddrs>(&mut self, address: T) -> Result<(), ClientError> {
		if let Err(error) = self.socket.connect(address) {
			return Err(ClientError::Connect(error));
		}

		self.log.print(LogLevel::Info, format!("establishing connection to {:?}...", self.socket.peer_addr().unwrap()), 0);
		self.send_stream.encode(&self.handshake);
		let bytes = self.send_stream.export().unwrap();

		self.send_bytes(&bytes)?;

		Ok(())
	}

	pub fn is_connection_valid(&self) -> bool {
		self.connection_initialized
	}

	/// Send a byte vector to the server.
	fn send_bytes(&mut self, bytes: &Vec<u8>) -> Result<(), ClientError> {
		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send(&bytes) {
			return Err(ClientError::Send(error));
		}

		Ok(())
	}
}
