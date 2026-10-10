//! The LAN lobby: where the host waits for someone to join, and where the
//! joining player finds a game (or types the host's address) and connects.
//!
//! The network handles live in `LobbyNet`; what's on screen is described by
//! `LobbyView`. Keeping them apart means the screen only redraws when the
//! view actually changes, not every time a socket is checked.

use std::net::{SocketAddr, ToSocketAddrs};

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use super::AppState;
use super::board::Game;
use super::session::{ChosenDeck, ChosenMode, ClientLink, HostLink, PlayMode, Seat};
use super::sound::{PlaySound, Sound};
use super::widgets::*;
use crate::game::{Deck, PlayerId};
use crate::net::{
    Announcer, Browser, ClientConnection, ClientMessage, Connector, FoundHost, GAME_PORT, HostConnection, HostMessage,
    Listener, NetEvent, PROTOCOL_VERSION, local_ip, player_name,
};

/// At most this many found games are listed.
const MAX_LISTED: usize = 6;

/// The sockets and background work of the lobby. Removing this resource
/// (when leaving the lobby) stops all of it.
#[derive(Resource, Default)]
struct LobbyNet {
    // Hosting:
    listener: Option<Listener>,
    announcer: Option<Announcer>,
    host_connection: Option<HostConnection>,
    // Joining:
    browser: Option<Browser>,
    connector: Option<Connector>,
    client_connection: Option<ClientConnection>,
}

/// Everything the lobby screen shows. Redrawn whenever it changes.
#[derive(Resource, Default, Clone, PartialEq)]
struct LobbyView {
    mode: PlayMode,
    status: String,
    /// Hosting: this computer's address, to tell the other player.
    my_address: String,
    /// Joining: games found on the network.
    hosts: Vec<FoundHost>,
    /// Joining: the address typed in, and whether it's being edited.
    address: String,
    editing_address: bool,
}

#[derive(Component)]
struct LobbyItem;

#[derive(Component, Clone, Copy)]
enum LobbyButton {
    Back,
    /// One of the listed games, by position in `LobbyView::hosts`.
    FoundHost(usize),
    AddressField,
    Connect,
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Lobby), open_lobby)
            .add_systems(OnExit(AppState::Lobby), close_lobby)
            .add_systems(
                Update,
                (
                    click,
                    type_address,
                    host_update,
                    join_update,
                    redraw.run_if(resource_exists_and_changed::<LobbyView>),
                )
                    .chain()
                    .run_if(in_state(AppState::Lobby)),
            );
    }
}

fn open_lobby(mut commands: Commands, mode: Res<ChosenMode>) {
    let mut net = LobbyNet::default();
    let mut view = LobbyView { mode: mode.0, ..default() };

    match mode.0 {
        PlayMode::Host => {
            view.my_address = match local_ip() {
                Some(ip) => format!("{ip}"),
                None => "unknown (check your network settings)".to_string(),
            };
            match Listener::start(GAME_PORT) {
                Ok(listener) => {
                    net.listener = Some(listener);
                    view.status = "Waiting for another player to join...".to_string();
                }
                Err(error) => view.status = format!("Couldn't host on port {GAME_PORT}: {error}"),
            }
            // If announcing fails, the other player can still type the address.
            net.announcer = Announcer::start(player_name(), GAME_PORT).ok();
        }
        PlayMode::Join => match Browser::start() {
            Ok(browser) => {
                net.browser = Some(browser);
                view.status = "Looking for games on your network...".to_string();
            }
            Err(_) => view.status = "Can't search for games here. Type the host's address instead.".to_string(),
        },
        PlayMode::VsAi => {}
    }
    commands.insert_resource(net);
    commands.insert_resource(view);
}

fn close_lobby(mut commands: Commands) {
    commands.remove_resource::<LobbyNet>();
    commands.remove_resource::<LobbyView>();
}

