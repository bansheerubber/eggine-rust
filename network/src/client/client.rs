use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::MAX_PACKET_SIZE;
use crate::handshake::{ Handshake, Version, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ DisconnectionReason, Packet, SubPayload, };

#[derive(Debug)]
pub enum ClientError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
	/// Emitted if we were disconnected by the server. Fatal.
	Disconnected(DisconnectionReason),
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
			ClientError::Disconnected(_) => true,
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
	/// The last time we received data from the server.
	last_activity: Instant,
	receive_buffer: [u8; MAX_PACKET_SIZE],
	receive_stream: NetworkReadStream,
	send_stream: NetworkWriteStream,
	socket: UdpSocket,
}

impl Client {
	/// Initialize the client and connect to the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ClientError> {
		let socket = match UdpSocket::bind("[::]:0") {
			Ok(socket) => socket,
			Err(error) => return Err(ClientError::Create(error)),
		};

		if let Err(error) = socket.set_nonblocking(true) {
			return Err(ClientError::Create(error));
		}

		if let Err(error) = socket.connect(address) {
			return Err(ClientError::Create(error));
		}

		let mut receive_buffer = Vec::new();
		receive_buffer.resize(MAX_PACKET_SIZE + 1, 0);

		Ok(Client {
			address: socket.local_addr().unwrap(),
			last_activity: Instant::now(),
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
			println!("! received too big of a packet");
			return Err(ClientError::PacketTooBig);
		}

		self.last_activity = Instant::now();

		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&self.receive_buffer[0..read_bytes]);
		self.receive_stream.import(buffer).unwrap();

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<Packet>();
		for sub_payload in packet.get_sub_payloads() {
			match sub_payload {
				SubPayload::Disconnect(reason) => {
					println!(". server told us to disconnect with reason {:?}", reason);
					return Err(ClientError::Disconnected(*reason));
				},
				SubPayload::Ping(time) => {
					println!(". got ping with time {}", time);
					self.test_encode_packet().unwrap();
				},
				SubPayload::Pong(time) => {
					println!(". got pong with time {}", time);
				}
			}
		}

		Ok(())
	}

	fn test_encode_packet(&mut self) -> Result<(), ClientError> {
		let mut packet = Packet::new(0, 0);
		packet.add_sub_payload(SubPayload::Pong(
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
		));

		self.send_stream.encode(&packet);

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		let bytes = self.send_stream.export().unwrap();
		if let Err(error) = self.socket.send(&bytes) {
			return Err(ClientError::Send(error));
		}

		Ok(())
	}

	pub fn test_send(&mut self) {
		let mut stream = NetworkWriteStream::new();

		let handshake = Handshake {
			checksum: [0; 16],
			version: Version {
				branch: String::from("master"),
				major: 0,
				minor: 0,
				revision: 0,
			}
		};

		stream.encode(&handshake);

		if !stream.can_export() {
			panic!("Could not export");
		} else {
			self.socket.send(&mut stream.export().unwrap()).unwrap();
		}
	}
}
