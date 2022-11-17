use std::collections::{ HashMap, HashSet, };
use std::net::{ Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Duration, Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::MAX_PACKET_SIZE;
use crate::handshake::{ Handshake, Version, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ DisconnectionReason, Packet, SubPayload, };

use super::ClientConnection;

#[derive(Debug)]
pub enum ServerError {
	/// Emitted ff we receive data from a blacklisted IP. Non-fatal.
	Blacklisted(SocketAddr),
	/// Emitted if we encountered a problem during client connection creation. Non-fatal.
	ClientCreation,
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
	/// Maps IP address & port to a client.
	address_to_client: HashMap<SocketAddr, ClientConnection>,
	/// If we get too many invalid packets from an IP address, add them to the blacklist so we immediately discard any
	/// additional packets from them
	blacklist: HashSet<Ipv6Addr>,
	/// Handshae we compare client handshakes against.
	handshake: Handshake,
	/// The buffer we write into when we receive data.
	receive_buffer: Vec<u8>,
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

		// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
		// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
		// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
		// `0..MAX_PACKET_SIZE`.
		let mut receive_buffer = Vec::new();
		receive_buffer.resize(MAX_PACKET_SIZE + 1, 0);

		Ok(Server {
			address: socket.local_addr().unwrap(),
			address_to_client: HashMap::new(),
			blacklist: HashSet::new(),
			handshake: Handshake {
				checksum: [0; 16],
				version: Version {
					branch: String::from("master"),
					major: 0,
					minor: 0,
					revision: 0,
				},
			},
			receive_buffer,
			receive_stream: NetworkReadStream::new(),
			socket,
			send_stream: NetworkWriteStream::new(),
		})
	}

	/// Perform all necessary network functions for this tick. This includes receiving data, sending data, and figuring
	/// out all `ClientConnection`s' time-to-live.
	pub fn tick(&mut self) -> Result<(), ServerError> {
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

		// time out clients that have lived for too long
		let now = Instant::now();
		let time_out_clients = self.address_to_client.iter()
			.filter_map(|(_, client)| {
				if now - client.last_activity > Duration::from_secs(30) {
					Some(client.address)
				} else {
					None
				}
			})
			.collect::<Vec<SocketAddr>>();

		for address in time_out_clients {
			self.disconnect_client(address, DisconnectionReason::Timeout);
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
		if self.blacklist.contains(&address) {
			return Err(ServerError::Blacklisted(source));
		}

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			println!("@ received too big of a packet from {:?}", address); // @ indicates that the ip was blacklisted for this
			self.blacklist.insert(address.clone());
			return Err(ServerError::PacketTooBig(source));
		}

		// now we're done error checking, figure out what to do with the data we just got
		if self.address_to_client.contains_key(&source) {

		} else {
			let buffer = self.disown_receive_buffer();
			self.initialize_client(source, &address, buffer)?;

			self.test_encode_packet(source).unwrap();
		}

		Ok(ReceiveResult::None)
	}

	/// Decode a packet.
	fn decode_packet(&mut self, buffer: Vec<u8>) -> Result<(), ServerError> {
		Ok(())
	}

	fn test_encode_packet(&mut self, address: SocketAddr) -> Result<(), ServerError> {
		let mut packet = Packet::new(0, 0);
		packet.add_sub_payload(SubPayload::Ping(
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
		));

		self.send_stream.encode(&packet);

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&self.send_stream.export().unwrap(), address) {
			return Err(ServerError::Send(error));
		}

		Ok(())
	}

	/// Create a new receive buffer, returning the old one.
	fn disown_receive_buffer(&mut self) -> Vec<u8> {
		let mut new_vector = Vec::new();
		new_vector.resize(MAX_PACKET_SIZE + 1, 0);
		std::mem::replace(&mut self.receive_buffer, new_vector)
	}

	fn disconnect_client(&mut self, address: SocketAddr, reason: DisconnectionReason) {
		// TODO encode client disconnect
		println!("! {:?} timed out", address);
		self.address_to_client.remove(&address);
	}

	/// Attempt to initialize a connection with a client who just talked to us.
	fn initialize_client(&mut self, source: SocketAddr, address: &Ipv6Addr, handshake_buffer: Vec<u8>) -> Result<(), ServerError> {
		self.receive_stream.import(handshake_buffer).unwrap();

		println!("Client talking from {:?}", source);

		// check handshake
		let handshake = self.receive_stream.decode::<Handshake>();
		if handshake != self.handshake {
			println!("  @ invalid handshake"); // @ indicates that the ip was blacklisted for this
			self.blacklist.insert(address.clone());
			return Err(ServerError::ClientCreation);
		}

		// figure out if they already joined on this ip/port
		if self.address_to_client.contains_key(&source) {
			println!("  ! already connected");
			return Err(ServerError::ClientCreation);
		}

		// we're home free, add the client to the client list
		println!("  . initialized client connection successfully");
		self.address_to_client.insert(source, ClientConnection {
			address: source,
			last_activity: Instant::now(),
		});

		Ok(())
	}
}
