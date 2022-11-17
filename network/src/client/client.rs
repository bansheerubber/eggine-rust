use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::Instant;
use streams::{ ReadStream, WriteStream, };

use crate::MAX_PACKET_SIZE;
use crate::handshake::{ Handshake, Version, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };

#[derive(Debug)]
pub enum ClientError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
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
	receive_buffer: Vec<u8>,
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

		// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
		// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
		// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
		// `0..MAX_PACKET_SIZE`.
		let mut receive_buffer = Vec::new();
		receive_buffer.resize(MAX_PACKET_SIZE + 1, 0);

		Ok(Client {
			address: socket.local_addr().unwrap(),
			last_activity: Instant::now(),
			receive_buffer,
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

		let buffer = self.disown_receive_buffer();
		self.receive_stream.import(buffer).unwrap();

		println!("{:?}", self.receive_stream);

		Ok(())
	}

	/// Create a new receive buffer, returning the old one.
	fn disown_receive_buffer(&mut self) -> Vec<u8> {
		let mut new_vector = Vec::new();
		new_vector.resize(MAX_PACKET_SIZE + 1, 0);
		std::mem::replace(&mut self.receive_buffer, new_vector)
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
