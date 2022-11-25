use std::collections::HashSet;

use network::{ Client, Server, };

fn main() {
	let mut arguments = HashSet::new();
	let mut last_argument = String::new();
	for argument in std::env::args() {
		arguments.insert(argument.clone());
		last_argument = argument;
	}

	if arguments.contains("--server") {
		let mut server = Server::new(last_argument).unwrap();
		loop {
			if let Err(error) = server.tick() {
				if error.is_fatal() {
					panic!("{:?}", error);
				} else {
					panic!("{:?}", error);
				}
			}
			std::thread::sleep(std::time::Duration::from_millis(33));
		}
	} else {
		let mut client = Client::new("[::]:0").unwrap();
		client.initialize_connection(last_argument).expect("Could not initialize connection to the server");
		std::thread::sleep(std::time::Duration::from_secs(1));

		let mut last_ping = std::time::Instant::now();

		loop {
			// if std::time::Instant::now() - last_ping > std::time::Duration::from_secs(1) {
				client.ping().unwrap();
				// last_ping = std::time::Instant::now();
			// }

			if let Err(error) = client.tick() {
				if error.is_fatal() {
					panic!("{:?}", error);
				}
			}

			std::thread::sleep(std::time::Duration::from_millis(16));
		}
	}
}
