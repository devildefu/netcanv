use std::io::Cursor;
use std::sync::{Arc, Mutex, Weak};

use async_std::net::{SocketAddr, TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use async_tungstenite::async_std::ConnectStream;
use async_tungstenite::tungstenite::error::CapacityError;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};

use dashmap::DashMap;

use netcanv_protocol::matchmaker::*;

const MAX_ROOM_ID: u32 = 9999;

type Rooms = DashMap<u32, Arc<Mutex<Room>>>;

struct Destination {
   sender: UnboundedSender<Message>,
   peer_addr: SocketAddr,
}

impl Destination {
   pub fn new(sender: UnboundedSender<Message>, peer_addr: SocketAddr) -> Self {
      Self { sender, peer_addr }
   }

   /// Get a reference to the destination's peer addr.
   fn peer_addr(&self) -> SocketAddr {
      self.peer_addr
   }
}

#[derive(Clone)]
#[allow(dead_code)]
struct Room {
   host: Arc<Destination>,
   clients: Vec<Weak<Destination>>,
   id: u32,
}

struct Matchmaker {
   rooms: Rooms,
   host_rooms: DashMap<SocketAddr, u32>,
   relay_clients: DashMap<SocketAddr, u32>,
}

impl Matchmaker {
   fn new() -> Self {
      Self {
         rooms: DashMap::new(),
         host_rooms: DashMap::new(),
         relay_clients: DashMap::new(),
      }
   }
}

fn find_free_room_id(rooms: &Rooms) -> Option<u32> {
   use nanorand::{Rng, WyRand};

   let mut rng = WyRand::new();

   for _ in 1..50 {
      let id = rng.generate_range(0..=MAX_ROOM_ID);
      if !rooms.contains_key(&id) {
         return Some(id);
      }
   }

   None
}

fn send_packet(dest: &Destination, packet: &Packet) -> anyhow::Result<()> {
   match &packet {
      Packet::Relayed(..) => (),
      packet => eprintln!("- sending packet {} -> {:?}", dest.peer_addr(), packet),
   }

   let sender = &dest.sender;

   // Let's make room for one kilobyte of data, usually that's all matchmaker needs,
   // and it will save some time with constant reallocation when more capacity is needed.
   let mut buf = Vec::with_capacity(1024);
   bincode::serialize_into(&mut buf, packet)?;
   sender.unbounded_send(Message::Binary(buf))?;

   Ok(())
}

fn send_error(dest: &Destination, error: &str) -> anyhow::Result<()> {
   send_packet(dest, &error_packet(error))
}

fn host(mm: Arc<Matchmaker>, dest: Arc<Destination>) -> anyhow::Result<()> {
   match find_free_room_id(&mm.rooms) {
      Some(room_id) => {
         let room = Room {
            host: dest.clone(),
            clients: Vec::new(),
            id: room_id,
         };
         {
            mm.rooms.insert(room_id, Arc::new(Mutex::new(room)));
            mm.host_rooms.insert(dest.peer_addr(), room_id);
         }
         send_packet(&dest, &Packet::RoomId(room_id))?;
      }
      None => send_error(&dest, "Could not find any more free rooms. Try again")?,
   }

   Ok(())
}

fn join(mm: Arc<Matchmaker>, dest: &Destination, room_id: u32) -> anyhow::Result<()> {
   let room = match mm.rooms.get(&room_id) {
      Some(room) => room,
      None => {
         send_error(
            dest,
            "No room found with the given ID. Check whether you spelled the ID correctly",
         )?;
         return Ok(());
      }
   };

   let room = room.lock().unwrap();

   let client_addr = dest.peer_addr();
   let host_addr = room.host.peer_addr();

   send_packet(&room.host, &Packet::ClientAddress(client_addr))?;
   send_packet(dest, &Packet::HostAddress(host_addr))
}

fn add_relay(
   mm: Arc<Matchmaker>,
   dest: Arc<Destination>,
   host_addr: Option<SocketAddr>,
) -> anyhow::Result<()> {
   let peer_addr = dest.peer_addr();
   eprintln!("- relay requested from {}", peer_addr);

   let host_addr: SocketAddr = host_addr.unwrap_or(peer_addr);

   let room_id = match mm.host_rooms.get(&host_addr) {
      Some(id) => *id,
      None => {
         send_error(&dest, "The host seems to have disconnected")?;
         return Ok(());
      }
   };

   mm.relay_clients.insert(peer_addr, room_id);
   mm.rooms.get_mut(&room_id).unwrap().lock().unwrap().clients.push(Arc::downgrade(&dest));

   // Don't forget to notify the requester that the relay is now ready.
   send_packet(&dest, &Packet::Relayed(peer_addr, vec![]))?;

   Ok(())
}

