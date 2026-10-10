//! Playing over a local network (LAN).
//!
//! One player *hosts*: their copy of the game runs the rules. The other
//! *joins*: it sends the host the `Action`s its player takes, and the host
//! sends back the whole `GameState` whenever it changes. That way the two
//! copies can never disagree, because only one of them makes decisions.
//!
//! This module only deals with sockets and messages, with plain Rust threads
//! and channels; the screens that use it live in `ui`.
//!
//! - `protocol.rs`: the messages the two sides send each other.
//! - `connection.rs`: a TCP connection that sends and receives messages.
//! - `discovery.rs`: finding games on the network without typing addresses.

mod connection;
mod discovery;
mod protocol;

pub use connection::{Connector, Listener, NetEvent};
pub use discovery::{Announcer, Browser, FoundHost, local_ip};
pub use protocol::{
    ClientConnection, ClientMessage, GAME_PORT, HostConnection, HostMessage, PROTOCOL_VERSION, player_name,
};
