//! An abstraction for sockets, communicating over the global bus.

use std::cmp::Ordering;
use std::sync::Arc;

use futures::channel::oneshot;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use instant::Duration;
use netcanv_protocol::relay;
use nysa::global as bus;
use url::Url;

use crate::common::{deserialize_bincode, serialize_bincode, Fatal};
use crate::Error;

/// Runtime for managing active connections.
pub struct SocketSystem {}

impl SocketSystem {
   /// Starts the socket system.
   pub fn new() -> Arc<Self> {
      Arc::new(Self {})
   }

   /// Resolves the socket addresses the given hostname could refer to.
   fn resolve_address_with_default_port(url: &str) -> netcanv::Result<Url> {
      let url = if !url.starts_with("ws://") && !url.starts_with("wss://") {
         format!("wss://{}", url)
      } else {
         url.to_owned()
      };

      let url = Url::parse(&url).map_err(|_| Error::InvalidUrl)?;

      Ok(url)
   }

   async fn connect_inner(self: Arc<Self>, url: String) -> netcanv::Result<Socket> {
      Ok(Socket {})
   }

   /// Initiates a new connection to the relay at the given hostname (IP address or DNS domain).
   pub fn connect(self: Arc<Self>, hostname: String) -> oneshot::Receiver<netcanv::Result<Socket>> {
      log::info!("connecting to {}", hostname);
      todo!()
   }
}

impl Drop for SocketSystem {
   fn drop(&mut self) {
      log::info!("cleaning up remaining sockets");
      todo!()
   }
}

pub struct Socket {}

impl Socket {
   /// Sends a packet to the receiving end of the socket.
   pub fn send(&self, packet: relay::Packet) {
      todo!()
   }

   /// Receives packets from the sending end of the socket.
   pub fn recv(&mut self) -> Option<relay::Packet> {
      todo!()
   }
}

#[derive(Clone, Debug)]
enum Signal {
   SendPong(Vec<u8>),
   Quit,
}

struct SocketQuitter {}

impl SocketQuitter {
   async fn quit(self) {
      const QUIT_TIMEOUT: Duration = Duration::from_millis(250);
      todo!()
   }
}
