//! Finding games on the local network.
//!
//! A host shouts "I'm hosting a game" once a second as a UDP *broadcast*,
//! which every computer on the local network receives. Joining players
//! listen for those shouts and list the hosts they hear.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The UDP port announcements are sent to.
pub const DISCOVERY_PORT: u16 = 7778;
/// Every announcement starts with this, so stray packets are ignored.
const MAGIC: &str = "CARDGAME1";
/// Hosts not heard from for this long are dropped from the list.
const FORGET_AFTER: Duration = Duration::from_secs(4);

/// Announces a hosted game until dropped.
pub struct Announcer {
    stop: Arc<AtomicBool>,
}

impl Announcer {
    pub fn start(name: String, game_port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", 0))?;
        socket.set_broadcast(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        thread::spawn(move || {
            let message = format!("{MAGIC} {game_port} {name}");
            while !stop_flag.load(Ordering::Relaxed) {
                // To everyone on the network, and to this computer itself so
                // two copies of the game on one machine can find each other.
                let _ = socket.send_to(message.as_bytes(), (Ipv4Addr::BROADCAST, DISCOVERY_PORT));
                let _ = socket.send_to(message.as_bytes(), (Ipv4Addr::LOCALHOST, DISCOVERY_PORT));
                thread::sleep(Duration::from_secs(1));
            }
        });
        Ok(Announcer { stop })
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A game someone is hosting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundHost {
    pub name: String,
    pub address: SocketAddr,
}

/// Listens for announcements until dropped.
pub struct Browser {
    /// Each host, and when it was last heard from. Shared with the
    /// listening thread, hence `Arc<Mutex<..>>`.
    heard: Arc<Mutex<HashMap<SocketAddr, (FoundHost, Instant)>>>,
    stop: Arc<AtomicBool>,
}

impl Browser {
    pub fn start() -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))?;
        // Wake up regularly to check `stop`.
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        let heard = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (heard_by_thread, stop_flag) = (heard.clone(), stop.clone());

        thread::spawn(move || {
            let mut buffer = [0u8; 512];
            while !stop_flag.load(Ordering::Relaxed) {
                let Ok((length, sender)) = socket.recv_from(&mut buffer) else { continue };
                let Some(host) = parse(&buffer[..length], sender.ip()) else { continue };
                if let Ok(mut heard) = heard_by_thread.lock() {
                    heard.insert(host.address, (host, Instant::now()));
                }
            }
        });
        Ok(Browser { heard, stop })
    }

    /// The hosts heard from recently, sorted by name.
    pub fn hosts(&self) -> Vec<FoundHost> {
        let Ok(heard) = self.heard.lock() else { return Vec::new() };
        let mut hosts: Vec<FoundHost> = heard
            .values()
            .filter(|(_, last_heard)| last_heard.elapsed() < FORGET_AFTER)
            .map(|(host, _)| host.clone())
            .collect();
        hosts.sort_by(|a, b| (&a.name, a.address).cmp(&(&b.name, b.address)));
        hosts
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Read an announcement like "CARDGAME1 7777 my-laptop".
fn parse(bytes: &[u8], sender: IpAddr) -> Option<FoundHost> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut parts = text.splitn(3, ' ');
    if parts.next()? != MAGIC {
        return None;
    }
    let port: u16 = parts.next()?.parse().ok()?;
    let name = parts.next().unwrap_or("Unknown").to_string();
    Some(FoundHost { name, address: SocketAddr::new(sender, port) })
}

/// This computer's address on the local network, to show to the other
/// player. Connecting a UDP socket sends nothing; it only makes the system
/// pick which network interface it would use.
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.168.0.1", 9)).ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announcements_are_parsed() {
        let sender: IpAddr = "192.168.1.20".parse().unwrap();
        let host = parse(b"CARDGAME1 7777 my laptop", sender).unwrap();
        assert_eq!(host.name, "my laptop");
        assert_eq!(host.address, "192.168.1.20:7777".parse().unwrap());
        assert_eq!(parse(b"SOMETHING ELSE", sender), None);
    }
}
