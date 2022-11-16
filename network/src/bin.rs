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
		let server = Server::new(last_argument).unwrap();

		loop {

		}
	} else {
		let client = Client::new(last_argument).unwrap();
	}
}
