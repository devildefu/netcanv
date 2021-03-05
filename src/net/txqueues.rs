use std::collections::HashMap;
use std::net::SocketAddr;

use crossbeam_channel::{Sender};
use laminar::Packet;
use serde::Serialize;
use thiserror::Error;

pub struct SendQueue<T: Serialize> {
    queued_packets: HashMap<SocketAddr, Vec<T>>,
}

#[derive(Error, Debug)]
pub enum SendError {
    #[error("Couldn't serialize a queue")]
    Serialize(#[from] bincode::Error),
    #[error("The sender is closed")]
    Send,
}

impl<T: Serialize> SendQueue<T> {

    pub fn new() -> Self {
        Self {
            queued_packets: HashMap::new(),
        }
    }

    pub fn enqueue(&mut self, dest_addr: SocketAddr, packet: T) {
        if !self.queued_packets.contains_key(&dest_addr) {
            self.queued_packets.insert(dest_addr, Vec::new());
        }
        self.queued_packets.get_mut(&dest_addr).unwrap().push(packet);
    }

    pub fn send(&mut self, tx: &Sender<Packet>) -> Result<(), SendError> {
        for (addr, queue) in &mut self.queued_packets {
            let payload = bincode::serialize(queue)?;
            let packet = Packet::reliable_ordered(*addr, payload, None);
            tx.send(packet).map_err(|_| SendError::Send)?;
            queue.clear();
        }
        Ok(())
    }

}
