use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };
use streams::{ ReadStream, WriteStream, };
use tokio::sync::mpsc;
use tokio::net::{ ToSocketAddrs, UdpSocket, };

use crate::error::NetworkStreamError;
use crate::log::{ Log, LogLevel, };
use crate::network_stream::{ NetworkReadStream, NetworkWriteStream, };
use crate::payload::{ NtpPacketHeader, NtpRequestPacket, NtpResponsePacket, };

use super::{ Times, TimesShiftRegister, MAX_NTP_PACKET_SIZE, NTP_MAGIC_NUMBER, };

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
	/// Emitted if we could not match a address to an id.
	NoId(SocketAddr),
	/// Emitted if we could not match an ID to a host ID.
	NoHostId(u32),
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
			NtpServerError::NoId(_) => false,
			NtpServerError::NoHostId(_) => false,
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
	/// Used to translate a socket address to the ID that they've been assigned.
	pub address_to_id: HashMap<SocketAddr, u32>,
	/// The address the server is connected to, if any.
	host_address: Option<SocketAddr>,
	/// Used to translate between client IDs and the host ID they expect to find in packets we send to them.
	pub id_to_host_id: HashMap<u32, u32>,
	log: Log,
	/// Used to distinguish between packets and timing statistics.
	packet_indexes: HashMap<u32, u8>,
	/// Amount of time it takes to read system time, in nanoseconds.
	precision: u64,
	/// The stream we import data into when we receive data.
	receive_stream: NetworkReadStream,
	/// The stream we use to export data so we can sent it to a client.
	send_stream: NetworkWriteStream,
	/// The last time we sent a packet to the NTP server.
	send_times: HashMap<(u32, u8), i128>,
	/// Used for determining the best system time correction.
	shift_registers: HashMap<u32, TimesShiftRegister>,
	socket: Arc<UdpSocket>,
	rx: mpsc::Receiver<Message>,
}

impl NtpServer {
	pub async fn new<T: ToSocketAddrs>(address: T, host_address: Option<T>) -> Result<Self, NtpServerError> {
		let socket = match UdpSocket::bind(address).await {
			Ok(socket) => socket,
			Err(error) => return Err(NtpServerError::Create(error)),
		};

		if let Some(host_address) = host_address {
			if let Err(error) = socket.connect(host_address).await {
				return Err(NtpServerError::Create(error));
			}
		}

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

		let host_address = if let Ok(address) = socket.peer_addr() {
			Some(address)
		} else {
			None
		};

		Ok(NtpServer {
			address_to_id: HashMap::new(),
			host_address,
			id_to_host_id: HashMap::new(),
			log: Log::default(),
			packet_indexes: HashMap::new(),
			precision: (total / BENCHMARK_TIMES) as u64,
			receive_stream: NetworkReadStream::new(),
			send_stream: NetworkWriteStream::new(),
			send_times: HashMap::new(),
			shift_registers: HashMap::new(),
			socket,
			rx,
		})
	}

	/// Send a time synchronization request to the server.
	pub async fn sync_time(&mut self, address: Option<SocketAddr>) -> Result<(), NtpServerError> {
		// determine the address
		let address = if let Some(address) = address {
			address
		} else {
			self.host_address.unwrap()
		};

		let Some(id) = self.address_to_id.get(&address) else {
			return Err(NtpServerError::NoId(address));
		};

		let Some(host_id) = self.id_to_host_id.get(id) else {
			return Err(NtpServerError::NoHostId(*id));
		};

		if !self.packet_indexes.contains_key(id) {
			self.packet_indexes.insert(*id, 0);
		}

		// encode packet header
		let header = NtpPacketHeader {
			id: *host_id,
			magic_number: String::from(NTP_MAGIC_NUMBER),
			packet_type: 0,
		};

		self.send_stream.encode(&header)?;

		// encode request packet
		let packet_index = u8::overflowing_add(self.packet_indexes[id], 1).0;
		self.packet_indexes.insert(*id, packet_index);
		let packet = NtpRequestPacket {
			index: packet_index,
		};

		self.send_stream.encode(&packet)?;

		// wait for socket to become writable
		if let Err(error) = self.socket.writable().await {
			return Err(NtpServerError::Send(error));
		}

		let time = Self::get_micros();
		let result = if let Err(error) = self.socket.send_to(&self.send_stream.export()?, address).await {
			Err(NtpServerError::Send(error))
		} else {
			Ok(())
		};

		self.send_times.insert((*id, packet_index), time);

		return result;
	}

