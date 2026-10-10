use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{Justify, TextLayout};

use super::animate::{Animation, AnimationKind};
use super::layout::*;
use super::playback::{Playback, caption};
use super::{CardPopup, Game, GraveyardView, TurnTimer};
use crate::game::{
    CardId, Effect, EffectKind, GameEvent, GameState, MAX_TURNS, Outcome, Phase, PlacedCard, PlayerId, Zone, card,
    energy_for_turn,
};
use crate::ui::AppState;
use crate::ui::session::{ConnectionLost, Seat};
use crate::ui::widgets::*;

const SECRET_BACKGROUND: Color = Color::srgb(0.16, 0.12, 0.24);
const SECRET_LABEL_BACKGROUND: Color = Color::srgb(0.28, 0.18, 0.40);
const MISSION_DONE: Color = Color::srgb(0.35, 0.85, 0.45);
/// Your card played this turn, not yet revealed to the opponent.
const PENDING_BORDER: Color = Color::srgb(1.0, 0.8, 0.2);
/// Your face-down secret card.
const SECRET_BORDER: Color = Color::srgb(0.7, 0.45, 1.0);
/// A card you can drag to another zone right now.
const MOVABLE_BORDER: Color = Color::srgb(0.35, 0.95, 0.55);
/// A card that is being destroyed.
const DESTROYED_BORDER: Color = Color::srgb(0.9, 0.15, 0.15);
const TIMER_WARNING: Color = Color::srgb(1.0, 0.35, 0.3);
/// Below this many seconds left, the timer turns red.
const TIMER_WARNING_SECONDS: f32 = 20.0;

/// Marks things that get deleted and redrawn whenever the game changes.
/// Despawning an entity also despawns its children, so only the top-level
/// entity of each card needs this.
#[derive(Component)]
pub struct Dynamic;

/// The turn countdown text, updated every frame without a full redraw.
#[derive(Component)]
pub struct TimerText;

/// A card the local player can pick up, and where it comes from.
#[derive(Component, Clone, Copy)]
pub enum Draggable {
    FromHand(CardId),
    /// A face-up card that can move to another zone (Cloud Whale).
    OnBoard {
        uid: u32,
        from: Zone,
    },
    /// A card played this turn, which can go back to the hand.
    Pending {
        uid: u32,
    },
}

/// What the details popup shows about a card. Every card you can see gets
/// one, so clicking it can open the popup.
#[derive(Component, Clone)]
pub struct CardDetails {
    pub card: CardId,
    /// Current power, including bonuses.
    pub power: i32,
    pub owner: PlayerId,
    /// Where the card is, e.g. "Zone B" or "Your hand".
    pub location: String,
    /// Extra facts about this particular card, e.g. "A copy".
    pub notes: Vec<String>,
}

impl CardDetails {
    fn new(card: CardId, power: i32, owner: PlayerId, location: impl Into<String>) -> Self {
        CardDetails { card, power, owner, location: location.into(), notes: Vec::new() }
    }

    /// Add a note if `condition` is true. Returns `self` so calls can chain.
    fn note_if(mut self, condition: bool, note: &str) -> Self {
        if condition {
            self.notes.push(note.to_string());
        }
        self
    }
}

/// Components for anything drawn by `redraw`: redrawn when the game changes,
/// and removed when leaving the game screen.
fn dynamic() -> impl Bundle {
    (Dynamic, DespawnOnExit(AppState::InGame))
}

/// A zone-shaped panel with a label strip across its middle.
fn spawn_panel(commands: &mut Commands, x: f32, width: f32, background: Color, label_background: Color) {
    commands.spawn((
        Sprite::from_color(background, Vec2::new(width, ZONE_HEIGHT)),
        Transform::from_xyz(x, 0.0, 0.0),
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        Sprite::from_color(label_background, Vec2::new(width, ZONE_LABEL_HEIGHT)),
        Transform::from_xyz(x, 0.0, 1.0),
        DespawnOnExit(AppState::InGame),
    ));
}

