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
use crate::payload::NtpClientPacket;

use super::{ MAX_NTP_PACKET_SIZE, NTP_MAGIC_NUMBER, };

#[derive(Debug)]
pub enum NtpServerError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(tokio::io::Error),
	/// Emitted if we could not convert a `SourceAddr` into an `Ipv6Addr` during a receive call. Non-fatal.
	InvalidIP,
	/// Emitted if the client did not send us the expected magic number.
	InvalidMagicNumber(SocketAddr),
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
	expected_client_packet: NtpClientPacket,
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
			expected_client_packet: NtpClientPacket {
				index: 0,
				magic_number: String::from(NTP_MAGIC_NUMBER),
			},
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

		let client_packet = self.receive_stream.decode::<NtpClientPacket>()?.0;
		self.expected_client_packet.index = client_packet.index;
		if client_packet != self.expected_client_packet {
			self.log.print(LogLevel::Error, format!("received invalid magic number from {:?}", source), 0);
			return Err(NtpServerError::InvalidMagicNumber(source));
		}

		// send the server time back
		let mut buffer: [u8; 41] = [0; 41];

		buffer[0] = client_packet.index;

		// we need to send the times as quick as possible since the longer we take, the more inaccurate the `send_time` is
		// going to be. sends two 128 bit integers for receive time and send time.
		buffer[1] = (recv_time & 0xFF) as u8;
		buffer[2] = ((recv_time >> 8) & 0xFF) as u8;
		buffer[3] = ((recv_time >> 16) & 0xFF) as u8;
		buffer[4] = ((recv_time >> 24) & 0xFF) as u8;
		buffer[5] = ((recv_time >> 32) & 0xFF) as u8;
		buffer[6] = ((recv_time >> 40) & 0xFF) as u8;
		buffer[7] = ((recv_time >> 48) & 0xFF) as u8;
		buffer[8] = ((recv_time >> 56) & 0xFF) as u8;
		buffer[9] = ((recv_time >> 64) & 0xFF) as u8;
		buffer[10] = ((recv_time >> 72) & 0xFF) as u8;
		buffer[11] = ((recv_time >> 80) & 0xFF) as u8;
		buffer[12] = ((recv_time >> 88) & 0xFF) as u8;
		buffer[13] = ((recv_time >> 96) & 0xFF) as u8;
		buffer[14] = ((recv_time >> 104) & 0xFF) as u8;
		buffer[15] = ((recv_time >> 112) & 0xFF) as u8;
		buffer[16] = ((recv_time >> 120) & 0xFF) as u8;

		// send precision
		buffer[32] = (self.precision & 0xFF) as u8;
		buffer[33] = ((self.precision >> 8) & 0xFF) as u8;
		buffer[34] = ((self.precision >> 16) & 0xFF) as u8;
		buffer[35] = ((self.precision >> 24) & 0xFF) as u8;
		buffer[36] = ((self.precision >> 32) & 0xFF) as u8;
		buffer[37] = ((self.precision >> 40) & 0xFF) as u8;
		buffer[38] = ((self.precision >> 48) & 0xFF) as u8;
		buffer[39] = ((self.precision >> 56) & 0xFF) as u8;

		let send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

		// send time encoding
		buffer[17] = (send_time & 0xFF) as u8;
		buffer[18] = ((send_time >> 8) & 0xFF) as u8;
		buffer[19] = ((send_time >> 16) & 0xFF) as u8;
		buffer[20] = ((send_time >> 24) & 0xFF) as u8;
		buffer[21] = ((send_time >> 32) & 0xFF) as u8;
		buffer[22] = ((send_time >> 40) & 0xFF) as u8;
		buffer[23] = ((send_time >> 48) & 0xFF) as u8;
		buffer[24] = ((send_time >> 56) & 0xFF) as u8;
		buffer[25] = ((send_time >> 64) & 0xFF) as u8;
		buffer[26] = ((send_time >> 72) & 0xFF) as u8;
		buffer[27] = ((send_time >> 80) & 0xFF) as u8;
		buffer[28] = ((send_time >> 88) & 0xFF) as u8;
		buffer[29] = ((send_time >> 96) & 0xFF) as u8;
		buffer[30] = ((send_time >> 104) & 0xFF) as u8;
		buffer[31] = ((send_time >> 112) & 0xFF) as u8;
		buffer[32] = ((send_time >> 120) & 0xFF) as u8;

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&buffer, source).await {
			return Err(NtpServerError::Send(error));
		}

		Ok(())
	}

	/// Get time in microseconds.
	fn get_micros() -> i128 {
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128
	}
}
