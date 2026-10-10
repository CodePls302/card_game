//! A TCP connection that sends and receives whole messages.
//!
//! Each message is turned into one line of JSON. Two background threads do
//! the slow work so the game never waits on the network: one writes
//! outgoing messages, the other reads incoming ones. The game talks to them
//! through channels (queues that are safe to share between threads).

use std::io::{self, BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::marker::PhantomData;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Something that arrived from the other side.
pub enum NetEvent<T> {
    Message(T),
    /// The other side closed the connection, or it broke.
    Disconnected,
}

/// One end of a connection that sends `Out` messages and receives `In`
/// messages. Dropping it closes the connection.
pub struct Connection<Out, In> {
    outgoing: Sender<Out>,
    /// A `Receiver` can't be shared between threads on its own; the `Mutex`
    /// makes it safe, which Bevy requires of resources.
    incoming: Mutex<Receiver<NetEvent<In>>>,
    /// Kept so that dropping the connection can close the socket, which also
    /// stops the reading thread.
    stream: TcpStream,
    _types: PhantomData<fn(Out, In)>,
}

impl<Out, In> Connection<Out, In>
where
    Out: Serialize + Send + 'static,
    In: DeserializeOwned + Send + 'static,
{
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        // Send small messages straight away instead of batching them.
        stream.set_nodelay(true)?;
        let (outgoing, to_send) = mpsc::channel::<Out>();
        let (received, incoming) = mpsc::channel::<NetEvent<In>>();

        let writer_stream = stream.try_clone()?;
        thread::spawn(move || {
            let mut writer = BufWriter::new(writer_stream);
            // Ends when the `Connection` (and so its `Sender`) is dropped.
            for message in to_send {
                let line = match serde_json::to_string(&message) {
                    Ok(line) => line,
                    Err(error) => {
                        eprintln!("Couldn't encode a message: {error}");
                        continue;
                    }
                };
                if writeln!(writer, "{line}").and_then(|_| writer.flush()).is_err() {
                    break;
                }
            }
        });

        let reader_stream = stream.try_clone()?;
        thread::spawn(move || {
            for line in BufReader::new(reader_stream).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str(&line) {
                    Ok(message) => {
                        if received.send(NetEvent::Message(message)).is_err() {
                            return; // The game no longer listens.
                        }
                    }
                    Err(error) => eprintln!("Couldn't read a message: {error}"),
                }
            }
            let _ = received.send(NetEvent::Disconnected);
        });

        Ok(Connection { outgoing, incoming: Mutex::new(incoming), stream, _types: PhantomData })
    }

    /// Queue a message to be sent. Never waits.
    pub fn send(&self, message: Out) {
        // If the writing thread has stopped, the reading side will report
        // the disconnection, so the error can be ignored here.
        let _ = self.outgoing.send(message);
    }

    /// Everything that has arrived since the last call. Never waits.
    pub fn receive(&self) -> Vec<NetEvent<In>> {
        match self.incoming.lock() {
            Ok(incoming) => incoming.try_iter().collect(),
            Err(_) => vec![NetEvent::Disconnected],
        }
    }
}

impl<Out, In> Drop for Connection<Out, In> {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// Waits in the background for one player to connect to the host.
pub struct Listener {
    connected: Mutex<Receiver<TcpStream>>,
    stop: Arc<AtomicBool>,
}

impl Listener {
    pub fn start(port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        // Don't block on `accept`, so the thread can notice `stop`.
        listener.set_nonblocking(true)?;
        let (found, connected) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if stream.set_nonblocking(false).is_ok() {
                            let _ = found.send(stream);
                        }
                        return;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(error) => {
                        eprintln!("Couldn't accept a connection: {error}");
                        return;
                    }
                }
            }
        });
        Ok(Listener { connected: Mutex::new(connected), stop })
    }

    /// The new connection, once someone has connected.
    pub fn try_accept(&self) -> Option<TcpStream> {
        self.connected.lock().ok()?.try_recv().ok()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Connects to a host in the background, so the screen doesn't freeze if
/// the address doesn't answer.
pub struct Connector {
    result: Mutex<Receiver<io::Result<TcpStream>>>,
}

impl Connector {
    pub fn start(address: SocketAddr) -> Self {
        let (done, result) = mpsc::channel();
        thread::spawn(move || {
            let _ = done.send(TcpStream::connect_timeout(&address, Duration::from_secs(4)));
        });
        Connector { result: Mutex::new(result) }
    }

    /// The outcome, once the attempt has finished.
    pub fn try_finish(&self) -> Option<io::Result<TcpStream>> {
        self.result.lock().ok()?.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::game::{Action, GameState, Target, Zone, all_cards};
    use crate::net::{ClientConnection, ClientMessage, HostConnection, HostMessage};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// Wait (up to a few seconds) for something to arrive.
    fn wait_for<T>(mut poll: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(value) = poll() {
                return value;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "nothing arrived");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn host_and_client_exchange_messages() {
        // Port 0 asks the system for any free port.
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = ClientConnection::new(TcpStream::connect(address).unwrap()).unwrap();
        let host = HostConnection::new(listener.accept().unwrap().0).unwrap();

        let action = Action::PlayCard { card: crate::game::CardId(4), to: Target::Zone(Zone::B) };
        client.send(ClientMessage::Action(action));
        let received = wait_for(|| host.receive().into_iter().next());
        assert!(matches!(received, NetEvent::Message(ClientMessage::Action(a)) if a == action));

        // A whole game state makes it across too.
        let game = GameState::new(all_cards(), all_cards(), &mut StdRng::seed_from_u64(1));
        host.send(HostMessage::State(Box::new(game.clone())));
        let received = wait_for(|| client.receive().into_iter().next());
        let NetEvent::Message(HostMessage::State(state)) = received else { panic!("expected a state") };
        assert_eq!(state.player(crate::game::PlayerId::Two).hand, game.player(crate::game::PlayerId::Two).hand);

        // Dropping one end is seen as a disconnection by the other.
        drop(host);
        let received = wait_for(|| client.receive().into_iter().next());
        assert!(matches!(received, NetEvent::Disconnected));
    }
}
