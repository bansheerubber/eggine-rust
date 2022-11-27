use std::collections::HashSet;
use tokio;

use network::{ Client, Server, };

#[tokio::main]
async fn main() {
	let mut arguments = HashSet::new();
	let mut last_argument = String::new();
	for argument in std::env::args() {
		arguments.insert(argument.clone());
		last_argument = argument;
	}

	if arguments.contains("--server") {
		let mut server = Server::new(last_argument).await.unwrap();
		loop {
			if let Err(error) = server.tick().await {
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
		client.initialize_connection(last_argument).await.expect("Could not initialize connection to the server");
		std::thread::sleep(std::time::Duration::from_secs(1));

		let mut last_ping = std::time::Instant::now();

		loop {
			if std::time::Instant::now() - last_ping > std::time::Duration::from_secs(1) {
				if let Err(error) = client.ping() {
					if error.is_fatal() {
						panic!("{:?}", error);
					} else {
						println!("{:?}", error);
					}
				}
				last_ping = std::time::Instant::now();
			}

			if let Err(error) = client.tick().await {
				if error.is_fatal() {
					panic!("{:?}", error);
				} else {
					println!("{:?}", error);
				}
			}

			std::thread::sleep(std::time::Duration::from_millis(33));
		}
	}
}
