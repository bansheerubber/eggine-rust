use std::net::{ Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Duration, Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::MAX_PACKET_SIZE;
use crate::handshake::{ Handshake, Version, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ DisconnectionReason, Packet, SubPayload, };

use super::{ ClientConnection, ClientTable, };

#[derive(Debug)]
pub enum ServerError {
	/// Emitted ff we receive data from a blacklisted IP. Non-fatal.
	Blacklisted(SocketAddr),
	/// Emitted if we encountered a problem during client connection creation. Non-fatal.
	ClientCreation,
	/// Emitted if we could not find a `ClientConnection` associated with a `SocketAddr`
	CouldNotFindClient,
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
	/// Emitted if we could not convert a `SourceAddr` into an `Ipv6Addr` during a receive call. Non-fatal.
	InvalidIP,
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig(SocketAddr),
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(std::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(std::io::Error),
	/// Emitted if a socket call would block. With the non-blocking flag set, this indicates that we have consumed all
	/// available packets from the socket at the moment. Non-fatal.
	WouldBlock,
}

impl ServerError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			ServerError::Blacklisted(_) => false,
			ServerError::ClientCreation => false,
			ServerError::CouldNotFindClient => false,
			ServerError::Create(_) => true,
			ServerError::InvalidIP => false,
			ServerError::PacketTooBig(_) => false,
			ServerError::Receive(_) => true,
			ServerError::Send(_) => true,
			ServerError::WouldBlock => false,
		}
	}
}

#[derive(Debug)]
pub struct Server {
	/// The address the server is bound to
	address: SocketAddr,
	client_table: ClientTable,
	/// Handshae we compare client handshakes against.
	handshake: Handshake,
	/// The buffer we write into when we receive data.
	receive_buffer: [u8; MAX_PACKET_SIZE],
	receive_stream: NetworkReadStream,
	send_stream: NetworkWriteStream,
	socket: UdpSocket,
}

pub enum ReceiveResult {
	None,
}

