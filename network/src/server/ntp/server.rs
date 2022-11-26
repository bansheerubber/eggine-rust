use std::collections::HashSet;
use std::net::{ Ipv6Addr, SocketAddr, };
use std::sync::Arc;
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::ReadStream;
use tokio::sync::mpsc;
use tokio::net::{ ToSocketAddrs, UdpSocket, };

use crate::error::NetworkStreamError;
use crate::log::{ Log, LogLevel, };
use crate::network_stream::NetworkReadStream;
use crate::payload::{ NtpPacketHeader, NtpRequestPacket, };

use super::{ MAX_NTP_PACKET_SIZE, NTP_MAGIC_NUMBER, };

#[derive(Debug)]
pub enum NtpServerError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(tokio::io::Error),
	/// Emitted if we could not convert a `SourceAddr` into an `Ipv6Addr` during a receive call. Non-fatal.
	InvalidIP,
	/// Emitted if the client did not send us the expected magic number.
	InvalidMagicNumber(SocketAddr),
	/// Emitted if the client sent us a packet type that we do not recognize
	InvalidPacketType(SocketAddr),
	/// Emitted if we encountered a problem with the tokio mpsc UDP socket communication.
	MpscError(mpsc::error::TryRecvError),
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
	/// Emitted if we receive data from a non-whitelisted IP. Non-fatal.
	NotWhitelisted(SocketAddr),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig(SocketAddr),
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(tokio::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(tokio::io::Error),
}

impl NtpServerError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			NtpServerError::Create(_) => true,
			NtpServerError::InvalidIP => false,
			NtpServerError::InvalidPacketType(_) => false,
			NtpServerError::InvalidMagicNumber(_) => false,
			NtpServerError::MpscError(error) => {
				if let mpsc::error::TryRecvError::Disconnected = error {
					return true;
				} else {
					return false;
				}
			},
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

impl From<mpsc::error::TryRecvError> for NtpServerError {
	fn from(error: mpsc::error::TryRecvError) -> Self {
		NtpServerError::MpscError(error)
	}
}

type Message = (SocketAddr, [u8; MAX_NTP_PACKET_SIZE + 1], usize, i128);

#[derive(Debug)]
pub struct NtpServer {
	/// The address the server is bound to
	address: SocketAddr,
	/// The IPV6 addresses that are allowed to communicate with the NTP server.
	pub address_whitelist: HashSet<Ipv6Addr>,
	log: Log,
	/// Amount of time it takes to read system time, in nanoseconds.
	precision: u64,
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	socket: Arc<UdpSocket>,
	rx: mpsc::Receiver<Message>,
}

