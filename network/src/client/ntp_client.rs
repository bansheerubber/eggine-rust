use std::collections::VecDeque;
use std::net::{ SocketAddr, ToSocketAddrs, UdpSocket, };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, Duration, };
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

/// Collect several `Times` in order to statistically determine the best NTP offset/delay pair to use to correct our
/// system time.
#[derive(Debug)]
pub struct TimesShiftRegister {
	/// How long it takes to measure system time, in nanoseconds.
	last_best: Option<Times>,
	precision: u64,
	max_amount: usize,
	times: VecDeque<Times>,
}

impl TimesShiftRegister {
	pub fn new(max_amount: usize) -> Self {
		// benchmark precision
		const BENCHMARK_TIMES: u128 = 1000;
		let mut total = 0;
		for _ in 0..BENCHMARK_TIMES {
			let start = Instant::now();
			#[allow(unused_must_use)] {
				SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros();
			}

			total += (Instant::now() - start).as_nanos();
		}

		TimesShiftRegister {
			last_best: None,
			precision: (total / BENCHMARK_TIMES) as u64,
			max_amount,
			times: VecDeque::new(),
		}
	}

	/// Add a `Times` to the shift register.
	pub fn add_time(&mut self, times: Times) {
		let last_best = self.best().cloned();

		if self.times.len() > self.max_amount {
			self.times.pop_back();
		}
		self.times.push_front(times);

		let best = self.best().cloned();

		if last_best.is_some() && last_best != best {
			self.last_best = Some(last_best.unwrap().clone());
		}
	}

	/// Returns the best `Times` for use in correcting system time.
	pub fn best(&self) -> Option<&Times> {
		let mut minimum = (16_000_000, None);
		for times in self.times.iter() {
			if times.delay() < minimum.0 {
				minimum = (times.delay(), Some(times));
			}
		}

		minimum.1
	}

	/// Calculates the jitter in time offsets.
	pub fn jitter(&self) -> Option<f64> {
		let Some(best) = self.best() else {
			return None;
		};

		let n = self.times.len() as i128 - 1;
		let mut differences = 0.0;
		for times in self.times.iter() {
			if times == best {
				continue;
			}

			differences += (1.0 / n as f64) * f64::powi(times.time_offset() as f64 - best.time_offset() as f64, 2);
		}

		Some(f64::sqrt(differences))
	}

	/// Calculates the synchronization distance. Represents maximum error.
	pub fn synchronization_distance(&self) -> Option<f64> {
		let Some(best) = self.best() else {
			return None;
		};

		// sum of client and server precisions, sum grows at 15 microseconds per second
		let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128;
		let epsilon = self.precision as f64 / 1000.0 + best.server_precision as f64 / 1000.0
			+ 15.0 * (current_time - best.client_send_time) as f64 / 1_000_000.0;

		Some(epsilon + best.delay() as f64 / 2.0)
	}

	/// Calculates delay variance.
	pub fn delay_std(&self) -> Option<f64> {
		let mean = self.times.iter().fold(0.0, |accum, times| accum + times.delay() as f64) / (self.times.len() as f64);

		Some(f64::sqrt(
			self.times.iter()
				.fold(0.0, |accum, times| accum + f64::powi(times.delay() as f64 - mean, 2)) / (self.times.len() as f64)
		))
	}
}

/// Used for client-sided time adjustments after syncing to the server's clock. All times are in microseconds. Based on
/// NTP (https://en.wikipedia.org/wiki/Network_Time_Protocol#Clock_synchronization_algorithm).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Times {
	/// The time we received the server's answer.
	client_receive_time: i128,
	/// The time we sent our time request to the server.
	client_send_time: i128,
	/// The precision of the server.
	server_precision: u64,
	/// The time the server received our time request.
	server_receive_time: i128,
	/// The time the server sent its response to us.
	server_send_time: i128,
}

impl Times {
	/// The time between the client sending a time request and receiving the server's response. Based on NTP's round-trip
	/// delay calculation.
	pub fn delay(&self) -> i128 {
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
	/// Used for determining the best system time correction.
	shift_register: TimesShiftRegister,
	/// The socket the client is connected to the server on.
	socket: UdpSocket,
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

		if let Err(error) = socket.set_read_timeout(Some(Duration::from_secs(5))) {
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
			shift_register: TimesShiftRegister::new(300),
			socket,
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

			send_time = self.get_corrected_time();

			if let Err(error) = self.socket.send(&self.send_stream.export()?) {
				return Err(NtpClientError::Send(error));
			}
		}

		// receive the time
		{
			let read_bytes = match self.socket.recv(&mut self.receive_buffer) {
				Ok(a) => a,
				Err(error) => {
					if error.raw_os_error().unwrap() == 11 {
						return Err(NtpClientError::WouldBlock);
					} else {
						return Err(NtpClientError::Receive(error));
					}
				},
			};

			let recv_time = self.get_corrected_time();

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

			self.shift_register.add_time(Times {
				client_receive_time: recv_time.system_time(),
				client_send_time: send_time.system_time(),
				server_precision: packet.precision,
				server_receive_time: packet.receive_time,
				server_send_time: packet.send_time,
			});

			let best = self.shift_register.best().unwrap();

			println!("time offset: {}us", best.time_offset());
			println!("round-trip: {}us", best.delay());
			println!("jitter: {}us", self.shift_register.jitter().unwrap());

			if self.shift_register.delay_std().is_some() {
				println!("delay variance: {}", self.shift_register.delay_std().unwrap());
			}

			println!("synchronization distance: {}us", self.shift_register.synchronization_distance().unwrap());

			if self.shift_register.last_best.is_some() {
				println!("distance from last best: {}", best.time_offset() - self.shift_register.last_best.unwrap().time_offset());
			}
		}

		Ok(())
	}

	pub fn get_corrected_time(&self) -> CorrectedTime {
		let offset = if let Some(best) = self.shift_register.best() {
			best.time_offset()
		} else {
			0
		};

		CorrectedTime::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as i128, offset)
	}
}