	/// Consume all messages that the tokio UDP socket sent us.
	pub async fn process_all(&mut self) -> Result<(), NtpServerError> {
		loop {
			match self.rx.try_recv() {
				Ok(message) => {
					if let Err(error) = self.process(message).await {
						println!("{:?}", error);
						if error.is_fatal() {
							return Err(error);
						}
					}
				},
				Err(mpsc::error::TryRecvError::Empty) => return Ok(()),
				Err(error) => return Err(error.into()),
			}
		}
	}

	/// Filter bad packets, then read the packet header and figure out what to do with the packet.
	async fn process(&mut self, message: Message) -> Result<(), NtpServerError> {
		let (source, receive_buffer, read_bytes, recv_time) = message;

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

		// stop non-whitelisted data from continuing
		if !self.id_to_host_id.contains_key(&packet_header.id) {
			return Err(NtpServerError::NotWhitelisted(source));
		}

		self.address_to_id.insert(source, packet_header.id);

		match packet_header.packet_type {
			0 => self.process_request(source, packet_header.id, recv_time).await?,
			1 => self.process_response(packet_header.id, recv_time)?,
			_ => {
				self.log.print(LogLevel::Error, format!("received invalid packet type from {:?}", source), 0);
				return Err(NtpServerError::InvalidPacketType(source));
			}
		}

		Ok(())
	}