impl NtpServer {
	pub async fn new<T: ToSocketAddrs>(address: T) -> Result<Self, NtpServerError> {
		let socket = match UdpSocket::bind(address).await {
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

		// set up the read thread
		let (tx, rx) = mpsc::channel::<Message>(100);
		let socket = Arc::new(socket);
		let receive_socket = socket.clone();
		tokio::spawn(async move {
			loop {
				let mut receive_buffer = [0; MAX_NTP_PACKET_SIZE + 1];
				let (read_bytes, address) = receive_socket.recv_from(&mut receive_buffer).await.expect("Could not receive");
				let recv_time = Self::get_micros();

				tx.send((address, receive_buffer, read_bytes, recv_time)).await.expect("Could not send");
			}
		});

		Ok(NtpServer {
			address: socket.local_addr().unwrap(),
			address_whitelist: HashSet::new(),
			precision: (total / BENCHMARK_TIMES) as u64,
			log: Log::default(),
			receive_stream: NetworkReadStream::new(),
			socket,
			rx,
		})
	}

	/// Consume all messages that the tokio UDP socket sent us.
	pub async fn process_all(&mut self) -> Result<(), NtpServerError> {
		loop {
			match self.rx.try_recv() {
				Ok(message) => self.process(message).await?,
				Err(mpsc::error::TryRecvError::Empty) => return Ok(()),
				Err(error) => return Err(error.into()),
			}
		}
	}

	/// Process a NTP timing information request. Send the timing information back to the socket.
	async fn process_request(&mut self, source: SocketAddr, recv_time: i128) -> Result<(), NtpServerError> {
		let client_packet = self.receive_stream.decode::<NtpRequestPacket>()?.0;

		// send the server time back
		let mut buffer: [u8; 51] = [0; 51];

		// write the NTP packet header
		buffer[0] = 'E' as u8;
		buffer[1] = 'G' as u8;
		buffer[2] = 'G' as u8;
		buffer[3] = 'I' as u8;
		buffer[4] = 'N' as u8;
		buffer[5] = 'E' as u8;
		buffer[6] = 'N' as u8;
		buffer[7] = 'T' as u8;
		buffer[8] = 'P' as u8;
		buffer[9] = 1;

		buffer[10] = client_packet.index;

		// we need to send the times as quick as possible since the longer we take, the more inaccurate the `send_time` is
		// going to be. sends two 128 bit integers for receive time and send time.
		buffer[11] = (recv_time & 0xFF) as u8;
		buffer[12] = ((recv_time >> 8) & 0xFF) as u8;
		buffer[13] = ((recv_time >> 16) & 0xFF) as u8;
		buffer[14] = ((recv_time >> 24) & 0xFF) as u8;
		buffer[15] = ((recv_time >> 32) & 0xFF) as u8;
		buffer[16] = ((recv_time >> 40) & 0xFF) as u8;
		buffer[17] = ((recv_time >> 48) & 0xFF) as u8;
		buffer[18] = ((recv_time >> 56) & 0xFF) as u8;
		buffer[19] = ((recv_time >> 64) & 0xFF) as u8;
		buffer[20] = ((recv_time >> 72) & 0xFF) as u8;
		buffer[21] = ((recv_time >> 80) & 0xFF) as u8;
		buffer[22] = ((recv_time >> 88) & 0xFF) as u8;
		buffer[23] = ((recv_time >> 96) & 0xFF) as u8;
		buffer[24] = ((recv_time >> 104) & 0xFF) as u8;
		buffer[25] = ((recv_time >> 112) & 0xFF) as u8;
		buffer[26] = ((recv_time >> 120) & 0xFF) as u8;

		// send precision
		buffer[27] = (self.precision & 0xFF) as u8;
		buffer[28] = ((self.precision >> 8) & 0xFF) as u8;
		buffer[29] = ((self.precision >> 16) & 0xFF) as u8;
		buffer[30] = ((self.precision >> 24) & 0xFF) as u8;
		buffer[31] = ((self.precision >> 32) & 0xFF) as u8;
		buffer[32] = ((self.precision >> 40) & 0xFF) as u8;
		buffer[33] = ((self.precision >> 48) & 0xFF) as u8;
		buffer[34] = ((self.precision >> 56) & 0xFF) as u8;

		let send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

		// send time encoding
		buffer[35] = (send_time & 0xFF) as u8;
		buffer[36] = ((send_time >> 8) & 0xFF) as u8;
		buffer[37] = ((send_time >> 16) & 0xFF) as u8;
		buffer[38] = ((send_time >> 24) & 0xFF) as u8;
		buffer[39] = ((send_time >> 32) & 0xFF) as u8;
		buffer[40] = ((send_time >> 40) & 0xFF) as u8;
		buffer[41] = ((send_time >> 48) & 0xFF) as u8;
		buffer[42] = ((send_time >> 56) & 0xFF) as u8;
		buffer[43] = ((send_time >> 64) & 0xFF) as u8;
		buffer[44] = ((send_time >> 72) & 0xFF) as u8;
		buffer[45] = ((send_time >> 80) & 0xFF) as u8;
		buffer[46] = ((send_time >> 88) & 0xFF) as u8;
		buffer[47] = ((send_time >> 96) & 0xFF) as u8;
		buffer[48] = ((send_time >> 104) & 0xFF) as u8;
		buffer[49] = ((send_time >> 112) & 0xFF) as u8;
		buffer[50] = ((send_time >> 120) & 0xFF) as u8;

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&buffer, source).await {
			return Err(NtpServerError::Send(error));
		}

		Ok(())
	}

	async fn process(&mut self, message: Message) -> Result<(), NtpServerError> {
		let (source, receive_buffer, read_bytes, recv_time) = message;

		// convert the `SocketAddr` into a `Ipv6Addr`. `Ipv6Addr`s do not contain the port the client connected from, the
		// lack of which is required for the blacklist implementation
		let address = if let SocketAddr::V6(address) = source {
			address.ip().clone()
		} else {
			return Err(NtpServerError::InvalidIP);
		};

		// stop non-whitelisted data from continuing
		if !self.address_whitelist.contains(&address) {
			return Err(NtpServerError::NotWhitelisted(source));
		}

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_NTP_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet from {:?}", source), 0);
			return Err(NtpServerError::PacketTooBig(source));
		}

		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&receive_buffer[0..read_bytes]);

		self.receive_stream.import(buffer)?;

		let packet_header = self.receive_stream.decode::<NtpPacketHeader>()?.0;
		if packet_header.magic_number != NTP_MAGIC_NUMBER {
			self.log.print(LogLevel::Error, format!("received invalid magic number from {:?}", source), 0);
			return Err(NtpServerError::InvalidMagicNumber(source));
		}

		match packet_header.packet_type {
			0 => self.process_request(source, recv_time).await?,
			1 => {},
			_ => {
				self.log.print(LogLevel::Error, format!("received invalid packet type from {:?}", source), 0);
				return Err(NtpServerError::InvalidPacketType(source));
			}
		}

		Ok(())
	}

	/// Get time in microseconds.
	fn get_micros() -> i128 {
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128
	}
}
