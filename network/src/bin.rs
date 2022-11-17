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
				}
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}
	} else {
		let mut client = Client::new(last_argument).unwrap();
		client.test_send();

		loop {
			if let Err(error) = client.tick() {
				if error.is_fatal() {
					panic!("{:?}", error);
				}
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}
	}
}
