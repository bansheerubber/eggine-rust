use rand::Rng;
use std::net::{ Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Duration, Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::error::NetworkStreamError;
use crate::handshake::{ Handshake, Version, };
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ AcknowledgeMask, DisconnectionReason, Packet, SubPayload, };
use crate::MAX_PACKET_SIZE;

use crate::ntp::{ NtpServer, NtpServerError, };
use super::{ ClientConnection, ClientTable, };

#[derive(Debug)]
pub enum ServerError {
	/// Emitted if we receive data from a blacklisted IP. Non-fatal.
	Blacklisted(SocketAddr),
	/// Emitted if we encountered a problem during client connection creation. Non-fatal.
	ClientCreation,
	/// Emitted if we could not find a `ClientConnection` associated with a `SocketAddr`
	CouldNotFindClient,
	/// Emitted if we could not convert a `SourceAddr` into an `Ipv6Addr` during a receive call. Non-fatal.
	InvalidIP,
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
	/// Wrapper for an error from the server NTP implementation
	NtpError(NtpServerError),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig(SocketAddr),
	/// Emitted if we encountered an OS error during a socket operation.
	Socket(std::io::ErrorKind),
}

impl ServerError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match self {
			ServerError::Blacklisted(_) => false,
			ServerError::ClientCreation => false,
			ServerError::CouldNotFindClient => false,
			ServerError::InvalidIP => false,
			ServerError::NetworkStreamError(_) => false,
			ServerError::NtpError(error) => {
				if let NtpServerError::Socket(error) = error {
					match error {
						tokio::io::ErrorKind::AddrInUse => true,
						tokio::io::ErrorKind::AddrNotAvailable => true,
						_ => false,
					}
				} else {
					false
				}
			},
			ServerError::PacketTooBig(_) => false,
			ServerError::Socket(_) => true,
		}
	}
}

impl From<NetworkStreamError> for ServerError {
	fn from(error: NetworkStreamError) -> Self {
		ServerError::NetworkStreamError(error)
	}
}

impl From<NtpServerError> for ServerError {
	fn from(error: NtpServerError) -> Self {
		ServerError::NtpError(error)
	}
}

impl From<std::io::Error> for ServerError {
	fn from(error: std::io::Error) -> Self {
		ServerError::Socket(error.kind())
	}
}

/// Represents a server host in the eggine network stack. Serves many clients, which are considered untrusted sources of
/// information. The two communicate using a packet format built upon the streams library.
#[derive(Debug)]
pub struct Server {
	client_table: ClientTable,
	/// Handshake we compare client handshakes against.
	handshake: Handshake,
	log: Log,
	ntp_server: NtpServer,
	/// The buffer we write into when we receive data.
	receive_buffer: [u8; MAX_PACKET_SIZE + 1],
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	/// The stream we use to export data so we can sent it to a client.
	send_stream: NetworkWriteStream,
	/// The socket the server is being hosted on.
	socket: UdpSocket,
}

impl Server {
	/// Initialize the server and listen on the specified address.
	pub async fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ServerError> {
		let socket = UdpSocket::bind(address)?;

		socket.set_nonblocking(true)?;

		let mut ntp_address = socket.local_addr()?;
		ntp_address.set_port(ntp_address.port() + 1);

		Ok(Server {
			client_table: ClientTable::default(),
			handshake: Handshake {
				checksum: [0; 16],
				ntp_id: 0,
				sequences: (0, 0),
				version: Version {
					branch: String::from("master"),
					major: 0,
					minor: 0,
					revision: 0,
				},
			},
			log: Log::default(),
			ntp_server: NtpServer::new(ntp_address, None).await?,
			// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
			// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
			// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
			// `0..MAX_PACKET_SIZE`.
			receive_buffer: [0; MAX_PACKET_SIZE + 1],
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			socket,
		})
	}

