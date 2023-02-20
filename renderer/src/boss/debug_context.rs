use std::collections::VecDeque;
use std::fs::{ File, OpenOptions, };
use std::io::{ BufWriter, Write, };
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct DebugContext {
	pub frametime_file: Option<BufWriter<File>>,
	pub frametimes: VecDeque<(u64, f64)>,
	pub frametimes_count: usize,
	pub last_second: u64,
	pub time_accumulator: f64,
}

impl DebugContext {
	pub fn default() -> Self {
		let file = OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open("frametimes.log")
			.expect("Could not open frametime file");

		DebugContext {
			frametime_file: Some(BufWriter::new(file)),
			frametimes: VecDeque::new(),
			frametimes_count: 60,
			last_second: 0,
			time_accumulator: 0.0,
		}
	}

	pub fn begin_tick(&mut self, deltatime: f64, frametime: Duration) {
		self.time_accumulator += deltatime;

		if self.frametimes.len() >= self.frametimes_count {
			self.frametimes.pop_front();
		}

		self.frametimes.push_back((frametime.as_micros() as u64, self.time_accumulator));

		if let Some(file) = self.frametime_file.as_mut() {
			file.write_all(
				format!("{} {}\n", frametime.as_micros(), self.time_accumulator).as_bytes()
			).expect("Could not write to frametime file");
		}

		let average = self.frametimes.iter().map(|x| x.0).sum::<u64>() as f32 / self.frametimes.len() as f32;
		let maximum = self.frametimes.iter().map(|x| x.0).max().unwrap() as f32;

		let lows_99_percent = self.frametimes.iter()
			.map(|x| x.0)
			.filter(|x| *x as f32 > maximum * 0.99)
			.collect::<Vec<u64>>();

		let lows_99_percent = lows_99_percent.iter().sum::<u64>() as f32 / lows_99_percent.len() as f32;

		let lows_50_percent = self.frametimes.iter()
			.map(|x| x.0)
			.filter(|x| *x as f32 > maximum * 0.5)
			.collect::<Vec<u64>>();

		let lows_50_percent = lows_50_percent.iter().sum::<u64>() as f32 / lows_50_percent.len() as f32;

		// flush frametime file every second
		if self.last_second != self.time_accumulator as u64 {
			println!("{} {} {}", average, lows_99_percent, lows_50_percent);

			// flush frametime file every second
			if let Some(file) = self.frametime_file.as_mut() {
				file.flush().expect("Could not flush frametime file");
			}
		}
	}

	pub fn end_tick(&mut self) {
		self.last_second = self.time_accumulator as u64;
	}
}
