use std::collections::HashSet;
use std::net::{ Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket, };
use std::sync::{ Arc, Mutex, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::ReadStream;

use crate::error::NetworkStreamError;
use crate::log::{ Log, LogLevel, };
use crate::network_stream::NetworkReadStream;
use crate::payload::NtpClientPacket;

pub const MAX_NTP_PACKET_SIZE: usize = 40;
pub const NTP_MAGIC_NUMBER: &str = "EGGINENTP";

#[derive(Debug)]
pub enum NtpServerError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(std::io::Error),
	/// Emitted if we could not convert a `SourceAddr` into an `Ipv6Addr` during a receive call. Non-fatal.
	InvalidIP,
	/// Emitted if the client did not send us the expected magic number.
	InvalidMagicNumber(SocketAddr),
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
	/// Emitted if we receive data from a non-whitelisted IP. Non-fatal.
	NotWhitelisted(SocketAddr),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig(SocketAddr),
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(std::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(std::io::Error),
}

impl NtpServerError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			NtpServerError::Create(_) => true,
			NtpServerError::InvalidIP => false,
			NtpServerError::InvalidMagicNumber(_) => false,
			NtpServerError::NetworkStreamError(_) => false,
			NtpServerError::NotWhitelisted(_) => false,
			NtpServerError::PacketTooBig(_) => false,
			NtpServerError::Receive(_) => true,
			NtpServerError::Send(_) => true,
		}
	}
}

impl From<NetworkStreamError> for NtpServerError {
	fn from(error: NetworkStreamError) -> Self {
		NtpServerError::NetworkStreamError(error)
	}
}

#[derive(Debug, Default)]
pub struct NtpServerWhitelist {
	/// The IPV6 addresses that are allowed to communicate with the NTP server.
	pub list: HashSet<Ipv6Addr>,
}

#[derive(Debug)]
pub struct NtpServer {
	/// The address the server is bound to
	address: SocketAddr,
	/// The IPV6 addresses that are allowed to communicate with the NTP server.
	address_whitelist: Arc<Mutex<NtpServerWhitelist>>,
	expected_client_packet: NtpClientPacket,
	log: Log,
	/// Amount of time it takes to read system time, in nanoseconds.
	precision: u64,
	/// The buffer we write into when we receive data.
	receive_buffer: [u8; MAX_NTP_PACKET_SIZE + 1],
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	socket: UdpSocket,
}

impl NtpServer {
	pub fn new<T: ToSocketAddrs>(
		address: T, address_whitelist: Arc<Mutex<NtpServerWhitelist>>
	) -> Result<Self, NtpServerError> {
		let socket = match UdpSocket::bind(address) {
			Ok(socket) => socket,
			Err(error) => return Err(NtpServerError::Create(error)),
		};

		// benchmark precision
		const BENCHMARK_TIMES: u128 = 10000;
		let mut total = 0;
		for _ in 0..BENCHMARK_TIMES {
			let start = Instant::now();
			#[allow(unused_must_use)] {
				SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros();
			}

			total += (Instant::now() - start).as_nanos();
		}

