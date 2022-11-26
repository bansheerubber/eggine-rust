use std::collections::{ HashMap, HashSet, };
use std::collections::hash_map::Iter;
use std::net::{ Ipv6Addr, SocketAddr, };

use super::ClientConnection;
use super::server::ServerError;

/// Stores `SocketAddr` to `ClientConnection` information, along with other server-sided client related data.
#[derive(Debug, Default)]
pub(crate) struct ClientTable {
	/// Maps IP address & port to a client.
	address_to_client: HashMap<SocketAddr, ClientConnection>,
	/// If we get too many invalid packets from an IP address, add them to the blacklist so we immediately discard any
	/// additional packets from them. The blacklist blocks any communication from IP addresses, regardless of port.
	blacklist: HashSet<Ipv6Addr>,
}

impl ClientTable {
	/// Get the client using the specified address.
	/*pub(crate) fn get_client(&self, source: &SocketAddr) -> Result<&ClientConnection, ServerError> {
		match self.address_to_client.get(&source) {
    	Some(client) => Ok(client),
    	None => Err(ServerError::CouldNotFindClient),
		}
	}*/

	/// Get the client using the specified address.
	pub(crate) fn get_client_mut(&mut self, source: &SocketAddr) -> Result<&mut ClientConnection, ServerError> {
		match self.address_to_client.get_mut(&source) {
    	Some(client) => Ok(client),
    	None => Err(ServerError::CouldNotFindClient),
		}
	}

	/// Determine if a client exists in the table.
	pub(crate) fn has_client(&self, source: &SocketAddr) -> bool {
		self.address_to_client.contains_key(source)
	}

	/// Add a client to the table.
	pub(crate) fn add_client(&mut self, source: SocketAddr, client_connection: ClientConnection) {
		self.address_to_client.insert(source, client_connection);
	}

	/// Remove a client from the table.
	pub(crate) fn remove_client(&mut self, source: &SocketAddr) {
		self.address_to_client.remove(source);
	}

	/// Return an iterator over the `SocketAddr` -> `ClientConnection` mapping.
	pub(crate) fn client_iter(&self) -> Iter<'_, std::net::SocketAddr, ClientConnection> {
		self.address_to_client.iter()
	}

	/// Return a mutable iterator over the `SocketAddr` -> `ClientConnection` mapping.
	/*pub(crate) fn client_iter_mut(&mut self) -> IterMut<'_, std::net::SocketAddr, ClientConnection> {
		self.address_to_client.iter_mut()
	}*/

	/// Add an IP address to the blacklist.
	pub(crate) fn add_to_blacklist(&mut self, address: Ipv6Addr) {
		self.blacklist.insert(address);
	}

	/// Remove an IP address from the blacklist.
	/*pub(crate) fn remove_from_blacklist(&mut self, address: &Ipv6Addr) {
		self.blacklist.remove(address);
	}*/

	/// Check if an IP address is in the blacklist.
	pub(crate) fn is_in_blacklist(&self, address: &Ipv6Addr) -> bool {
		self.blacklist.contains(address)
	}
}
