//! An abstraction for sockets, communicating over the global bus.

use std::cmp::Ordering;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::{Message, WebSocketError};
use netcanv_protocol::relay;
use url::Url;
use wasm_bindgen_futures::spawn_local;

use crate::common::{deserialize_bincode, serialize_bincode};
use crate::Error;

/// Runtime for managing active connections.
pub struct SocketSystem;

impl SocketSystem {
   /// Starts the socket system.
   pub fn new() -> Arc<Self> {
      Arc::new(Self)
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
      let address = Self::resolve_address_with_default_port(&url)?;
      let ws = WebSocket::open(address.as_str()).unwrap();
      let (write, mut read) = ws.split();

      let version = read.next().await.ok_or(Error::NoVersionPacket);

      let version = match version? {
         Ok(Message::Bytes(version)) => {
            let array: [u8; 4] = version.try_into().map_err(|_| Error::InvalidVersionPacket)?;
            u32::from_le_bytes(array)
         }
         _ => return Err(Error::InvalidVersionPacket),
      };

      match version.cmp(&relay::PROTOCOL_VERSION) {
         Ordering::Equal => (),
         Ordering::Less => return Err(Error::RelayIsTooOld),
         Ordering::Greater => return Err(Error::RelayIsTooNew),
      }

      log::debug!("version ok");

      log::debug!("starting receiver loop");
      let (recv_tx, recv_rx) = mpsc::unbounded();
      spawn_local(async move {
         if let Err(error) = Socket::receiver_loop(read, recv_tx).await {
            log::error!("receiver loop error: {:?}", error);
         }
      });

      log::debug!("starting sender loop");
      let (send_tx, send_rx) = mpsc::unbounded();
      spawn_local(async move {
         if let Err(error) = Socket::sender_loop(write, send_rx).await {
            log::error!("sender loop error: {:?}", error);
         }
      });

      Ok(Socket { recv_rx, send_tx })
   }

   /// Initiates a new connection to the relay at the given hostname (IP address or DNS domain).
   pub fn connect(self: Arc<Self>, hostname: String) -> oneshot::Receiver<netcanv::Result<Socket>> {
      log::info!("connecting to {}", hostname);
      let (socket_tx, socket_rx) = oneshot::channel();
      let self2 = Arc::clone(&self);

      spawn_local(async move {
         if socket_tx.send(self2.connect_inner(hostname).await).is_err() {
            panic!("Could not send ready socket to receiver");
         }
      });

      socket_rx
   }
}

pub struct Socket {
   recv_rx: mpsc::UnboundedReceiver<relay::Packet>,
   send_tx: mpsc::UnboundedSender<relay::Packet>,
}

impl Socket {
   async fn receiver_loop(
      mut read: SplitStream<WebSocket>,
      mut output: mpsc::UnboundedSender<relay::Packet>,
   ) -> netcanv::Result<()> {
      while let Some(msg) = read.next().await {
         match msg {
            Ok(Message::Bytes(data)) => {
               if data.len() > relay::MAX_PACKET_SIZE as usize {
                  return Err(Error::ReceivedPacketThatIsTooBig);
               }
               let packet = deserialize_bincode(&data)?;
               output.send(packet).await.unwrap();
            }
            Err(e) => match e {
               WebSocketError::ConnectionClose(_) => return Ok(()),
               other => {
                  return Err(Error::WebSocket {
                     error: other.to_string(),
                  })
               }
            },
            _ => log::info!("got unused message"),
         }
      }
      log::debug!("loop receiver done");
      Ok(())
   }

   async fn write_packet(
      write: &mut SplitSink<WebSocket, Message>,
      packet: relay::Packet,
   ) -> netcanv::Result<()> {
      let bytes = serialize_bincode(&packet)?;
      if bytes.len() > relay::MAX_PACKET_SIZE as usize {
         return Err(Error::TriedToSendPacketThatIsTooBig {
            max: relay::MAX_PACKET_SIZE as usize,
            size: bytes.len(),
         });
      }
      u32::try_from(bytes.len()).map_err(|_| Error::TriedToSendPacketThatIsWayTooBig)?;

      write.send(Message::Bytes(bytes)).await.unwrap();
      Ok(())
   }

   async fn sender_loop(
      mut write: SplitSink<WebSocket, Message>,
      mut input: mpsc::UnboundedReceiver<relay::Packet>,
   ) -> netcanv::Result<()> {
      while let Some(packet) = input.next().await {
         Self::write_packet(&mut write, packet).await?;
      }
      log::debug!("sender loop done");
      Ok(())
   }

   pub fn send(&self, packet: relay::Packet) {
      let mut send_tx = self.send_tx.clone();
      spawn_local(async move {
         send_tx.send(packet).await.unwrap();
      })
   }

   pub fn recv(&mut self) -> Option<relay::Packet> {
      self.recv_rx.try_next().ok().flatten()
   }
}
