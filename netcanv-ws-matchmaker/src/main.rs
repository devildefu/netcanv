use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex, Weak};

use async_std::net::{SocketAddr, TcpListener, TcpStream};
use async_std::task;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};

use netcanv_protocol::matchmaker::*;

const MAX_ROOM_ID: u32 = 9999;

type Rooms = HashMap<u32, Arc<Mutex<Room>>>;

struct Destination {
   sender: Mutex<UnboundedSender<Message>>,
   peer_addr: SocketAddr,
}

impl Destination {
   pub fn new(sender: UnboundedSender<Message>, peer_addr: SocketAddr) -> Self {
      Self {
         sender: Mutex::new(sender),
         peer_addr,
      }
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
   host_rooms: HashMap<SocketAddr, u32>,
   relay_clients: HashMap<SocketAddr, u32>,
}

impl Matchmaker {
   fn new() -> Self {
      Self {
         rooms: HashMap::new(),
         host_rooms: HashMap::new(),
         relay_clients: HashMap::new(),
      }
   }
}

fn find_free_room_id(rooms: &Rooms) -> Option<u32> {
   use rand::Rng;
   let mut rng = rand::thread_rng();
   for _ in 1..50 {
      let id = rng.gen_range(0..=MAX_ROOM_ID);
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

   let sender = dest.sender.lock().unwrap();

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

fn host(mm: Arc<Mutex<Matchmaker>>, dest: Arc<Destination>) -> anyhow::Result<()> {
   let mut mm = mm.lock().unwrap();
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

fn join(mm: Arc<Mutex<Matchmaker>>, dest: &Destination, room_id: u32) -> anyhow::Result<()> {
   let mm = mm.lock().unwrap();
   let room = match mm.rooms.get(&room_id) {
      Some(room) => room,
      None => {
         send_error(
            dest,
            "No room found with the given ID. Check whether you spelled the ID correctly",
         )?;
         return Ok(());
      }
   }
   .lock()
   .unwrap();
   let client_addr = dest.peer_addr();
   let host_addr = room.host.peer_addr();
   send_packet(&room.host, &Packet::ClientAddress(client_addr))?;
   send_packet(dest, &Packet::HostAddress(host_addr))
}

fn add_relay(
   mm: Arc<Mutex<Matchmaker>>,
   dest: Arc<Destination>,
   host_addr: Option<SocketAddr>,
) -> anyhow::Result<()> {
   let peer_addr = dest.peer_addr();
   eprintln!("- relay requested from {}", peer_addr);

   let host_addr: SocketAddr = host_addr.unwrap_or(peer_addr);
   let mut mm = mm.lock().unwrap();
   let room_id: u32;
   match mm.host_rooms.get(&host_addr) {
      Some(id) => room_id = *id,
      None => {
         send_error(&dest, "The host seems to have disconnected")?;
         return Ok(());
      }
   }
   mm.relay_clients.insert(peer_addr, room_id);
   mm.rooms.get_mut(&room_id).unwrap().lock().unwrap().clients.push(Arc::downgrade(&dest));

   // Don't forget to notify the requester that the relay is now ready.
   send_packet(&dest, &Packet::Relayed(peer_addr, vec![]))?;

   Ok(())
}

fn relay(
   mm: Arc<Mutex<Matchmaker>>,
   addr: SocketAddr,
   dest: &Arc<Destination>,
   to: Option<SocketAddr>,
   data: Vec<u8>, // Vec because it's moved out of the Relay packet
) -> anyhow::Result<()> {
   eprintln!("relaying packet (size: {} KiB)", data.len() as f32 / 1024.0);
   let mut mm = mm.lock().unwrap();
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
         drop(mm);
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
   mm: Arc<Mutex<Matchmaker>>,
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

async fn send_loop(
   mut rx: UnboundedReceiver<Message>,
   mut sink: SplitSink<WebSocketStream<TcpStream>, Message>,
) -> anyhow::Result<()> {
   while let Some(msg) = rx.next().await {
      sink.send(msg).await?;
   }

   Ok(())
}

async fn handle_connection(
   mm: Arc<Mutex<Matchmaker>>,
   stream: TcpStream,
   peer_addr: SocketAddr,
) -> anyhow::Result<()> {
   eprintln!("* mornin' mr. {}", peer_addr);
   // let (sender, receiver) = unbounded();
   let (sink, mut stream) = {
      let stream = async_tungstenite::accept_async(stream).await?;
      stream.split()
   };

   let (dest, rx) = {
      let (tx, rx) = unbounded();
      (Arc::new(Destination::new(tx, peer_addr)), rx)
   };

   task::spawn(send_loop(rx, sink));

   while let Some(msg) = stream.next().await {
      let msg = msg?;
      match msg {
         Message::Binary(ref data) => {
            let mut cursor = Cursor::new(data);
            let decoded = bincode::deserialize_from(&mut cursor)?;
            incoming_packet(mm.clone(), peer_addr, Arc::clone(&dest), decoded)?;
         }
         Message::Close(_) => {
            eprintln!("* bye bye mr. {} it was nice to see ya", peer_addr);
            break;
         }
         _ => eprintln!("Got ignored message"),
      }
   }

   Ok(())
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

   let state = Arc::new(Mutex::new(Matchmaker::new()));

   eprintln!("Listening for incoming connections");

   while let Ok((stream, addr)) = listener.accept().await {
      task::spawn(handle_connection(state.clone(), stream, addr));
   }

   Ok(())
}

fn main() -> anyhow::Result<()> {
   task::block_on(async_main())
}
