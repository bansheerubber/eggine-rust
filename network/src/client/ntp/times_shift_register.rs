use std::collections::VecDeque;
use std::time::{ Instant, SystemTime, UNIX_EPOCH, };

use super::Times;

/// Collect several `Times` in order to statistically determine the best NTP offset/delay pair to use to correct our
/// system time.
#[derive(Debug)]
pub struct TimesShiftRegister {
	last_best: Option<Times>,
	/// How long it takes to measure system time, in nanoseconds.
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
		let epsilon = self.precision as f64 / 1000.0 + best.server_precision() as f64 / 1000.0
			+ 15.0 * (current_time - best.client_send_time()) as f64 / 1_000_000.0;

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

	pub fn last_best(&self) -> Option<&Times> {
		self.last_best.as_ref()
	}
}
