use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{ SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };
use tokio::sync::mpsc;
use tokio::net::{ ToSocketAddrs, UdpSocket, };

use crate::error::NetworkStreamError;
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ NtpClientPacket, NtpServerPacket, };
use crate::server::ntp_server::{ NTP_MAGIC_NUMBER, MAX_NTP_PACKET_SIZE, };
use crate::MAX_PACKET_SIZE;

use super::times::Times;
use super::times_shift_register::TimesShiftRegister;

#[derive(Debug)]
pub enum NtpClientError {
	/// Emitted if we encountered a problem creating + binding the socket. Fatal.
	Create(tokio::io::Error),
	/// Emitted if we encountered a problem connecting to a server socket. Fatal.
	Connect(tokio::io::Error),
	/// Emitted if the server response does not have the correct packet index.
	InvalidIndex,
	/// Emitted if we encountered a problem with the tokio mpsc UDP socket communication.
	MpscError(mpsc::error::TryRecvError),
	/// Emitted if we encountered a problem with network streams.
	NetworkStreamError(NetworkStreamError),
	/// Emitted if a received packet is too big to be an eggine packet. Non-fatal.
	PacketTooBig,
	/// Emitted if we encountered an OS socket error during a receive. Fatal.
	Receive(tokio::io::Error),
	/// Emitted if we encountered an OS socket error during a send. Fatal.
	Send(tokio::io::Error),
	/// Emitted if a socket call would block. With the non-blocking flag set, this indicates that we have consumed all
	/// available packets from the socket at the moment. Non-fatal.
	WouldBlock,
}

impl NtpClientError {
	/// Identifies whether or not the server needs a restart upon the emission of an error.
	pub fn is_fatal(&self) -> bool {
		match *self {
			NtpClientError::Create(_) => true,
			NtpClientError::Connect(_) => true,
			NtpClientError::InvalidIndex => false,
			NtpClientError::MpscError(error) => {
				if let mpsc::error::TryRecvError::Disconnected = error {
					return true;
				} else {
					return false;
				}
			},
			NtpClientError::NetworkStreamError(_) => false,
			NtpClientError::PacketTooBig => false,
			NtpClientError::Receive(_) => true,
			NtpClientError::Send(_) => true,
			NtpClientError::WouldBlock => false,
		}
	}
}

impl From<NetworkStreamError> for NtpClientError {
	fn from(error: NetworkStreamError) -> Self {
		NtpClientError::NetworkStreamError(error)
	}
}

impl From<mpsc::error::TryRecvError> for NtpClientError {
	fn from(error: mpsc::error::TryRecvError) -> Self {
		NtpClientError::MpscError(error)
	}
}

