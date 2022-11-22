use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };

use crate::error::NetworkStreamError;
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
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
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
			NtpClientError::NetworkStreamError(_) => false,
			NtpClientError::PacketTooBig => false,
			NtpClientError::Receive(_) => true,
			NtpClientError::Send(_) => true,
		}
	}
}

impl From<NetworkStreamError> for NtpClientError {
	fn from(error: NetworkStreamError) -> Self {
		NtpClientError::NetworkStreamError(error)
	}
}

pub struct CorrectedTime {
	offset: i128,
	system_time: i128,
}

impl CorrectedTime {
	pub fn new(system_time: i128, offset: i128) -> Self {
		CorrectedTime {
			offset,
			system_time,
		}
	}

	pub fn get_offset(&self) -> i128 {
		self.offset
	}

	pub fn get_time(&self) -> i128 {
		self.system_time + self.offset
	}
}

impl Into<i128> for CorrectedTime {
	fn into(self) -> i128 {
		self.system_time + self.offset
	}
}

impl Into<u128> for CorrectedTime {
	fn into(self) -> u128 {
		self.system_time as u128 + self.offset as u128
	}
}

/// Used for client-sided time adjustments after syncing to the server's clock. All times are in microseconds. Based on
/// NTP (https://en.wikipedia.org/wiki/Network_Time_Protocol#Clock_synchronization_algorithm).
#[derive(Debug)]
pub struct Times {
	/// The time we received the server's answer.
	client_receive_time: i128,
	/// The time we sent our time request to the server.
	client_send_time: i128,
	/// The time the server received our time request.
	server_receive_time: i128,
	/// The time the server sent its response to us.
	server_send_time: i128,
}

impl Times {
	/// The time between the client sending a time request and receiving the server's response. Based on NTP's round-trip
	/// delay calculation.
	pub fn round_trip(&self) -> i128 {
		(self.client_receive_time - self.client_send_time) - (self.server_send_time - self.server_receive_time)
	}

	/// Calculate the difference in absolute time between the client and server times. Based on NTP's time offset
	/// calculation.
	pub fn time_offset(&self) -> i128 {
		((self.server_receive_time - self.client_send_time) + (self.server_send_time - self.client_receive_time)) / 2
	}

	/// The amount of time the server spent processing the client's packet.
	pub fn server_processing(&self) -> i128 {
		self.server_send_time - self.server_receive_time
	}

	/// Estimates the time it took for the client's packet to reach the server.
	pub fn estimate_first_leg(&self) -> i128 {
		self.server_receive_time - self.client_send_time
	}

	/// Estimates the time it took for the server's packet to reach the client.
	pub fn estimate_second_leg(&self) -> i128 {
		self.client_receive_time - self.server_send_time
	}
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
	pub fn new<T: ToSocketAddrs>(address: T, host_address: T) -> Result<Self, NtpClientError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(NtpClientError::Create(error)),
		};

		if let Err(error) = socket.connect(host_address) {
			return Err(NtpClientError::Create(error));
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

	pub fn sync_time(&mut self) -> Result<(), NtpClientError> {
		// send the time request
		let send_time;
		{
			let packet = NtpClientPacket {
				magic_number: String::from(NTP_MAGIC_NUMBER),
			};

			self.send_stream.encode(&packet)?;

			send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

			if let Err(error) = self.socket.send(&self.send_stream.export()?) {
				return Err(NtpClientError::Send(error));
			}
		}

		// receive the time
		{
			let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
				Ok(a) => a,
				Err(error) => {
					return Err(NtpClientError::Receive(error));
				},
			};

			let recv_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

			// make sure what we just read is not too big to be an eggine packet
			if read_bytes > MAX_PACKET_SIZE {
				self.log.print(LogLevel::Error, format!("received too big of a packet"), 0);
				return Err(NtpClientError::PacketTooBig);
			}

			// import raw bytes into the receive stream
			// TODO optimize this
			let mut buffer: Vec<u8> = Vec::new();
			buffer.extend(&self.receive_buffer[0..read_bytes]);
			self.receive_stream.import(buffer)?;

			// figure out what to do with the packet we just got
			let packet = self.receive_stream.decode::<NtpServerPacket>()?.0;

			self.times.client_receive_time = recv_time;
			self.times.client_send_time = send_time;

			self.times.server_receive_time = packet.receive_time;
			self.times.server_send_time = packet.send_time;

			println!("time offset: {}us", self.times.time_offset());
			println!("round-trip: {}us", self.times.round_trip());
		}

		Ok(())
	}
}
