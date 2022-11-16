use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };

#[derive(Debug)]
pub enum ClientError {
	SocketError(std::io::Error),
}

#[derive(Debug)]
pub struct Client {
	address: SocketAddr,
	socket: UdpSocket,
}

impl Client {
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
			socket,
		})
	}
}