/// Hosting: accept a player, check their Hello, and start the game.
fn host_update(
    mut commands: Commands,
    mut net: ResMut<LobbyNet>,
    mut view: ResMut<LobbyView>,
    deck: Res<ChosenDeck>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if view.mode != PlayMode::Host {
        return;
    }
    // Work on a copy, and only store it if something visible changed.
    let mut new_view = view.clone();

    if let Some(stream) = net.listener.as_ref().and_then(Listener::try_accept) {
        net.listener = None;
        match HostConnection::new(stream) {
            Ok(connection) => {
                net.host_connection = Some(connection);
                new_view.status = "A player connected. Checking their deck...".to_string();
            }
            Err(error) => new_view.status = format!("Connection failed: {error}"),
        }
    }

    let mut start_with: Option<Vec<crate::game::CardId>> = None;
    let mut drop_connection = false;
    if let Some(connection) = &net.host_connection {
        for event in connection.receive() {
            match event {
                NetEvent::Message(ClientMessage::Hello { version, name, deck: their_deck }) => {
                    let deck_check = Deck { name: name.clone(), cards: their_deck.clone() }.validate();
                    if version != PROTOCOL_VERSION {
                        connection.send(HostMessage::Refused("Different game version. Update both copies.".into()));
                        new_view.status = format!("{name} has a different version of the game.");
                        drop_connection = true;
                    } else if let Err(problem) = deck_check {
                        connection.send(HostMessage::Refused(format!("Your deck {problem}.")));
                        new_view.status = format!("{name}'s deck {problem}.");
                        drop_connection = true;
                    } else {
                        start_with = Some(their_deck);
                    }
                }
                NetEvent::Message(ClientMessage::Action(_)) => {}
                NetEvent::Disconnected => {
                    new_view.status = "The other player left.".to_string();
                    drop_connection = true;
                }
            }
        }
    }

    if let Some(their_deck) = start_with {
        if let Some(connection) = net.host_connection.take() {
            // The host is Player One. The game is sent to the other player
            // as soon as the board starts (see `network::host_send`).
            commands.insert_resource(Game::new(deck.0.clone(), their_deck));
            commands.insert_resource(Seat { me: PlayerId::One });
            commands.insert_resource(HostLink(connection));
            sounds.write(PlaySound(Sound::TurnStart));
            next_state.set(AppState::InGame);
        }
    } else if drop_connection {
        net.host_connection = None;
        // Listen for someone else.
        net.listener = Listener::start(GAME_PORT).ok();
    }
    view.set_if_neq(new_view);
}

/// Joining: list found games, finish connecting, and wait for the host to
/// send the game.
fn join_update(
    mut commands: Commands,
    mut net: ResMut<LobbyNet>,
    mut view: ResMut<LobbyView>,
    deck: Res<ChosenDeck>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if view.mode != PlayMode::Join {
        return;
    }
    let mut new_view = view.clone();
    if let Some(browser) = &net.browser {
        new_view.hosts = browser.hosts().into_iter().take(MAX_LISTED).collect();
    }

    if let Some(result) = net.connector.as_ref().and_then(Connector::try_finish) {
        net.connector = None;
        match result.and_then(ClientConnection::new) {
            Ok(connection) => {
                connection.send(ClientMessage::Hello {
                    version: PROTOCOL_VERSION,
                    name: player_name(),
                    deck: deck.0.clone(),
                });
                net.client_connection = Some(connection);
                new_view.status = "Connected. Waiting for the host...".to_string();
            }
            Err(error) => new_view.status = format!("Couldn't connect: {error}"),
        }
    }

    let mut first_state = None;
    let mut drop_connection = false;
    if let Some(connection) = &net.client_connection {
        for event in connection.receive() {
            match event {
                // Keep the latest, in case more than one arrived.
                NetEvent::Message(HostMessage::State(state)) => first_state = Some(*state),
                NetEvent::Message(HostMessage::Refused(reason)) => {
                    new_view.status = format!("The host said no: {reason}");
                    drop_connection = true;
                }
                NetEvent::Message(HostMessage::Rejected(_)) => {}
                NetEvent::Disconnected => {
                    new_view.status = "The host closed the connection.".to_string();
                    drop_connection = true;
                }
            }
        }
    }

    if let Some(state) = first_state {
        if let Some(connection) = net.client_connection.take() {
            // Whoever joins is Player Two.
            commands.insert_resource(Game(state));
            commands.insert_resource(Seat { me: PlayerId::Two });
            commands.insert_resource(ClientLink(connection));
            sounds.write(PlaySound(Sound::TurnStart));
            next_state.set(AppState::InGame);
        }
    } else if drop_connection {
        net.client_connection = None;
    }
    view.set_if_neq(new_view);
}

/// Turn what was typed into an address: "192.168.1.5" uses the game's
/// port, "192.168.1.5:7777" names its own.
fn parse_address(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    if let Ok(address) = text.parse::<SocketAddr>() {
        return Some(address);
    }
    (text, GAME_PORT).to_socket_addrs().ok()?.next()
}