/// The parts of the board that never change.
pub fn spawn_board(mut commands: Commands) {
    for zone in Zone::ALL {
        let x = zone_x(zone);
        spawn_panel(&mut commands, x, ZONE_WIDTH, PANEL, PANEL_LIGHT);
        commands.spawn((
            text(format!("Zone {}", zone.name()), 22.0, TEXT),
            Transform::from_xyz(x - 20.0, 12.0, 2.0),
            DespawnOnExit(AppState::InGame),
        ));
    }

    spawn_panel(&mut commands, SECRET_X, SECRET_WIDTH, SECRET_BACKGROUND, SECRET_LABEL_BACKGROUND);
    commands.spawn((
        text("Secrets", 24.0, TEXT),
        Transform::from_xyz(SECRET_X, 0.0, 2.0),
        DespawnOnExit(AppState::InGame),
    ));

    commands.spawn((
        TimerText,
        text("", 24.0, TEXT),
        Anchor::CENTER_LEFT,
        Transform::from_xyz(-VIEW_SIZE.x / 2.0 + 220.0, TOP_BAR_Y, 5.0),
        DespawnOnExit(AppState::InGame),
    ));

    commands.spawn((
        text("Esc: back to menu.   Click during the replay to skip ahead.", 14.0, NEUTRAL),
        Anchor::CENTER_LEFT,
        Transform::from_xyz(-VIEW_SIZE.x / 2.0 + 20.0, -VIEW_SIZE.y / 2.0 + 18.0, 2.0),
        DespawnOnExit(AppState::InGame),
    ));
}

/// What the current replay step should animate.
#[derive(Default)]
struct Highlights {
    /// A card that just appeared or turned over.
    flip_in: Option<u32>,
    /// A card that just did something.
    pop: Option<u32>,
    /// A card that was just destroyed: its owner and uid.
    destroyed: Option<(PlayerId, u32)>,
    /// A mission that was just completed.
    mission: Option<(PlayerId, Zone)>,
}

impl Highlights {
    fn for_event(event: &GameEvent) -> Self {
        match *event {
            GameEvent::Revealed { uid, .. }
            | GameEvent::Copied { uid, .. }
            | GameEvent::Moved { uid, .. }
            | GameEvent::SecretFlipped { uid, .. } => Highlights { flip_in: Some(uid), ..default() },
            GameEvent::Destroyed { player, uid, by_uid, .. } => {
                Highlights { pop: Some(by_uid), destroyed: Some((player, uid)), ..default() }
            }
            GameEvent::MissionCompleted { player, zone } => Highlights { mission: Some((player, zone)), ..default() },
            _ => Highlights::default(),
        }
    }

    /// The animation, if any, for the card with this uid.
    fn animation_for(&self, uid: u32, scale: f32) -> Option<Animation> {
        if self.flip_in == Some(uid) {
            Some(Animation::new(AnimationKind::FlipIn, scale))
        } else if self.pop == Some(uid) {
            Some(Animation::new(AnimationKind::Pop, scale))
        } else {
            None
        }
    }
}

