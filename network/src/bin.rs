use std::collections::HashSet;

use network::{ Client, Server, };
use network::client::client::ClientError;
use network::server::server::ServerError;

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
				if let Some(error) = error.as_any().downcast_ref::<ServerError>() {
					if error.is_fatal() {
						panic!("{:?}", error);
					}
				} else {
					panic!("{:?}", error);
				}
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}
	} else {
		let mut client = Client::new("[::]:0").unwrap();
		client.initialize_connection(last_argument).expect("Could not initialize connection to the server");

		loop {
			if let Err(error) = client.tick() {
				if let Some(error) = error.as_any().downcast_ref::<ClientError>() {
					if error.is_fatal() {
						panic!("{:?}", error);
					}
				} else {
					panic!("{:?}", error);
				}
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}
	}
}