fn click(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Cursor,
    buttons: Query<(&LobbyButton, &ButtonArea, &Transform)>,
    mut net: ResMut<LobbyNet>,
    mut view: ResMut<LobbyView>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = cursor.position() else { return };
    let button = button_under(cursor, &buttons);
    if button.is_some() {
        sounds.write(PlaySound(Sound::ButtonClick));
    }
    let mut new_view = view.clone();
    new_view.editing_address = matches!(button, Some(LobbyButton::AddressField));

    let busy = net.connector.is_some() || net.client_connection.is_some();
    let target = match button {
        Some(LobbyButton::Back) => {
            next_state.set(AppState::MainMenu);
            None
        }
        Some(LobbyButton::FoundHost(index)) if !busy => view.hosts.get(index).map(|host| host.address),
        Some(LobbyButton::Connect) if !busy => {
            let address = parse_address(&view.address);
            if address.is_none() {
                new_view.status = format!("\"{}\" isn't an address. Try something like 192.168.1.5", view.address);
            }
            address
        }
        _ => None,
    };
    if let Some(address) = target {
        net.connector = Some(Connector::start(address));
        new_view.status = format!("Connecting to {address}...");
    }
    view.set_if_neq(new_view);
}

/// Typing into the address field.
fn type_address(mut keys: MessageReader<KeyboardInput>, mut view: ResMut<LobbyView>) {
    for key in keys.read() {
        if !view.editing_address || key.state != ButtonState::Pressed {
            continue;
        }
        match key.key_code {
            KeyCode::Backspace => {
                view.address.pop();
            }
            KeyCode::Enter | KeyCode::Escape => view.editing_address = false,
            _ => {
                if let Some(typed) = &key.text {
                    // Only characters that can be part of an address.
                    let allowed = typed.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-'));
                    for character in allowed {
                        if view.address.len() < 40 {
                            view.address.push(character);
                        }
                    }
                }
            }
        }
    }
}

fn item() -> impl Bundle {
    (LobbyItem, DespawnOnExit(AppState::Lobby))
}

fn redraw(mut commands: Commands, view: Res<LobbyView>, old: Query<Entity, With<LobbyItem>>) {
    for entity in &old {
        commands.entity(entity).despawn();
    }

    let title = match view.mode {
        PlayMode::Host => "Hosting a LAN game",
        _ => "Join a LAN game",
    };
    commands.spawn((item(), text(title, 44.0, TEXT), Transform::from_xyz(0.0, 330.0, 1.0)));

    match view.mode {
        PlayMode::Host => {
            commands.spawn((
                item(),
                text(format!("Your address: {}", view.my_address), 30.0, MY_COLOR),
                Transform::from_xyz(0.0, 200.0, 1.0),
            ));
            commands.spawn((
                item(),
                text("The other player can pick your game from their list, or type this address.", 18.0, DIM_TEXT),
                Transform::from_xyz(0.0, 150.0, 1.0),
            ));
        }
        _ => {
            commands.spawn((
                item(),
                text("Games on your network:", 22.0, DIM_TEXT),
                Transform::from_xyz(0.0, 240.0, 1.0),
            ));
            if view.hosts.is_empty() {
                commands.spawn((
                    item(),
                    text("None found yet...", 18.0, NEUTRAL),
                    Transform::from_xyz(0.0, 190.0, 1.0),
                ));
            }
            for (index, host) in view.hosts.iter().enumerate() {
                let label = format!("{}   ({})", host.name, host.address.ip());
                let position = Vec2::new(0.0, 190.0 - index as f32 * 56.0);
                spawn_button(&mut commands, label, position, Vec2::new(520.0, 46.0), MY_COLOR)
                    .insert((item(), LobbyButton::FoundHost(index)));
            }

            commands.spawn((
                item(),
                text("Or type the host's address:", 20.0, DIM_TEXT),
                Transform::from_xyz(0.0, -170.0, 1.0),
            ));
            let (label, color) = if view.editing_address {
                (format!("{}|", view.address), MY_COLOR)
            } else if view.address.is_empty() {
                ("click to type, e.g. 192.168.1.5".to_string(), PANEL_LIGHT)
            } else {
                (view.address.clone(), PANEL_LIGHT)
            };
            spawn_button(&mut commands, label, Vec2::new(-80.0, -220.0), Vec2::new(380.0, 46.0), color)
                .insert((item(), LobbyButton::AddressField));
            spawn_button(&mut commands, "Connect", Vec2::new(210.0, -220.0), Vec2::new(160.0, 46.0), MY_COLOR)
                .insert((item(), LobbyButton::Connect));
        }
    }

    commands.spawn((item(), text(&view.status, 20.0, TEXT), Transform::from_xyz(0.0, -300.0, 1.0)));
    spawn_button(&mut commands, "Back", Vec2::new(0.0, -380.0), Vec2::new(200.0, 50.0), NEUTRAL)
        .insert((item(), LobbyButton::Back));
}
