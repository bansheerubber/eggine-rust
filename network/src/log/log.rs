#[derive(Debug)]
pub enum LogLevel {
	Blacklist,
	Error,
	Info,
}

#[derive(Debug, Default)]
pub struct Log {
	messages: Vec<String>,
}

impl Log {
	pub fn print(&mut self, log_level: LogLevel, message: String, indent: u8) {
		let log_level_char = match log_level {
			LogLevel::Blacklist => '@',
			LogLevel::Error => '!',
			LogLevel::Info => '.',
		};

		let mut indent_string = String::new();
		for _ in 0..indent {
			indent_string += "  ";
		}

		let message = format!("{}{} {}", indent_string, log_level_char, message);
		println!("{}", message);
		self.messages.push(message);
	}
}