	/// Perform all necessary network functions for this tick. This includes receiving data, sending data, and figuring
	/// out all `ClientConnection`s' time-to-live.
	pub async fn tick(&mut self) -> Result<(), ServerError> {
		{
			// send client outgoing packets
			// TODO make this a little more efficient, right now this sucks b/c i can't use a &self ref in the for loop source
			let sources = self.client_table.client_iter().map(|(source, _)| *source).collect::<Vec<SocketAddr>>();
			let mut reset_sources = Vec::new();
			for source in sources {
				let client = self.client_table.get_client_mut(&source)?;
				if client.outgoing_packet.get_sub_payloads().len() > 0 { // only send packets if we have information to send
					client.sequence += 1;
					client.outgoing_packet.prepare(
						client.acknowledge_mask,
						client.sequence,
						client.last_sequence_received.unwrap_or(0)
					);

					self.send_stream.encode(&client.outgoing_packet)?;

					let bytes = self.send_stream.export()?;
					self.send_bytes_to(source, &bytes)?;
					reset_sources.push(source);
				}
			}

			// reset client outgoing packets so we can write new information into them
			for source in reset_sources {
				let client = self.client_table.get_client_mut(&source)?;
				client.outgoing_packet.next();
			}
		}

		// receive packets
		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					if let ServerError::Socket(std::io::ErrorKind::WouldBlock) = error {
						break;
					} else if error.is_fatal() {
						return Err(error);
					}
				},
			}
		}

		// find clients that have lived for too long
		{
			let now = Instant::now();
			let time_out_clients = self.client_table.client_iter()
				.filter_map(|(_, client)| {
					if now - client.last_activity > Duration::from_secs(30) {
						Some(client.address)
					} else {
						None
					}
				})
				.collect::<Vec<SocketAddr>>();

			// force disconnects on timed out clients
			for source in time_out_clients {
				self.log.print(LogLevel::Error, format!("{:?} timed out", source), 0);
				self.disconnect_client(source, DisconnectionReason::Timeout)?;
			}
		}

		// send NTP packets to all clients
		for (address, _) in self.client_table.client_iter() {
			let mut ntp_address = address.clone();
			ntp_address.set_port(ntp_address.port() + 1);

			if let Err(error) = self.ntp_server.sync_time(Some(ntp_address)).await {
				if error.is_fatal() {
					return Err(error.into());
				}
			}
		}

		// process NTP packets
		self.ntp_server.process_all().await?;

		Ok(())
	}

	/// Attempt to receive data from the socket.
	fn recv(&mut self) -> Result<(), ServerError> {
		let (read_bytes, source) = self.socket.recv_from(&mut self.receive_buffer)?;

		// convert the `SocketAddr` into a `Ipv6Addr`. `Ipv6Addr`s do not contain the port the client connected from, the
		// lack of which is required for the blacklist implementation
		let address = if let SocketAddr::V6(address) = source {
			address.ip().clone()
		} else {
			return Err(ServerError::InvalidIP);
		};

		// stop blacklisted data from continuing
		if self.client_table.is_in_blacklist(&address) {
			return Err(ServerError::Blacklisted(source));
		}

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			self.log.print(LogLevel::Blacklist, format!("received too big of a packet from {:?}", source), 0);
			self.client_table.add_to_blacklist(address.clone());
			return Err(ServerError::PacketTooBig(source));
		}

		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&self.receive_buffer[0..read_bytes]);

		// now we're done error checking, figure out what to do with the data we just got
		if self.client_table.has_client(&source) {
			self.decode_packet(source, buffer)?;
		} else {
			self.initialize_client(source, &address, buffer)?;
		}

		Ok(())
	}

	// Ping a client at the specified address.
	pub fn ping(&mut self, source: SocketAddr) -> Result<(), ServerError> {
		let client = self.client_table.get_client_mut(&source)?;
		client.outgoing_packet.add_sub_payload(SubPayload::Ping(
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
		));

		Ok(())
	}

	/// Decode a packet from an already connected IP address.
	fn decode_packet(&mut self, source: SocketAddr, buffer: Vec<u8>) -> Result<(), ServerError> {
		let now = Instant::now();
		let client = self.client_table.get_client_mut(&source)?;
		client.last_activity = now;

		let last_ping_time = client.last_ping_time;

		self.receive_stream.import(buffer)?;
		let packet = match self.receive_stream.decode::<Packet>() {
			Ok((packet, _)) => packet,
			Err(error) => {
				self.log.print(LogLevel::Error, format!("could not decode packet from {:?} for {:?}", source, error), 0);
				return Err(error.into());
			},
		};

		let result = packet.handle_sequences(
			client.highest_acknowledge_received,
			client.last_sequence_received,
			client.acknowledge_mask
		);

		client.acknowledge_mask = result.new_acknowledge_mask;
		client.last_sequence_received = Some(result.remote_sequence);
		client.highest_acknowledge_received = Some(result.new_highest_acknowledged_sequence);

		// handle sub-payloads
		for sub_payload in packet.get_sub_payloads() {
			match sub_payload {
				SubPayload::Disconnect(reason) => {
					self.log.print(LogLevel::Info, format!("got disconnect from {:?} for reason {:?}", source, reason), 0);
					self.disconnect_client(source, DisconnectionReason::Requested)?;
					return Ok(()); // stop processing sub payloads now, the connection is now closed
				},
				SubPayload::Ping(time) => {
					self.log.print(LogLevel::Info, format!("got ping with time {}", time), 0);

					// send a pong to the client
					client.outgoing_packet.add_sub_payload(SubPayload::Pong(
						SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
					));
				},
				SubPayload::Pong(time) => {
					self.log.print(
						LogLevel::Info,
						format!(
							"got pong with time {} from {:?}. round-trip duration is {}us",
							time,
							source,
							(now - last_ping_time).as_micros()
						),
						0
					);
				}
			}
		}

		Ok(())
	}

	/// Send a byte vector to the specified client address.
	fn send_bytes_to(&mut self, source: SocketAddr, bytes: &Vec<u8>) -> Result<(), ServerError> {
		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		self.socket.send_to(&bytes, source)?;
		Ok(())
	}

	/// Attempt to initialize a connection with a new IP address who just talked to us. Test for a handshake, and make
	/// sure the handshake is compatible with the server's handshake.
	fn initialize_client(&mut self, source: SocketAddr, address: &Ipv6Addr, handshake_buffer: Vec<u8>)
		-> Result<(), ServerError>
	{
		self.receive_stream.import(handshake_buffer)?;

		self.log.print(LogLevel::Info, format!("client talking from {:?}", source), 0);

		// decode handshake
		let handshake = match self.receive_stream.decode::<Handshake>() {
			Ok((handshake, _)) => handshake,
			Err(error) => {
				self.log.print(LogLevel::Error, format!("could not decode handshake from {:?} for {:?}", source, error), 0);
				return Err(error.into());
			},
		};

		// check handshake
		if !self.handshake.is_compatible(&handshake) {
			self.log.print(
				LogLevel::Blacklist, format!("invalid handshake, theirs: {:?}, ours: {:?}", handshake, self.handshake), 1
			);
			self.client_table.add_to_blacklist(address.clone());
			return Err(ServerError::ClientCreation);
		}

		// figure out if they already joined on this ip/port
		if self.client_table.has_client(&source) {
			self.log.print(LogLevel::Error, format!("already connected"), 1);
			return Err(ServerError::ClientCreation);
		}

		// we're home free, add the client to the client list
		let sequence = 500;
		let their_sequence = 1000;

		let their_ntp_id = rand::thread_rng().gen::<u32>();

		self.log.print(LogLevel::Info, format!("established client connection successfully"), 1);
		self.client_table.add_client(source, ClientConnection {
			acknowledge_mask: AcknowledgeMask::default(),
			address: source,
			ntp_id_client: their_ntp_id,
			highest_acknowledge_received: Some(sequence),
			last_activity: Instant::now(),
			last_ping_time: Instant::now(),
			last_sequence_received: None,
			outgoing_packet: Packet::new(sequence, 0),
			sequence,
		});

		// add the client to the NTP server whitelist so they can get accurate times
		self.ntp_server.associate_host_id(their_ntp_id, handshake.ntp_id);

		self.handshake.sequences = (sequence, their_sequence);
		self.handshake.ntp_id = their_ntp_id; // tell the client to use this ID

		// send our handshake to the client
		self.send_stream.encode(&self.handshake)?;

		let bytes = self.send_stream.export()?;
		self.send_bytes_to(source, &bytes)?;

		Ok(())
	}

	/// Disconnect a client from the server. Removes the `ClientConnection` associated with the source address from the
	/// server connection state. Tell the client that their connection has been closed.
	fn disconnect_client(&mut self, source: SocketAddr, reason: DisconnectionReason) -> Result<(), ServerError> {
		let client = self.client_table.get_client_mut(&source)?;
		client.outgoing_packet.add_sub_payload(SubPayload::Disconnect(reason));
		self.send_stream.encode(&client.outgoing_packet)?;

		self.log.print(LogLevel::Info, format!("disconnected client with reason {:?}", reason), 0);

		// remove the client to the NTP server whitelist
		self.ntp_server.disconnect_id(source, client.ntp_id_client);

		// remove the client before we have a chance of erroring out during the send
		self.client_table.remove_client(&source);

		let bytes = self.send_stream.export()?;
		self.send_bytes_to(source, &bytes)?;

		Ok(())
	}
}
