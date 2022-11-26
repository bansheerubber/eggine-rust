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
	pub fn new(
		client_receive_time: i128,
		client_send_time: i128,
		server_precision: u64,
		server_receive_time: i128,
		server_send_time: i128
	) -> Self {
		Times {
			client_receive_time,
			client_send_time,
			server_precision,
			server_receive_time,
			server_send_time,
		}
	}

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

	pub fn server_precision(&self) -> u64 {
		self.server_precision
	}

	pub fn client_send_time(&self) -> i128 {
		self.client_send_time
	}
}
