use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::Instant;
use streams::WriteStream;

use crate::handshake::{ Handshake, Version, };
use crate::network_stream::NetworkWriteStream;

#[derive(Debug)]
pub enum ClientError {
	SocketError(std::io::Error),
}

#[derive(Debug)]
pub struct Client {
	address: SocketAddr,
	/// The last time we received data from the server.
	last_activity: Instant,
	socket: UdpSocket,
}

impl Client {
	/// Initialize the client and connect to the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ClientError> {
		let socket = match UdpSocket::bind("[::]:0") {
			Ok(socket) => socket,
			Err(error) => return Err(ClientError::SocketError(error)),
		};

		if let Err(error) = socket.set_nonblocking(true) {
			return Err(ClientError::SocketError(error));
		}

		if let Err(error) = socket.connect(address) {
			return Err(ClientError::SocketError(error));
		}

		Ok(Client {
			address: socket.local_addr().unwrap(),
			last_activity: Instant::now(),
			socket,
		})
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
