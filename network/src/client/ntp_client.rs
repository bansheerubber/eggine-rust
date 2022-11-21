use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::error::{ BoxedNetworkError, NetworkError, };
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ NtpClientPacket, NtpServerPacket, };
use crate::server::ntp_server::{ NTP_MAGIC_NUMBER, MAX_NTP_PACKET_SIZE, };
use crate::MAX_PACKET_SIZE;

#[derive(Debug)]
pub enum NtpClientError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
	/// Emitted if we encountered a problem connecting to a server socket. Fatal.
	Connect(std::io::Error),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig,
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(std::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(std::io::Error),
}

impl NtpClientError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			NtpClientError::Create(_) => true,
			NtpClientError::Connect(_) => true,
			NtpClientError::PacketTooBig => false,
			NtpClientError::Receive(_) => true,
			NtpClientError::Send(_) => true,
		}
	}
}

impl NetworkError for NtpClientError {
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

impl From<NtpClientError> for BoxedNetworkError {
	fn from(error: NtpClientError) -> Self {
		Box::new(error)
	}
}

#[derive(Debug)]
pub struct Times {
	client_receive_time: u128,
	client_send_time: u128,
	server_receive_time: u128,
	server_send_time: u128,
}

/// A client connection in the eggine network stack. Clients connect to servers, which are treated as a trusted source
/// of information. The two communicate using a packet format built upon the streams library.
#[derive(Debug)]
pub struct NtpClient {
	address: SocketAddr,
	log: Log,
	/// The buffer we write into when we receive data.
	receive_buffer: [u8; MAX_NTP_PACKET_SIZE + 1],
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	/// The stream we use to export data so we can sent it to a client.
	send_stream: NetworkWriteStream,
	/// The socket the client is connected to the server on.
	socket: UdpSocket,
	times: Times,
}

impl NtpClient {
	/// Initialize the a client socket bound to the specified address.
	pub fn new<T: ToSocketAddrs>(address: T) -> Result<Self, BoxedNetworkError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(NtpClientError::Create(error).into()),
		};

		if let Err(error) = socket.set_nonblocking(true) {
			return Err(NtpClientError::Create(error).into());
		}

		let mut receive_buffer = Vec::new();
		receive_buffer.resize(MAX_PACKET_SIZE + 1, 0);

		Ok(NtpClient {
			address: socket.local_addr().unwrap(),
			log: Log::default(),
			// create the receive buffer. if we ever receive a packet that is greater than `MAX_PACKET_SIZE`, then the recv
			// function call will say that we have read `MAX_PACKET_SIZE + 1` bytes. the extra read byte allows us to check
			// if a packet is too big to decode, while also allowing us to use all the packet bytes within the range
			// `0..MAX_PACKET_SIZE`.
			receive_buffer: [0; MAX_NTP_PACKET_SIZE + 1],
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			socket,
			times: Times {
				client_receive_time: 0,
				client_send_time: 0,
				server_receive_time: 0,
				server_send_time: 0,
			},
		})
	}

	pub fn recv_loop(&mut self) -> Result<(), BoxedNetworkError> {
		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					if let Some(error2) = error.as_any().downcast_ref::<NtpClientError>() {
						if error2.is_fatal() {
							return Err(error);
						}
					} else {
						return Err(error);
					}
				},
			}
		}
	}

	pub fn send(&mut self) -> Result<(), BoxedNetworkError> {
		let packet = NtpClientPacket {
			magic_number: String::from(NTP_MAGIC_NUMBER),
		};

		self.send_stream.encode(&packet)?;

		self.times.client_send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u128;

		if let Err(error) = self.socket.send(&self.send_stream.export()?) {
			return Err(NtpClientError::Send(error).into());
		}

		Ok(())
	}

	fn recv(&mut self) -> Result<(), BoxedNetworkError> {
		let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				return Err(NtpClientError::Receive(error).into());
			},
		};

		let recv_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u128;

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet"), 0);
			return Err(NtpClientError::PacketTooBig.into());
		}

		// import raw bytes into the receive stream
		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&self.receive_buffer[0..read_bytes]);
		self.receive_stream.import(buffer)?;

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<NtpServerPacket>()?.0;

		self.times.server_receive_time = packet.receive_time;
		self.times.server_send_time = packet.send_time;

		self.times.client_receive_time = recv_time;

		Ok(())
	}
}
