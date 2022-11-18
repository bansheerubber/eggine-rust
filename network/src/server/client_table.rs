use std::collections::HashMap;
use std::collections::hash_map::{ Iter, IterMut, };
use std::net::SocketAddr;

use super::ClientConnection;
use super::server::ServerError;

/// Stores `SocketAddr` to `ClientConnection` information, along with other server-sided client related data.
#[derive(Debug, Default)]
pub(crate) struct ClientTable {
	/// Maps IP address & port to a client.
	address_to_client: HashMap<SocketAddr, ClientConnection>,
}

impl ClientTable {
	pub(crate) fn get_client(&self, source: &SocketAddr) -> Result<&ClientConnection, ServerError> {
		match self.address_to_client.get(&source) {
    	Some(client) => Ok(client),
    	None => Err(ServerError::CouldNotFindClient),
		}
	}

	pub(crate) fn get_client_mut(&mut self, source: &SocketAddr) -> Result<&mut ClientConnection, ServerError> {
		match self.address_to_client.get_mut(&source) {
    	Some(client) => Ok(client),
    	None => Err(ServerError::CouldNotFindClient),
		}
	}

	pub(crate) fn has_client(&self, source: &SocketAddr) -> bool {
		self.address_to_client.contains_key(source)
	}

	pub(crate) fn add_client(&mut self, source: SocketAddr, client_connection: ClientConnection) {
		self.address_to_client.insert(source, client_connection);
	}

	pub(crate) fn remove_client(&mut self, source: &SocketAddr) {
		self.address_to_client.remove(source);
	}

	pub(crate) fn client_iter(&self) -> Iter<'_, std::net::SocketAddr, ClientConnection> {
		self.address_to_client.iter()
	}

	pub(crate) fn client_iter_mut(&mut self) -> IterMut<'_, std::net::SocketAddr, ClientConnection> {
		self.address_to_client.iter_mut()
	}
}