fn relay(
   mm: Arc<Matchmaker>,
   addr: SocketAddr,
   dest: &Arc<Destination>,
   to: Option<SocketAddr>,
   data: Vec<u8>, // Vec because it's moved out of the Relay packet
) -> anyhow::Result<()> {
   eprintln!("relaying packet (size: {} KiB)", data.len() as f32 / 1024.0);

   let room_id = match mm.relay_clients.get(&addr) {
      Some(id) => *id,
      None => {
         send_error(dest, "Only relay clients may send Relay packets")?;
         return Ok(());
      }
   };

   match mm.rooms.get_mut(&room_id) {
      Some(room) => {
         let mut room = room.lock().unwrap().clone();
         let mut nclients = 0;
         room.clients.retain(|client| client.upgrade().is_some());
         let packet = Packet::Relayed(addr, data);
         for client in &room.clients {
            let client = &client.upgrade().unwrap();
            if !Arc::ptr_eq(client, dest) {
               if let Some(addr) = to {
                  if client.peer_addr() != addr {
                     continue;
                  }
               }
               send_packet(client, &packet)?;
               nclients += 1;
            }
         }
         eprintln!("- relayed from {} to {} clients", addr, nclients);
      }
      None => {
         send_error(dest, "The host seems to have disconnected")?;
         return Ok(());
      }
   }

   Ok(())
}

fn incoming_packet(
   mm: Arc<Matchmaker>,
   peer_addr: SocketAddr,
   dest: Arc<Destination>,
   packet: Packet,
) -> anyhow::Result<()> {
   match &packet {
      Packet::Relay(..) => (),
      packet => eprintln!("- incoming packet: {:?}", packet),
   }

   match packet {
      Packet::Host => host(mm, dest),
      Packet::GetHost(room_id) => join(mm, &dest, room_id),
      Packet::RequestRelay(host_addr) => add_relay(mm, dest, host_addr),
      Packet::Relay(to, data) => relay(mm, peer_addr, &dest, to, data),
      _ => {
         eprintln!("! error/invalid packet: {:?}", packet);
         anyhow::bail!("Invalid packet")
      }
   }
}

fn disconnect(
   mm: Arc<Matchmaker>,
   peer_addr: SocketAddr,
   dest: Arc<Destination>,
) -> anyhow::Result<()> {
   if let Some((_, room_id)) = mm.host_rooms.remove(&peer_addr) {
      mm.rooms.remove(&room_id);
   }
   if let Some((_, room_id)) = mm.relay_clients.remove(&peer_addr) {
      if let Some(room) = mm.rooms.get_mut(&room_id) {
         let room = room.lock().unwrap();
         for client in &room.clients {
            let client = client.upgrade();
            if client.is_none() {
               continue;
            }
            let client = client.unwrap();
            if Arc::ptr_eq(&client, &dest) {
               continue;
            }
            let _ = send_packet(&client, &Packet::Disconnected(peer_addr));
         }
      }
   }
   Ok(())
}

async fn send_loop(
   mut rx: UnboundedReceiver<Message>,
   mut sink: SplitSink<WebSocketStream<ConnectStream>, Message>,
) -> anyhow::Result<()> {
   while let Some(msg) = rx.next().await {
      if let Err(e) = sink.send(msg).await {
         use async_tungstenite::tungstenite::error::Error::*;
         match e {
            ConnectionClosed => break,
            AlreadyClosed => {
               // According to the documentation this error is the fault of the programmer.
               // However, this error would crash the entire matchmaker and *all* rooms,
               // so it's better to treat it as a simple error and end the connection.
               // TODO: Use a better logger to make this error more visible
               eprintln!("! The connection has been closed, but the matchmaker is trying to work with already closed connection.");
               break;
            }
            Io(e) => {
               eprintln!("! I/O error: {:?}", e);
               break;
            },
            Tls(e) => {
               eprintln!("! TLS error: {:?}", e);
               break;
            },
            Capacity(CapacityError::TooManyHeaders) => eprintln!("! Capacity error: Too many headers"),
            Capacity(CapacityError::MessageTooLong { size, max_size }) =>
            eprintln!("! Capacity error: Message is bigger than the configured max message size (size is {} bytes, but maximum is {} bytes)", size, max_size),
            _ => {
               eprintln!("! Not handled error (report it, thanks): {:?}", e);
               break;
            },
         }
      }
   }

   Ok(())
}

