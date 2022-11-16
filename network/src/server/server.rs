use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };

#[derive(Debug)]
pub enum ServerError {
	SocketError(std::io::Error),
}

#[derive(Debug)]
pub struct Server {
	address: SocketAddr,
	socket: UdpSocket,
}

impl Server {
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, ServerError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(ServerError::SocketError(error)),
		};

		socket.set_nonblocking(true).unwrap();

		Ok(Server {
			address: socket.local_addr().unwrap(),
			socket,
		})
	}
}