/// Throw away everything that depends on the game state and draw it again.
/// During a replay this draws the replay's snapshot instead of the live game.
pub fn redraw(
    mut commands: Commands,
    game: Res<Game>,
    playback: Res<Playback>,
    graveyard_view: Res<GraveyardView>,
    popup: Res<CardPopup>,
    seat: Res<Seat>,
    lost: Res<ConnectionLost>,
    old: Query<Entity, With<Dynamic>>,
    assets: Res<AssetServer>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    // `Seat` is small and `Copy`, so take a plain copy to pass around.
    let seat = *seat;
    let step = playback.current();
    let shown: &GameState = step.map(|step| &step.after).unwrap_or(&game.0);
    let highlights = step.map(|step| Highlights::for_event(&step.event)).unwrap_or_default();
    let me = shown.player(seat.me);
    let my_turn = !playback.is_playing() && shown.phase() == Phase::Playing && !me.ended_turn;

    // The local player's hand. Cards that cost more than the energy left
    // this turn are darkened.
    for (index, &id) in me.hand.iter().enumerate() {
        // Later cards get a higher z so they are drawn on top.
        let position = hand_slot(index, me.hand.len()).extend(20.0 + index as f32);
        let affordable = card(id).cost <= me.energy;
        let border = if affordable && my_turn { CARD_BORDER } else { NEUTRAL };
        let entity = spawn_card(&mut commands, &assets, id, card(id).power, border, position, 1.0);
        let details = CardDetails::new(id, card(id).power, seat.me, "Your hand")
            .note_if(!affordable, "Costs more energy than you have left this turn.")
            .note_if(affordable && my_turn, "Drag it onto a zone to play it.");
        commands.entity(entity).insert((dynamic(), Draggable::FromHand(id), details));
        if !affordable {
            dim_card(&mut commands, entity);
        }
    }

    // The opponent's hand, while Night Owl lets you see it.
    if shown.sees_opponent_hand(seat.me) {
        let hand = &shown.player(seat.opponent()).hand;
        for (index, &id) in hand.iter().enumerate() {
            let position = opponent_hand_slot(index, hand.len()).extend(20.0 + index as f32);
            let entity =
                spawn_card(&mut commands, &assets, id, card(id).power, THEIR_COLOR, position, OPPONENT_HAND_SCALE);
            let details = CardDetails::new(id, card(id).power, seat.opponent(), "Opponent's hand")
                .note_if(true, "You can see this card thanks to Night Owl.");
            commands.entity(entity).insert((dynamic(), details));
        }
        let first = opponent_hand_slot(0, hand.len().max(1));
        commands.spawn((
            dynamic(),
            text("Opponent's hand:", 18.0, THEIR_COLOR),
            Anchor::CENTER_RIGHT,
            Transform::from_xyz(first.x - 40.0, first.y, 5.0),
        ));
    }

    // Cards in zones, and each zone's score and missions.
    for zone in Zone::ALL {
        for (player, is_me) in [(seat.me, true), (seat.opponent(), false)] {
            for (slot, placed) in shown.player(player).cards_at(zone).iter().enumerate() {
                let position = zone_slot(is_me, zone, slot).extend(10.0 + slot as f32);
                let entity = if !is_me && !placed.revealed {
                    spawn_card_back(&mut commands, position, BOARD_CARD_SCALE)
                } else {
                    let can_move = is_me && my_turn && can_move(placed);
                    let can_return = is_me && my_turn && placed.played_this_turn;
                    let border = if can_move {
                        MOVABLE_BORDER
                    } else if placed.revealed {
                        CARD_BORDER
                    } else {
                        PENDING_BORDER
                    };
                    let power = shown.card_power(player, zone, slot);
                    let entity =
                        spawn_card(&mut commands, &assets, placed.card, power, border, position, BOARD_CARD_SCALE);
                    if can_move {
                        commands.entity(entity).insert(Draggable::OnBoard { uid: placed.uid, from: zone });
                    } else if can_return {
                        commands.entity(entity).insert(Draggable::Pending { uid: placed.uid });
                    }
                    let details = CardDetails::new(placed.card, power, player, format!("Zone {}", zone.name()))
                        .note_if(!placed.revealed, "Played this turn: face-down until the turn ends.")
                        .note_if(can_return, "Drag it back to your hand to take it back.")
                        .note_if(placed.is_copy, "A copy made by Clockwork Ant.")
                        .note_if(can_move, "Can move this turn: drag it to another zone.")
                        .note_if(placed.moved_this_turn, "Already moved this turn.");
                    commands.entity(entity).insert(details);
                    entity
                };
                commands.entity(entity).insert(dynamic());
                if let Some(animation) = highlights.animation_for(placed.uid, BOARD_CARD_SCALE) {
                    commands.entity(entity).insert(animation);
                }
            }
        }
        let winner = shown.zone_winner(zone);
        spawn_scores(
            &mut commands,
            seat,
            zone,
            shown.zone_power(seat.me, zone),
            shown.zone_power(seat.opponent(), zone),
            winner,
        );
        spawn_mission(&mut commands, seat, shown, zone, highlights.mission);
    }

    // A card being destroyed: draw it where it was, shrinking away.
    if let (Some((owner, uid)), Some(previous)) = (highlights.destroyed, playback.previous()) {
        if let Some((zone, slot)) = previous.locate(owner, uid) {
            let placed = previous.player(owner).cards_at(zone)[slot];
            let position = zone_slot(owner == seat.me, zone, slot).extend(60.0);
            let power = previous.card_power(owner, zone, slot);
            let entity =
                spawn_card(&mut commands, &assets, placed.card, power, DESTROYED_BORDER, position, BOARD_CARD_SCALE);
            commands.entity(entity).insert((dynamic(), Animation::new(AnimationKind::ShrinkAway, BOARD_CARD_SCALE)));
        }
    }

    // Secret zones. The opponent only sees your secrets once they flip.
    for (player, is_me) in [(seat.me, true), (seat.opponent(), false)] {
        for (slot, placed) in shown.player(player).secrets.iter().enumerate() {
            let position = secret_slot(is_me, slot).extend(10.0 + slot as f32);
            let entity = if is_me || placed.revealed {
                let border = if placed.revealed { CARD_BORDER } else { SECRET_BORDER };
                let power = card(placed.card).power;
                let entity = spawn_card(&mut commands, &assets, placed.card, power, border, position, BOARD_CARD_SCALE);
                let can_return = is_me && my_turn && placed.played_this_turn;
                if can_return {
                    commands.entity(entity).insert(Draggable::Pending { uid: placed.uid });
                }
                let details = CardDetails::new(placed.card, power, player, "Secret zone")
                    .note_if(placed.revealed, "Flipped face-up: its secret has been used.")
                    .note_if(!placed.revealed, "Face-down: the opponent can't see it.")
                    .note_if(can_return, "Drag it back to your hand to take it back.");
                commands.entity(entity).insert(details);
                entity
            } else {
                spawn_card_back(&mut commands, position, BOARD_CARD_SCALE)
            };
            commands.entity(entity).insert(dynamic());
            if let Some(animation) = highlights.animation_for(placed.uid, BOARD_CARD_SCALE) {
                commands.entity(entity).insert(animation);
            }
        }
    }

    // Top bar.
    commands.spawn((
        dynamic(),
        text(format!("Turn {} / {MAX_TURNS}", shown.turn()), 28.0, TEXT),
        // Anchor the text by its left edge so it grows to the right.
        Anchor::CENTER_LEFT,
        Transform::from_xyz(-VIEW_SIZE.x / 2.0 + 20.0, TOP_BAR_Y, 5.0),
    ));
    let opponent = shown.player(seat.opponent());
    commands.spawn((
        dynamic(),
        text(format!("Opponent   hand {}   deck {}", opponent.hand.len(), opponent.deck.len()), 20.0, THEIR_COLOR),
        Anchor::CENTER_RIGHT,
        Transform::from_xyz(VIEW_SIZE.x / 2.0 - 20.0, TOP_BAR_Y, 5.0),
    ));

    // The caption: what's happening in the replay, or what to do next.
    match step {
        Some(step) => {
            let color = match event_player(&step.event) {
                Some(player) if player == seat.me => MY_COLOR,
                Some(_) => THEIR_COLOR,
                None => TEXT,
            };
            commands
                .spawn((
                    dynamic(),
                    Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.7), Vec2::new(900.0, 40.0)),
                    Transform::from_xyz(ZONES_CAPTION_X, CAPTION_Y, 50.0),
                    Animation::new(AnimationKind::Pop, 1.0),
                ))
                .with_children(|banner| {
                    banner.spawn((
                        text(caption(&step.event, shown, seat), 22.0, color),
                        Transform::from_xyz(0.0, 0.0, 0.1),
                    ));
                });
        }
        None => {
            let status = match shown.phase() {
                Phase::GameOver => "",
                Phase::Playing if me.ended_turn => "Waiting for opponent...",
                Phase::Playing => {
                    "Drag cards into a zone, then End Turn. Drag this turn's cards back to your hand to undo."
                }
            };
            commands.spawn((
                dynamic(),
                text(status, 18.0, DIM_TEXT),
                Transform::from_xyz(ZONES_CAPTION_X, CAPTION_Y, 5.0),
            ));
        }
    }

    // Energy and deck, above the End Turn button.
    commands
        .spawn((
            dynamic(),
            Sprite::from_color(ENERGY_COLOR, Vec2::new(150.0, 44.0)),
            Transform::from_xyz(END_TURN_POS.x, END_TURN_POS.y + 90.0, 5.0),
        ))
        .with_children(|badge| {
            badge.spawn((
                text(format!("Energy {} / {}", me.energy, energy_for_turn(shown.turn())), 20.0, TEXT),
                Transform::from_xyz(0.0, 0.0, 0.1),
            ));
        });
    commands.spawn((
        dynamic(),
        text(format!("Deck {}", me.deck.len()), 18.0, DIM_TEXT),
        Transform::from_xyz(END_TURN_POS.x, END_TURN_POS.y + 45.0, 5.0),
    ));

    let destroyed = shown.player(seat.me).graveyard.len() + shown.player(seat.opponent()).graveyard.len();
    spawn_button(
        &mut commands,
        format!("Graveyard ({destroyed})"),
        GRAVEYARD_BUTTON_POS,
        GRAVEYARD_BUTTON_SIZE,
        NEUTRAL,
    )
    .insert(dynamic());
    if graveyard_view.open {
        spawn_graveyard(&mut commands, &assets, seat, shown);
    }

    if lost.0 {
        spawn_disconnected(&mut commands);
    }
    if let Some(details) = &popup.0 {
        spawn_popup(&mut commands, &assets, seat, details);
    }

    match shown.outcome() {
        Some(outcome) if !playback.is_playing() => spawn_game_over(&mut commands, seat, outcome),
        Some(_) => {}
        None => spawn_end_turn_button(&mut commands, my_turn),
    }
}