impl Server {
	/// Initialize the server and listen on the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ServerError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(ServerError::Create(error)),
		};

		socket.set_nonblocking(true).unwrap();

		Ok(Server {
			address: socket.local_addr().unwrap(),
			client_table: ClientTable::default(),
			handshake: Handshake {
				checksum: [0; 16],
				sequences: (0, 0),
				version: Version {
					branch: String::from("master"),
					major: 0,
					minor: 0,
					revision: 0,
				},
			},
			// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
			// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
			// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
			// `0..MAX_PACKET_SIZE`.
			receive_buffer: [0; MAX_PACKET_SIZE],
			receive_stream: NetworkReadStream::new(),
			socket,
			send_stream: NetworkWriteStream::new(),
		})
	}

	/// Perform all necessary network functions for this tick. This includes receiving data, sending data, and figuring
	/// out all `ClientConnection`s' time-to-live.
	pub fn tick(&mut self) -> Result<(), ServerError> {
		{
			// send client outgoing packets
			// TODO make this a little more efficient, right now this sucks b/c i can't use a &self ref in the for loop source
			let sources = self.client_table.client_iter().map(|(source, _)| *source).collect::<Vec<SocketAddr>>();
			for source in sources {
				let client = self.client_table.get_client(&source)?;
				self.send_stream.encode(&client.outgoing_packet);

				let bytes = self.send_stream.export().unwrap();
				self.send_bytes_to(source, &bytes)?;
			}

			// reset client outgoing packets so we can write new information into them
			for (_, client) in self.client_table.client_iter_mut() {
				client.outgoing_packet = Packet::new(0, 0); // TODO add reset function to packet
			}
		}

		// receive packets
		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					if error.is_fatal() {
						return Err(error);
					} else if let ServerError::WouldBlock = error {
						break;
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
				println!("! {:?} timed out", source);
				if let Err(error) = self.disconnect_client(source, DisconnectionReason::Timeout) {
					if let ServerError::CouldNotFindClient = error {
						unreachable!();
					} else {
						return Err(error);
					}
				}
			}
		}

		Ok(())
	}

	/// Attempt to receive data from the socket.
	fn recv(&mut self) -> Result<ReceiveResult, ServerError> {
		let (read_bytes, source) = match self.socket.recv_from(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				if error.raw_os_error().unwrap() == 11 {
					return Err(ServerError::WouldBlock);
				} else {
					return Err(ServerError::Receive(error));
				}
			},
		};

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
			println!("@ received too big of a packet from {:?}", address); // @ indicates that the ip was blacklisted for this
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

			let client = self.client_table.get_client_mut(&source)?;
			client.outgoing_packet.add_sub_payload(SubPayload::Ping(
				SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
			));
		}

		Ok(ReceiveResult::None)
	}

	/// Decode a packet from an already connected IP address.
	fn decode_packet(&mut self, source: SocketAddr, buffer: Vec<u8>) -> Result<(), ServerError> {
		let now = Instant::now();
		let client = self.client_table.get_client_mut(&source)?;
		client.last_activity = now;

		let last_ping_time = client.last_ping_time;

		self.receive_stream.import(buffer).unwrap();
		let packet = self.receive_stream.decode::<Packet>();
		for sub_payload in packet.get_sub_payloads() {
			match sub_payload {
				SubPayload::Disconnect(reason) => {
					println!(". got disconnect from {:?} for reason {:?}", source, reason);
					self.disconnect_client(source, DisconnectionReason::Requested)?;
					return Ok(()); // stop processing sub payloads now, the connection is now closed
				},
				SubPayload::Ping(time) => {
					println!(". got ping with time {}", time);
				},
				SubPayload::Pong(time) => {
					println!(
						". got pong with time {} from {:?}. round-trip duration is {}us",
						time,
						source,
						(now - last_ping_time).as_micros()
					);
				}
			}
		}

		Ok(())
	}

	/// Send a byte vector to the specified client address.
	fn send_bytes_to(&mut self, source: SocketAddr, bytes: &Vec<u8>) -> Result<(), ServerError> {
		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&bytes, source) {
			return Err(ServerError::Send(error));
		}

		Ok(())
	}

	/// Attempt to initialize a connection with a new IP address who just talked to us. Test for a handshake, and make
	/// sure the handshake is compatible with the server's handshake.
	fn initialize_client(&mut self, source: SocketAddr, address: &Ipv6Addr, handshake_buffer: Vec<u8>)
		-> Result<(), ServerError>
	{
		self.receive_stream.import(handshake_buffer).unwrap();

		println!(". client talking from {:?}", source);

		// check handshake
		let handshake = self.receive_stream.decode::<Handshake>();
		if handshake != self.handshake {
			println!("  @ invalid handshake"); // @ indicates that the ip was blacklisted for this
			self.client_table.add_to_blacklist(address.clone());
			return Err(ServerError::ClientCreation);
		}

		// figure out if they already joined on this ip/port
		if self.client_table.has_client(&source) {
			println!("  ! already connected");
			return Err(ServerError::ClientCreation);
		}

		// we're home free, add the client to the client list
		println!("  . initialized client connection successfully");
		self.client_table.add_client(source, ClientConnection {
			address: source,
			last_activity: Instant::now(),
			last_ping_time: Instant::now(),
			outgoing_packet: Packet::new(0, 0),
		});

		// send our handshake to the client
		self.send_stream.encode(&self.handshake);

		let bytes = self.send_stream.export().unwrap();
		self.send_bytes_to(source, &bytes)?;

		Ok(())
	}

	/// Disconnect a client from the server. Removes the `ClientConnection` associated with the source address from the
	/// server connection state. Tell the client that their connection has been closed.
	fn disconnect_client(&mut self, source: SocketAddr, reason: DisconnectionReason) -> Result<(), ServerError> {
		let client = self.client_table.get_client_mut(&source)?;
		client.outgoing_packet.add_sub_payload(SubPayload::Disconnect(reason));
		self.send_stream.encode(&client.outgoing_packet);

		println!(". disconnected client with reason {:?}", reason);

		self.client_table.remove_client(&source); // remove the client before we have a chance of erroring out during the send

		let bytes = self.send_stream.export().unwrap();
		self.send_bytes_to(source, &bytes)?;

		Ok(())
	}
}
