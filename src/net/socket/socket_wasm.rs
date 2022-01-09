use std::fmt::Debug;
use std::io::Cursor;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use nysa::global as bus;
use serde::de::DeserializeOwned;
use serde::Serialize;

use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::MessageEvent;

use crate::token::Token;

/// A token for connecting a socket asynchronously.
///
/// Once a socket connects successfully, [`Connected`] is pushed onto the bus, containing this
/// token and the socket handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionToken(usize);

/// A successful connection message.
pub struct Connected<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   pub token: ConnectionToken,
   pub socket: Socket<T>,
}

/// A message pushed onto the bus when there's a new packet incoming from a socket.
#[derive(Debug)]
pub struct IncomingPacket<T>
where
   T: DeserializeOwned,
{
   pub token: ConnectionToken,
   pub data: T,
}

/// A message to the network subsystem that a packet should be sent with the given data.
#[derive(Debug)]
enum SendPacket<T>
where
   T: DeserializeOwned + Serialize,
{
   Packet(IncomingPacket<T>),
   Quit(ConnectionToken),
}

/// A trait describing a valid, (de)serializable, owned packet.
pub trait Packet: 'static + Send + DeserializeOwned + Serialize {}

/// A unique handle to a socket.
//
// These handles cannot be cloned or copied, as each handle owns a single socket thread.
// Once a handle is dropped, its associated thread is also asked to quit, and joined to the calling
// thread.
pub struct Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   token: ConnectionToken,
   system: Arc<SocketSystem<T>>,
}

impl<T> Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   /// Returns the socket's connection token.
   pub fn token(&self) -> ConnectionToken {
      self.token
   }

   /// Issues a request that a packet with the provided data should be serialized and sent over the
   /// socket.
   pub fn send(&self, data: T) {
      self.system.send(
         SendPacket::Packet(IncomingPacket {
            data,
            token: self.token,
         }),
         self.token,
      );
   }
}

impl<T> Drop for Socket<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   fn drop(&mut self) {
      self.system.send(SendPacket::Quit::<T>(self.token), self.token);
   }
}

/// A socket handling subsystem for the given packet type `T`.
pub struct SocketSystem<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   inner: Mutex<SocketSystemInner<T>>,
}

static CONNECTION_TOKEN: Token = Token::new();

impl<T> SocketSystem<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   pub fn new() -> Arc<Self> {
      Arc::new(Self {
         inner: Mutex::new(SocketSystemInner::new()),
      })
   }

   fn send(&self, packet: SendPacket<T>, token: ConnectionToken) {
      let inner = self.inner.lock().unwrap();
      inner.send(packet, token);
   }

   fn resolve_address_with_default_port(
      address: &str,
      default_port: u16,
   ) -> anyhow::Result<url::Url> {
      let url = url::Url::parse(address)?;

      // if let None = url.port() {
      //    // Url::set_port on Error does nothing, so it is fine to ignore it
      //    #[allow(unused_must_use)]
      //    {
      //       url.set_port(Some(default_port));
      //    }
      // }

      Ok(url)
   }

   pub fn connect(
      self: &Arc<Self>,
      address: String,
      default_port: u16,
   ) -> anyhow::Result<ConnectionToken> {
      let token = ConnectionToken(CONNECTION_TOKEN.next());

      let this = Arc::clone(self);
      {
         {
            let mut inner = this.inner.lock().unwrap();
            let address = Self::resolve_address_with_default_port(&address, default_port)?;
            inner.connect(token, &address)?;
         }

         let socket = Socket {
            token,
            system: this,
         };
         bus::push(Connected { token, socket });
      }

      Ok(token)
   }
}

/// The inner, non thread-safe data of `SocketSystem`.
struct SocketSystemInner<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   _phantom_data: PhantomData<T>,
}

impl<T> SocketSystemInner<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   // This "inner" version of SocketSystem contains methods that operate on the inner vec of socket
   // threads. These are "raw" versions of the public, safe API.

   fn new() -> Self {
      Self {
         _phantom_data: PhantomData,
      }
   }

   fn send(&self, packet: SendPacket<T>, token: ConnectionToken) {
      match packet {
         // Serialize and send the packet.
         SendPacket::Packet(packet) if packet.token == token => {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            catch!(bincode::serialize_into(&mut cursor, &packet.data));

            let array = Uint8Array::new_with_length(buf.len() as _);
            array.copy_from(&buf);

            send(array, token.0);
         }
         // Quit when the owning socket is dropped.
         SendPacket::Quit(quit_token) if quit_token == token => {
            close(quit_token.0);
         }
         _ => (),
      }
   }

   fn connect(&mut self, token: ConnectionToken, address: impl AsRef<str>) -> anyhow::Result<()> {
      connect(token.0, address.as_ref());

      Ok(())
   }
}

#[wasm_bindgen(raw_module = "../www/index.js")]
extern "C" {
   #[wasm_bindgen]
   fn connect(token: usize, address: &str);

   #[wasm_bindgen]
   fn send(data: Uint8Array, token: usize);

   #[wasm_bindgen]
   fn close(token: usize);
}

#[wasm_bindgen]
pub fn receive(event: MessageEvent, token: usize) {
   use netcanv_protocol::matchmaker as mm;

   if let Ok(abuf) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
      let array = js_sys::Uint8Array::new(&abuf).to_vec();
      let mut cursor = Cursor::new(array);

      let data: mm::Packet = catch!(bincode::deserialize_from(&mut cursor));
      bus::push(IncomingPacket {
         token: ConnectionToken(token),
         data,
      });
   }
}