/// Captions sit centred over the three zones.
const ZONES_CAPTION_X: f32 = 90.0;

/// The player an event is about, if any, to colour its caption.
fn event_player(event: &GameEvent) -> Option<PlayerId> {
    match *event {
        GameEvent::Revealed { player, .. }
        | GameEvent::Copied { player, .. }
        | GameEvent::Moved { player, .. }
        | GameEvent::SecretFlipped { player, .. }
        | GameEvent::MissionCompleted { player, .. } => Some(player),
        // The destroyed card's owner lost it, so colour by who did it.
        GameEvent::Destroyed { player, .. } => Some(player.opponent()),
        _ => None,
    }
}

/// Keep the countdown up to date. Only writes when the text actually
/// changes, so the text isn't re-laid-out every frame.
pub fn update_timer_text(
    timer: Res<TurnTimer>,
    game: Res<Game>,
    playback: Res<Playback>,
    seat: Res<Seat>,
    mut texts: Query<(&mut Text2d, &mut TextColor), With<TimerText>>,
) {
    let counting = game.0.phase() == Phase::Playing && !game.0.player(seat.me).ended_turn && !playback.is_playing();
    let remaining = timer.0.remaining_secs();
    let label = if counting {
        let seconds = remaining.ceil() as u32;
        format!("Time {}:{:02}", seconds / 60, seconds % 60)
    } else {
        String::new()
    };
    let color = if remaining < TIMER_WARNING_SECONDS { TIMER_WARNING } else { TEXT };

    for (mut text, mut text_color) in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
}

