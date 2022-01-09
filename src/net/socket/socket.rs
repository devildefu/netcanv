//! An abstraction for sockets, communicating over the global bus.

use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Cursor;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use nysa::global as bus;
use serde::de::DeserializeOwned;
use serde::Serialize;

use async_std::net::TcpStream;
use async_std::task::{self, JoinHandle};
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use async_tungstenite::async_std::ConnectStream;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::stream::{SplitSink, SplitStream};
use futures::{future, SinkExt, StreamExt};

use crate::common::Fatal;
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

      // Wait for each send loop to complete, otherwise netcanv will close too quickly,
      // and the matchmaker will not end the connection
      let mut inner = self.system.inner.lock().unwrap();
      task::block_on(async {
         inner.wait().await;
      });
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
      let mut url = url::Url::parse(&format!("ws://{}", address))?;

      if let None = url.port() {
         // Url::set_port on Error does nothing, so it is fine to ignore it
         #[allow(unused_must_use)]
         {
            url.set_port(Some(default_port));
         }
      }

      Ok(url)
   }

   pub fn connect(
      self: &Arc<Self>,
      address: String,
      default_port: u16,
   ) -> anyhow::Result<ConnectionToken> {
      let token = ConnectionToken(CONNECTION_TOKEN.next());

      let this = Arc::clone(self);
      task::spawn(async move {
         {
            let mut inner = this.inner.lock().unwrap();
            let address = catch!(Self::resolve_address_with_default_port(
               &address,
               default_port
            ));
            catch!(inner.connect(token, &address));
         }

         let socket = Socket {
            token,
            system: this,
         };
         bus::push(Connected { token, socket });
      });

      Ok(token)
   }
}

/// A socket slot containing join handles for the receiving and sending thread, respectively,
/// and sender to communicate with the send loop.
struct Slot<T: DeserializeOwned + Serialize> {
   receiving_task: JoinHandle<()>,
   sending_task: JoinHandle<()>,
   sender: UnboundedSender<SendPacket<T>>,
}

/// The inner, non thread-safe data of `SocketSystem`.
struct SocketSystemInner<T>
where
   T: 'static + Send + DeserializeOwned + Serialize + Debug,
{
   socket_threads: HashMap<ConnectionToken, Option<Slot<T>>>,
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
         socket_threads: HashMap::new(),
         _phantom_data: PhantomData,
      }
   }

   fn send(&self, packet: SendPacket<T>, token: ConnectionToken) {
      if let Some(Some(Slot { sender, .. })) = self.socket_threads.get(&token) {
         if let Err(e) = sender.unbounded_send(packet) {
            bus::push(Fatal(anyhow::anyhow!("internal error")));
            log::info!("{:?}", e);
         }
      }
   }

   async fn receive_loop(
      mut stream: SplitStream<WebSocketStream<ConnectStream>>,
      token: ConnectionToken,
   ) {
      use async_tungstenite::tungstenite::{error::ProtocolError, Error as WsError};
      while let Some(msg) = stream.next().await {
         match msg {
            Ok(Message::Binary(ref data)) => {
               let mut cursor = Cursor::new(data);

               let data: T = catch!(bincode::deserialize_from(&mut cursor));
               bus::push(IncomingPacket { token, data });
            }
            Ok(Message::Close(_)) => {
               break;
            }
            Err(WsError::Protocol(ProtocolError::ResetWithoutClosingHandshake)) => {
               bus::push(Fatal(anyhow::anyhow!("Matchmaker has been closed")));
            }
            _ => log::info!("Got {:?}, ignored", msg),
         }
      }

      println!("receive loop done");
   }

   async fn send_loop(
      mut rx: UnboundedReceiver<SendPacket<T>>,
      mut sink: SplitSink<WebSocketStream<ConnectStream>, Message>,
      token: ConnectionToken,
   ) {
      'send: while let Some(message) = rx.next().await {
         // send() and close() have the same errors, so we can put them in the same if
         if let Err(e) = match message {
            SendPacket::Packet(packet) if packet.token == token => {
               let mut buf = vec![];
               let mut cursor = Cursor::new(&mut buf);
               catch!(bincode::serialize_into(&mut cursor, &packet.data));

               sink.send(Message::Binary(buf)).await
            }
            SendPacket::Quit(quit_token) if quit_token == token => {
               // If there was an error when closing, we need to pass it on,
               // if not, we can just exit the loop
               if let Err(e) = sink.close().await {
                  Err(e)
               } else {
                  break 'send;
               }
            }
            _ => Ok(()),
         } {
            match e {
               _ => bus::push(Fatal(anyhow::anyhow!(
                  "Not handled connection error: {:?}",
                  e
               ))),
            }
         }
      }

      println!("send loop done");
   }

   async fn async_connect(
      address: impl AsRef<str>,
      token: ConnectionToken,
   ) -> anyhow::Result<Slot<T>> {
      let address = address.as_ref();
      println!("{}", address);

      // Connect to matchmaker
      let (sink, stream) = {
         let (stream, _) = async_tungstenite::async_std::connect_async(address).await?;
         let (sink, stream) = stream.split();
         (sink, stream)
      };

      // Channel for sending data to matchmaker
      // Sender is for Socket<T>, and Receiver is for send loop
      let (sender, receiver) = {
         let (tx, rx) = unbounded();
         (tx, rx)
      };

      let receiving_task = task::spawn(Self::receive_loop(stream, token));
      let sending_task = task::spawn(Self::send_loop(receiver, sink, token));

      Ok(Slot {
         receiving_task,
         sending_task,
         sender,
      })
   }

   fn connect(&mut self, token: ConnectionToken, address: impl AsRef<str>) -> anyhow::Result<()> {
      let Slot {
         receiving_task,
         sending_task,
         sender,
      } = task::block_on(Self::async_connect(address, token))?;

      self.socket_threads.insert(
         token,
         Some(Slot {
            receiving_task,
            sending_task,
            sender,
         }),
      );

      Ok(())
   }

   pub async fn wait(&mut self) {
      // Take all senders
      let senders = self.socket_threads.iter_mut().filter_map(|(_, slot)| {
         if let Some(slot) = slot.take() {
            Some(slot.sending_task)
         } else {
            None
         }
      });

      // Combine all senders into one future
      future::select_all(senders).await;
   }
}