		Ok(NtpServer {
			address: socket.local_addr().unwrap(),
			address_whitelist,
			expected_client_packet: NtpClientPacket {
				magic_number: String::from(NTP_MAGIC_NUMBER),
			},
			precision: (total / BENCHMARK_TIMES) as u64,
			log: Log::default(),
			receive_buffer: [0; MAX_NTP_PACKET_SIZE + 1],
			receive_stream: NetworkReadStream::new(),
			socket,
		})
	}

	pub fn recv_loop(&mut self) -> Result<(), NtpServerError> {
		loop {
			match self.recv() {
				Ok(_) => {},
				Err(error) => {
					println!("{:?}", error);
					if error.is_fatal() {
						return Err(error);
					}
				},
			}
		}
	}

	fn recv(&mut self) -> Result<(), NtpServerError> {
		let (read_bytes, source) = match self.socket.recv_from(&mut self.receive_buffer) {
			Ok(a) => a,
			Err(error) => {
				return Err(NtpServerError::Receive(error));
			},
		};

		let recv_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

		// convert the `SocketAddr` into a `Ipv6Addr`. `Ipv6Addr`s do not contain the port the client connected from, the
		// lack of which is required for the blacklist implementation
		let address = if let SocketAddr::V6(address) = source {
			address.ip().clone()
		} else {
			return Err(NtpServerError::InvalidIP);
		};

		// stop non-whitelisted data from continuing
		if !self.address_whitelist.lock().unwrap().list.contains(&address) {
			return Err(NtpServerError::NotWhitelisted(source));
		}

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_NTP_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet from {:?}", source), 0);
			return Err(NtpServerError::PacketTooBig(source));
		}

		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&self.receive_buffer[0..read_bytes]);

		self.receive_stream.import(buffer)?;

		let client_packet = self.receive_stream.decode::<NtpClientPacket>()?.0;
		if client_packet != self.expected_client_packet {
			self.log.print(LogLevel::Error, format!("received invalid magic number from {:?}", source), 0);
			return Err(NtpServerError::InvalidMagicNumber(source));
		}

		// send the server time back
		let mut buffer: [u8; 40] = [0; 40];
		let send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

		// we need to send the times as quick as possible since the longer we take, the more inaccurate the `send_time` is
		// going to be. sends two 128 bit integers for receive time and send time.
		buffer[0] = (recv_time & 0xFF) as u8;
		buffer[1] = ((recv_time >> 8) & 0xFF) as u8;
		buffer[2] = ((recv_time >> 16) & 0xFF) as u8;
		buffer[3] = ((recv_time >> 24) & 0xFF) as u8;
		buffer[4] = ((recv_time >> 32) & 0xFF) as u8;
		buffer[5] = ((recv_time >> 40) & 0xFF) as u8;
		buffer[6] = ((recv_time >> 48) & 0xFF) as u8;
		buffer[7] = ((recv_time >> 56) & 0xFF) as u8;
		buffer[8] = ((recv_time >> 64) & 0xFF) as u8;
		buffer[9] = ((recv_time >> 72) & 0xFF) as u8;
		buffer[10] = ((recv_time >> 80) & 0xFF) as u8;
		buffer[11] = ((recv_time >> 88) & 0xFF) as u8;
		buffer[12] = ((recv_time >> 96) & 0xFF) as u8;
		buffer[13] = ((recv_time >> 104) & 0xFF) as u8;
		buffer[14] = ((recv_time >> 112) & 0xFF) as u8;
		buffer[15] = ((recv_time >> 120) & 0xFF) as u8;

		// send time encoding
		buffer[16] = (send_time & 0xFF) as u8;
		buffer[17] = ((send_time >> 8) & 0xFF) as u8;
		buffer[18] = ((send_time >> 16) & 0xFF) as u8;
		buffer[19] = ((send_time >> 24) & 0xFF) as u8;
		buffer[20] = ((send_time >> 32) & 0xFF) as u8;
		buffer[21] = ((send_time >> 40) & 0xFF) as u8;
		buffer[22] = ((send_time >> 48) & 0xFF) as u8;
		buffer[23] = ((send_time >> 56) & 0xFF) as u8;
		buffer[24] = ((send_time >> 64) & 0xFF) as u8;
		buffer[25] = ((send_time >> 72) & 0xFF) as u8;
		buffer[26] = ((send_time >> 80) & 0xFF) as u8;
		buffer[27] = ((send_time >> 88) & 0xFF) as u8;
		buffer[28] = ((send_time >> 96) & 0xFF) as u8;
		buffer[29] = ((send_time >> 104) & 0xFF) as u8;
		buffer[30] = ((send_time >> 112) & 0xFF) as u8;
		buffer[31] = ((send_time >> 120) & 0xFF) as u8;

		// send precision
		buffer[32] = (self.precision & 0xFF) as u8;
		buffer[33] = ((self.precision >> 8) & 0xFF) as u8;
		buffer[34] = ((self.precision >> 16) & 0xFF) as u8;
		buffer[35] = ((self.precision >> 24) & 0xFF) as u8;
		buffer[36] = ((self.precision >> 32) & 0xFF) as u8;
		buffer[37] = ((self.precision >> 40) & 0xFF) as u8;
		buffer[38] = ((self.precision >> 48) & 0xFF) as u8;
		buffer[39] = ((self.precision >> 56) & 0xFF) as u8;

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&buffer, source) {
			return Err(NtpServerError::Send(error));
		}

		Ok(())
	}
}