/// Could this card be moved to another zone this turn? The rules make the
/// final decision; this only decides whether the UI lets you pick it up.
fn can_move(placed: &PlacedCard) -> bool {
    card(placed.card).effect == Effect::Movable && placed.revealed && !placed.moved_this_turn
}

/// Score badges on the right of a zone's label: opponent above, you below.
/// The badge of whoever is winning the zone is shown in their colour.
fn spawn_scores(commands: &mut Commands, seat: Seat, zone: Zone, mine: i32, theirs: i32, winner: Option<PlayerId>) {
    let x = zone_x(zone) + 110.0;
    let badges = [
        (mine, -20.0, if winner == Some(seat.me) { MY_COLOR } else { NEUTRAL }),
        (theirs, 20.0, if winner == Some(seat.opponent()) { THEIR_COLOR } else { NEUTRAL }),
    ];
    for (score, y, color) in badges {
        commands
            .spawn((dynamic(), Sprite::from_color(color, Vec2::new(44.0, 30.0)), Transform::from_xyz(x, y, 3.0)))
            .with_children(|badge| {
                badge.spawn((text(score.to_string(), 20.0, TEXT), Transform::from_xyz(0.0, 0.0, 0.1)));
            });
    }
}

/// The zone's mission under its name, and who has completed it on each
/// player's side of the label strip.
fn spawn_mission(
    commands: &mut Commands,
    seat: Seat,
    shown: &GameState,
    zone: Zone,
    just_completed: Option<(PlayerId, Zone)>,
) {
    let x = zone_x(zone) - 20.0;
    commands.spawn((
        dynamic(),
        text(shown.zone_mission(zone).description(), 13.0, DIM_TEXT),
        Transform::from_xyz(x, -12.0, 3.0),
    ));
    for (player, label, y) in
        [(seat.me, "You: mission complete", -38.0), (seat.opponent(), "Opponent: mission complete", 38.0)]
    {
        if !shown.mission_done(player, zone) {
            continue;
        }
        // The text sits inside an invisible parent, and the parent is what
        // pops: text sets its own scale to stay sharp.
        let mut holder = commands.spawn((dynamic(), Transform::from_xyz(x, y, 3.0), Visibility::default()));
        holder.with_children(|holder| {
            holder.spawn(text(label, 12.0, MISSION_DONE));
        });
        if just_completed == Some((player, zone)) {
            holder.insert(Animation::new(AnimationKind::Pop, 1.0));
        }
    }
}