#[derive(Clone, Copy, Debug)]
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

	pub fn offset(&self) -> i128 {
		self.offset
	}

	pub fn system_time(&self) -> i128 {
		self.system_time
	}

	pub fn time(&self) -> i128 {
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

type Message = ([u8; MAX_NTP_PACKET_SIZE + 1], usize, i128);

/// A client connection in the eggine network stack. Clients connect to servers, which are treated as a trusted source
/// of information. The two communicate using a packet format built upon the streams library.
#[derive(Debug)]
pub struct NtpClient {
	address: SocketAddr,
	log: Log,
	packet_index: u8,
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	/// The stream we use to export data so we can sent it to a client.
	send_stream: NetworkWriteStream,
	/// The last time we sent a packet to the NTP server.
	send_times: HashMap<u8, i128>,
	/// Used for determining the best system time correction.
	shift_register: TimesShiftRegister,
	/// The socket the client is connected to the server on.
	socket: Arc<UdpSocket>,
	rx: mpsc::Receiver<Message>,
}

impl NtpClient {
	/// Initialize the a client socket bound to the specified address.
	pub async fn new<T: ToSocketAddrs>(address: T, host_address: T) -> Result<Self, NtpClientError> {
		let socket = match UdpSocket::bind(address).await {
			Ok(socket) => socket,
			Err(error) => return Err(NtpClientError::Create(error)),
		};

		if let Err(error) = socket.connect(host_address).await {
			return Err(NtpClientError::Create(error));
		}

		let (tx, rx) = mpsc::channel::<Message>(100);

		// set up the read thread
		let socket = Arc::new(socket);
		let receive_socket = socket.clone();
		tokio::spawn(async move {
			loop {
				let mut receive_buffer = [0; MAX_NTP_PACKET_SIZE + 1];
				let read_bytes = receive_socket.recv(&mut receive_buffer).await.expect("Could not receive");
				let recv_time = Self::get_micros();

				tx.send((receive_buffer, read_bytes, recv_time)).await.expect("Could not send");
			}
		});

		Ok(NtpClient {
			address: socket.local_addr().unwrap(),
			log: Log::default(),
			packet_index: 0,
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			send_times: HashMap::new(),
			shift_register: TimesShiftRegister::new(300),
			socket,
			rx,
		})
	}

	/// Send a time synchronization request to the server.
	pub async fn sync_time(&mut self) -> Result<(), NtpClientError> {
		self.packet_index = u8::overflowing_add(self.packet_index, 1).0;
		let packet = NtpClientPacket {
			index: self.packet_index,
			magic_number: String::from(NTP_MAGIC_NUMBER),
		};

		self.send_stream.encode(&packet)?;

		// wait for socket to become writable
		if let Err(error) = self.socket.writable().await {
			return Err(NtpClientError::Receive(error));
		}

		self.send_times.insert(self.packet_index, Self::get_micros());

		if let Err(error) = self.socket.send(&self.send_stream.export()?).await {
			Err(NtpClientError::Send(error))
		} else {
			Ok(())
		}
	}

	pub fn get_corrected_time(&self) -> CorrectedTime {
		let offset = if let Some(best) = self.shift_register.best() {
			best.time_offset()
		} else {
			0
		};

		CorrectedTime::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128, offset)
	}

	/// Consume all messages that the tokio UDP socket sent us.
	pub async fn process_all(&mut self) -> Result<(), NtpClientError> {
		loop {
			match self.rx.try_recv() {
				Ok(message) => self.process(message)?,
				Err(mpsc::error::TryRecvError::Empty) => return Ok(()),
				Err(error) => return Err(error.into()),
			}
		}
	}

	/// Process data from a message.
	fn process(&mut self, message: Message) -> Result<(), NtpClientError> {
		let (receive_buffer, read_bytes, recv_time) = message;

		// make sure what we just read is not too big to be an eggine packet
		if read_bytes > MAX_PACKET_SIZE {
			self.log.print(LogLevel::Error, format!("received too big of a packet"), 0);
			return Err(NtpClientError::PacketTooBig);
		}

		// import raw bytes into the receive stream
		// TODO optimize this
		let mut buffer: Vec<u8> = Vec::new();
		buffer.extend(&receive_buffer[0..read_bytes]);
		self.receive_stream.import(buffer)?;

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<NtpServerPacket>()?.0;
		let send_time = self.send_times[&packet.packet_index];
		self.shift_register.add_time(Some(Times::new(
			recv_time,
			send_time,
			packet.precision,
			packet.receive_time,
			packet.send_time,
		)));

		let best = self.shift_register.best().unwrap();

		println!("time offset: {}us", best.time_offset());
		println!("round-trip: {}us", best.delay());
		println!("jitter: {}us", self.shift_register.jitter().unwrap());
		println!("delay std: {}", self.shift_register.delay_std());
		println!("synchronization distance: {}us", self.shift_register.synchronization_distance().unwrap());

		if self.shift_register.last_best().is_some() {
			println!("distance from last best: {}", best.time_offset() - self.shift_register.last_best().unwrap().time_offset());
		}

		Ok(())
	}

	/// Get time in microseconds.
	fn get_micros() -> i128 {
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128
	}
}
