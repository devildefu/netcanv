//! An abstraction for sockets, communicating over the global bus.

use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use netcanv_protocol::relay;
use nysa::global as bus;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::common::{deserialize_bincode, serialize_bincode, Fatal};
use crate::Error;

/// Runtime for managing active connections.
pub struct SocketSystem {
   quitters: Mutex<Vec<SocketQuitter>>,
}

impl SocketSystem {
   /// Starts the socket system.
   pub fn new() -> Arc<Self> {
      Arc::new(Self {
         quitters: Mutex::new(Vec::new()),
      })
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

   fn connect_inner(self: Arc<Self>, url: String) -> netcanv::Result<Socket> {
      let address = Self::resolve_address_with_default_port(&url)?;
      let socket = Socket::new(address);

      // INSERT VERSION CHECK HERE

      let mut quitters = self.quitters.lock().unwrap();
      quitters.push(SocketQuitter {
         socket: socket.inner(),
      });

      Ok(socket)
   }

   /// Initiates a new connection to the relay at the given hostname (IP address or DNS domain).
   pub fn connect(self: Arc<Self>, hostname: String) -> oneshot::Receiver<netcanv::Result<Socket>> {
      log::info!("connecting to {}", hostname);
      let (socket_tx, socket_rx) = oneshot::channel();
      let self2 = Arc::clone(&self);

      if socket_tx.send(self2.connect_inner(hostname)).is_err() {
         panic!("Could not send ready socket to receiver");
      }

      socket_rx
   }
}

impl Drop for SocketSystem {
   fn drop(&mut self) {
      log::info!("cleaning up remaining sockets");
      let mut handles = self.quitters.lock().unwrap();
      for handle in handles.drain(..) {
         handle.quit();
      }
   }
}

pub struct Socket {
   inner: Rc<SocketImpl>,
}

impl Socket {
   fn new(address: Url) -> Self {
      Self {
         inner: Rc::new(SocketImpl::new(address.as_str())),
      }
   }

   fn inner(&self) -> Rc<SocketImpl> {
      Rc::clone(&self.inner)
   }

   /// Sends a packet to the receiving end of the socket.
   pub fn send(&self, packet: relay::Packet) {
      let bytes = serialize_bincode(&packet).unwrap();
      if bytes.len() > relay::MAX_PACKET_SIZE as usize {
         panic!(
            "Tried to send packet that is too big, max: {}, size: {}",
            relay::MAX_PACKET_SIZE,
            bytes.len()
         );
      }
      u32::try_from(bytes.len()).unwrap();

      self.inner.send(&bytes);
   }

   /// Receives packets from the sending end of the socket.
   pub fn recv(&mut self) -> Option<relay::Packet> {
      let data = self.inner.recv()?;
      log::debug!("{:?}", data);

      if data.len() > relay::MAX_PACKET_SIZE as usize {
         panic!("Received packet that is too big");
      }

      let packet = deserialize_bincode(&data).unwrap();
      Some(packet)
   }
}

struct SocketQuitter {
   socket: Rc<SocketImpl>,
}

impl SocketQuitter {
   fn quit(self) {
      self.socket.quit();
   }
}

#[wasm_bindgen(raw_module = "socket")]
extern "C" {
   type SocketImpl;

   #[wasm_bindgen(constructor)]
   fn new(address: &str) -> SocketImpl;

   #[wasm_bindgen(method)]
   fn send(this: &SocketImpl, data: &[u8]);

   #[wasm_bindgen(method)]
   fn recv(this: &SocketImpl) -> Option<Box<[u8]>>;

   #[wasm_bindgen(method)]
   fn quit(this: &SocketImpl);
}

#[wasm_bindgen(js_name = checkVersion)]
pub fn check_version(buffer: Box<[u8]>) -> bool {
   if buffer.len() > 4 {
      return false;
   }

   let buffer: [u8; 4] = buffer[0..4].try_into().unwrap();
   let version = u32::from_le_bytes(buffer);

   match version.cmp(&relay::PROTOCOL_VERSION) {
      Ordering::Equal => true,
      Ordering::Less | Ordering::Greater => false,
   }
}