fn spawn_end_turn_button(commands: &mut Commands, enabled: bool) {
    let (color, label) = if enabled { (MY_COLOR, "End Turn") } else { (NEUTRAL, "Waiting") };
    spawn_button(commands, label, END_TURN_POS, END_TURN_SIZE, color).insert(dynamic());
}

/// Every destroyed card, yours on top and the opponent's below.
fn spawn_graveyard(commands: &mut Commands, assets: &AssetServer, seat: Seat, shown: &GameState) {
    commands.spawn((
        dynamic(),
        Sprite::from_color(Color::srgb(0.03, 0.03, 0.06), Vec2::new(VIEW_SIZE.x, 560.0)),
        Transform::from_xyz(0.0, 0.0, 300.0),
    ));
    commands.spawn((dynamic(), text("Graveyard", 32.0, TEXT), Transform::from_xyz(0.0, 180.0, 301.0)));

    let scale = 0.55;
    for (player, label, y, color) in
        [(seat.me, "Yours", 70.0, MY_COLOR), (seat.opponent(), "Opponent's", -100.0, THEIR_COLOR)]
    {
        commands.spawn((
            dynamic(),
            text(label, 20.0, color),
            Anchor::CENTER_LEFT,
            Transform::from_xyz(-550.0, y, 301.0),
        ));
        let cards = &shown.player(player).graveyard;
        if cards.is_empty() {
            commands.spawn((
                dynamic(),
                text("Nothing destroyed yet", 16.0, NEUTRAL),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-400.0, y, 301.0),
            ));
        }
        for (index, &id) in cards.iter().enumerate() {
            let position = Vec3::new(-370.0 + index as f32 * 62.0, y, 302.0 + index as f32);
            let entity = spawn_card(commands, assets, id, card(id).power, NEUTRAL, position, scale);
            let details = CardDetails::new(id, card(id).power, player, "Graveyard").note_if(true, "Destroyed.");
            commands.entity(entity).insert((dynamic(), details));
        }
    }
    commands.spawn((
        dynamic(),
        text("Closes by itself, or click Graveyard again", 14.0, NEUTRAL),
        Transform::from_xyz(0.0, -195.0, 301.0),
    ));
}

/// What each kind of effect means, for the details popup.
fn kind_explanation(kind: Option<EffectKind>) -> &'static str {
    match kind {
        None => "No special effect: it just adds its power to the zone.",
        Some(EffectKind::Flip) => "Flip: happens once, when the card is revealed at the end of a turn.",
        Some(EffectKind::Trigger) => "Trigger: happens by itself whenever its condition is met.",
        Some(EffectKind::Passive) => "Passive: always active while the card is face-up.",
        Some(EffectKind::Secret) => "Secret: played face-down in your secret zone; flips when its condition is met.",
    }
}

/// A large view of one card with everything about it. Drawn above the rest
/// of the board; any click closes it.
const POPUP_Z: f32 = 400.0;