	/// Process a NTP timing information request. Send the timing information back to the socket.
	async fn process_request(&mut self, source: SocketAddr, id: u32, recv_time: i128) -> Result<(), NtpServerError> {
		let client_packet = self.receive_stream.decode::<NtpRequestPacket>()?.0;

		let Some(host_id) = self.id_to_host_id.get(&id) else {
			return Err(NtpServerError::NoHostId(id));
		};

		// send the server time back
		let mut buffer: [u8; MAX_NTP_PACKET_SIZE] = [0; MAX_NTP_PACKET_SIZE];

		// write the NTP packet header
		buffer[0] = 9;
		buffer[1] = 0;
		buffer[2] = 'E' as u8;
		buffer[3] = 'G' as u8;
		buffer[4] = 'G' as u8;
		buffer[5] = 'I' as u8;
		buffer[6] = 'N' as u8;
		buffer[7] = 'E' as u8;
		buffer[8] = 'N' as u8;
		buffer[9] = 'T' as u8;
		buffer[10] = 'P' as u8;
		buffer[11] = (host_id & 0xFF) as u8;
		buffer[12] = ((host_id >> 8) & 0xFF) as u8;
		buffer[13] = ((host_id >> 16) & 0xFF) as u8;
		buffer[14] = ((host_id >> 24) & 0xFF) as u8;
		buffer[15] = 1;

		buffer[16] = client_packet.index;

		// we need to send the times as quick as possible since the longer we take, the more inaccurate the `send_time` is
		// going to be. sends two 128 bit integers for receive time and send time.
		buffer[17] = (recv_time & 0xFF) as u8;
		buffer[18] = ((recv_time >> 8) & 0xFF) as u8;
		buffer[19] = ((recv_time >> 16) & 0xFF) as u8;
		buffer[20] = ((recv_time >> 24) & 0xFF) as u8;
		buffer[21] = ((recv_time >> 32) & 0xFF) as u8;
		buffer[22] = ((recv_time >> 40) & 0xFF) as u8;
		buffer[23] = ((recv_time >> 48) & 0xFF) as u8;
		buffer[24] = ((recv_time >> 56) & 0xFF) as u8;
		buffer[25] = ((recv_time >> 64) & 0xFF) as u8;
		buffer[26] = ((recv_time >> 72) & 0xFF) as u8;
		buffer[27] = ((recv_time >> 80) & 0xFF) as u8;
		buffer[28] = ((recv_time >> 88) & 0xFF) as u8;
		buffer[29] = ((recv_time >> 96) & 0xFF) as u8;
		buffer[30] = ((recv_time >> 104) & 0xFF) as u8;
		buffer[31] = ((recv_time >> 112) & 0xFF) as u8;
		buffer[32] = ((recv_time >> 120) & 0xFF) as u8;

		// send precision
		buffer[33] = (self.precision & 0xFF) as u8;
		buffer[34] = ((self.precision >> 8) & 0xFF) as u8;
		buffer[35] = ((self.precision >> 16) & 0xFF) as u8;
		buffer[36] = ((self.precision >> 24) & 0xFF) as u8;
		buffer[37] = ((self.precision >> 32) & 0xFF) as u8;
		buffer[38] = ((self.precision >> 40) & 0xFF) as u8;
		buffer[39] = ((self.precision >> 48) & 0xFF) as u8;
		buffer[40] = ((self.precision >> 56) & 0xFF) as u8;

		let send_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;

		// send time encoding
		buffer[41] = (send_time & 0xFF) as u8;
		buffer[42] = ((send_time >> 8) & 0xFF) as u8;
		buffer[43] = ((send_time >> 16) & 0xFF) as u8;
		buffer[44] = ((send_time >> 24) & 0xFF) as u8;
		buffer[45] = ((send_time >> 32) & 0xFF) as u8;
		buffer[46] = ((send_time >> 40) & 0xFF) as u8;
		buffer[47] = ((send_time >> 48) & 0xFF) as u8;
		buffer[48] = ((send_time >> 56) & 0xFF) as u8;
		buffer[49] = ((send_time >> 64) & 0xFF) as u8;
		buffer[50] = ((send_time >> 72) & 0xFF) as u8;
		buffer[51] = ((send_time >> 80) & 0xFF) as u8;
		buffer[52] = ((send_time >> 88) & 0xFF) as u8;
		buffer[53] = ((send_time >> 96) & 0xFF) as u8;
		buffer[54] = ((send_time >> 104) & 0xFF) as u8;
		buffer[55] = ((send_time >> 112) & 0xFF) as u8;
		buffer[56] = ((send_time >> 120) & 0xFF) as u8;

		// TODO check if the amount of bytes sent in the socket matches the size of the exported vector
		if let Err(error) = self.socket.send_to(&buffer, source).await {
			return Err(NtpServerError::Send(error));
		}

		Ok(())
	}

	/// Process the response sent. Update internal state using the timing information received.
	fn process_response(&mut self, id: u32, recv_time: i128) -> Result<(), NtpServerError> {
		if !self.shift_registers.contains_key(&id) {
			self.shift_registers.insert(id, TimesShiftRegister::new(300, self.precision));
		}

		// figure out what to do with the packet we just got
		let packet = self.receive_stream.decode::<NtpResponsePacket>()?.0;

		let send_time = self.send_times[&(id, packet.packet_index)];
		let shift_register = self.shift_registers.get_mut(&id).unwrap();
		shift_register.add_time(Some(Times::new(
			recv_time,
			send_time,
			packet.precision,
			packet.receive_time,
			packet.send_time,
		)));

		// print some continuously running statistics
		// let best = shift_register.best().unwrap();

		// println!("time offset: {}us", best.time_offset());
		// println!("round-trip: {}us", best.delay());
		// println!("jitter: {}us", shift_register.jitter().unwrap());
		// println!("delay std: {}", shift_register.delay_std());
		// println!("synchronization distance: {}us", shift_register.synchronization_distance().unwrap());

		// if shift_register.last_best().is_some() {
		// 	println!(
		// 		"distance from last best: {}",
		// 		best.time_offset() - shift_register.last_best().unwrap().time_offset()
		// 	);
		// }

		Ok(())
	}

	/// Get time in microseconds.
	fn get_micros() -> i128 {
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128
	}
}