async fn handle_connection(
   mm: Arc<Matchmaker>,
   stream: TcpStream,
   peer_addr: SocketAddr,
) -> anyhow::Result<()> {
   eprintln!("* mornin' mr. {}", peer_addr);

   let (sink, mut stream) = {
      let stream = async_tungstenite::accept_async(stream).await?;
      stream.split()
   };

   let (dest, rx) = {
      let (tx, rx) = unbounded();
      (Arc::new(Destination::new(tx, peer_addr)), rx)
   };

   let send = task::spawn(send_loop(rx, sink));

   'main: while let Some(msg) = stream.next().await {
      match msg {
         Ok(Message::Binary(ref data)) => {
            let mut cursor = Cursor::new(data);
            let decoded = bincode::deserialize_from(&mut cursor).or_else(|error| {
               eprintln!("! error/packet decode from {}: {}", peer_addr, error);
               Err(error)
            })?;

            incoming_packet(Arc::clone(&mm), peer_addr, Arc::clone(&dest), decoded)?;
         }
         Ok(Message::Close(frame)) => {
            eprintln!("* bye bye mr. {} it was nice to see ya", peer_addr);

            if let Some(frame) = frame {
               eprintln!("** code: {}\n** reason: {}", frame.code, frame.reason);
            }

            disconnect(Arc::clone(&mm), peer_addr, Arc::clone(&dest))?;

            // NOTE: tungstenite wants to drop the connection only when we get Error::ConnectionClosed
         }
         Ok(_) => eprintln!("Got ignored message"),
         Err(e) => {
            use async_tungstenite::tungstenite::error::Error::*;
            match e {
               ConnectionClosed => {
                  println!("zesral sie");
                  break 'main;
               },
               AlreadyClosed => {
                  // According to the documentation this error is the fault of the programmer.
                  // However, this error would crash the entire matchmaker and *all* rooms,
                  // so it's better to treat it as a simple error and end the connection.
                  // TODO: Use a better logger to make this error more visible
                  eprintln!("! The connection has been closed, but the matchmaker is trying to work with already closed connection.");
                  break 'main;
               }
               Io(e) => {
                  eprintln!("! I/O error: {:?}", e);
                  break 'main;
               },
               Tls(e) => {
                  eprintln!("! TLS error: {:?}", e);
                  break 'main;
               },
               Capacity(CapacityError::TooManyHeaders) => eprintln!("! Capacity error: Too many headers"),
               Capacity(CapacityError::MessageTooLong { size, max_size }) =>
               eprintln!("! Capacity error: Buffer capacity exhausted (got {} bytes, but maximum is {} bytes)", size, max_size),
               _ => {
                  eprintln!("! Not handled error (report it, thanks): {:?}", e);
                  break 'main;
               },
            }
         }
      }
   }

   send.await?;

   Ok(())
}

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
   F: Future<Output = anyhow::Result<()>> + Send + 'static,
{
   task::spawn(async move {
      if let Err(e) = fut.await {
         eprintln!("{}", e)
      }
   })
}

async fn async_main() -> anyhow::Result<()> {
   let mut port = DEFAULT_PORT;
   let mut args = std::env::args();
   args.next();
   if let Some(port_str) = args.next() {
      port = port_str.parse()?;
   }

   eprintln!("NetCanv Matchmaker: starting on port {}", port);

   let localhost = SocketAddr::from(([0, 0, 0, 0], port));
   let listener = TcpListener::bind(localhost).await?;

   let state = Arc::new(Matchmaker::new());

   eprintln!("Listening for incoming connections");

   while let Ok((stream, addr)) = listener.accept().await {
      spawn_and_log_error(handle_connection(state.clone(), stream, addr));
   }

   Ok(())
}

fn main() -> anyhow::Result<()> {
   task::block_on(async_main())
}