fn spawn_popup(commands: &mut Commands, assets: &AssetServer, seat: Seat, details: &CardDetails) {
    let def = card(details.card);
    commands.spawn((
        dynamic(),
        Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.75), VIEW_SIZE * 1.5),
        Transform::from_xyz(0.0, 0.0, POPUP_Z),
    ));
    commands.spawn((
        dynamic(),
        Sprite::from_color(PANEL, Vec2::new(940.0, 440.0)),
        Transform::from_xyz(0.0, 0.0, POPUP_Z + 1.0),
    ));

    let position = Vec3::new(-300.0, 0.0, POPUP_Z + 2.0);
    let big_card = spawn_card(commands, assets, details.card, details.power, CARD_BORDER, position, 2.4);
    commands.entity(big_card).insert(dynamic());

    // The text column, left-aligned to the right of the card.
    let x = -120.0;
    popup_text(commands, bold_text(def.name, 34.0, TEXT), Vec2::new(x, 170.0), Anchor::CENTER_LEFT);

    let mut stats = format!("Cost {}    Power {}", def.cost, details.power);
    if details.power != def.power {
        stats.push_str(&format!("  (printed {}, boosted by {:+})", def.power, details.power - def.power));
    }
    popup_text(commands, text(stats, 20.0, ENERGY_COLOR), Vec2::new(x, 125.0), Anchor::CENTER_LEFT);

    let (owner, owner_color) =
        if details.owner == seat.me { ("Yours", MY_COLOR) } else { ("The opponent's", THEIR_COLOR) };
    let where_text = text(format!("{owner}  -  {}", details.location), 18.0, owner_color);
    popup_text(commands, where_text, Vec2::new(x, 92.0), Anchor::CENTER_LEFT);

    // Rules text, then what its kind of effect means. These wrap, so they
    // hang down from their top-left corner, left-aligned.
    let description =
        if def.effect == Effect::None { "No special effect.".to_string() } else { def.effect.description() };
    let rules =
        popup_text(commands, wrapped_text(description, 20.0, TEXT, 520.0), Vec2::new(x, 65.0), Anchor::TOP_LEFT);
    commands.entity(rules).insert(TextLayout::justify(Justify::Left));
    let meaning = wrapped_text(kind_explanation(def.effect.kind()), 15.0, DIM_TEXT, 520.0);
    let meaning = popup_text(commands, meaning, Vec2::new(x, 5.0), Anchor::TOP_LEFT);
    commands.entity(meaning).insert(TextLayout::justify(Justify::Left));

    for (index, note) in details.notes.iter().enumerate() {
        let y = -60.0 - index as f32 * 26.0;
        popup_text(commands, text(format!("- {note}"), 16.0, DIM_TEXT), Vec2::new(x, y), Anchor::CENTER_LEFT);
    }

    let hint = text("Click anywhere or press Esc to close", 14.0, NEUTRAL);
    popup_text(commands, hint, Vec2::new(0.0, -200.0), Anchor::CENTER);
}

/// One piece of popup text. `anchor` says which point of the text sits at
/// `position`: its left edge, its top-left corner, or its centre.
fn popup_text(commands: &mut Commands, bundle: impl Bundle, position: Vec2, anchor: Anchor) -> Entity {
    commands.spawn((dynamic(), bundle, anchor, Transform::from_translation(position.extend(POPUP_Z + 2.0)))).id()
}

fn spawn_disconnected(commands: &mut Commands) {
    commands
        .spawn((
            dynamic(),
            Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.9), Vec2::new(VIEW_SIZE.x, 180.0)),
            Transform::from_xyz(0.0, 0.0, 350.0),
        ))
        .with_children(|overlay| {
            overlay.spawn((text("Your opponent disconnected", 44.0, THEIR_COLOR), Transform::from_xyz(0.0, 20.0, 0.1)));
            overlay.spawn((
                text("Press Enter to return to the menu", 22.0, DIM_TEXT),
                Transform::from_xyz(0.0, -45.0, 0.1),
            ));
        });
}

fn spawn_game_over(commands: &mut Commands, seat: Seat, outcome: Outcome) {
    let (headline, color) = match outcome {
        Outcome::Winner(player) if player == seat.me => ("Victory!", MY_COLOR),
        Outcome::Winner(_) => ("Defeat", THEIR_COLOR),
        Outcome::Draw => ("Draw", TEXT),
    };
    commands
        .spawn((
            dynamic(),
            Sprite::from_color(Color::srgba(0.0, 0.0, 0.0, 0.8), Vec2::new(VIEW_SIZE.x, 180.0)),
            Transform::from_xyz(0.0, 0.0, 200.0),
        ))
        .with_children(|overlay| {
            overlay.spawn((text(headline, 64.0, color), Transform::from_xyz(0.0, 20.0, 0.1)));
            overlay.spawn((
                text("Press Enter to return to the menu", 22.0, DIM_TEXT),
                Transform::from_xyz(0.0, -45.0, 0.1),
            ));
        });
}
