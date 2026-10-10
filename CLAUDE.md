# Card Game

A 1v1 card game in Rust + Bevy 0.20, styled after Marvel Snap. Currently a
local proof of concept against a stand-in AI; LAN play comes next, online later.

The developer is learning Rust: explain new Rust concepts when introducing
them, and prefer plain, readable code over clever code.

## Rules so far
- Decks are exactly 15 cards, no duplicates, built from the 25-card catalogue.
  Draw 8 to start, then 1 at the start of each later turn.
- Energy each turn is turn number + 1 (2 on turn 1, 9 on turn 8); unused
  energy is lost. Card costs range 1-9.
- 3 zones (A, B, C), up to 5 cards per player each; each zone is scored per
  player from the power of their face-up cards there.
- Each player also has a secret zone (up to 3 cards). Only Secret cards go
  there; they stay face-down until their condition triggers, then stay put.
  Cards there add no power.
- Both players play at the same time, with 2 minutes per turn (End Turn is
  pressed automatically at 0). Cards played this turn can be dragged back to
  the hand for a refund until End Turn. Cards stay face-down until both end
  the turn. Then cards reveal one at a time: Player One's first on odd turns,
  Player Two's first on even turns, each in the order they were played.
- End of turn order: Triggers (Clockwork Ant copies, Dune Runner wanders),
  then reveals with their Flip effects and secret reactions, then the next
  turn starts (draw, energy) and start-of-turn secrets (Night Owl) flip.
- Effect kinds: Flip (resolves as the card is revealed at end of turn),
  Trigger (waits for a condition), Passive (ongoing), Secret (see above).
- Missions: each zone gets one random mission at game start ("Fill this
  location", "Destroy an enemy card here", "Reach 20 power here"). Each player
  completes it separately, and once complete it stays complete. If exactly
  one player has completed it, they win the zone; otherwise higher power wins.
  Win more zones to win the game; total power breaks ties.
- Destroyed cards go to their owner's graveyard.
- The game ends after 8 turns.
- Decided by the designer: Moss Troll destroys weaker face-up cards of *both*
  players at its zone; only the original Clockwork Ant makes copies (one per
  turn); Cloud Whale can move once per turn; Night Owl (Secret) reveals the
  enemy hand for your next turn; Dune Runner moves to a random zone each turn.
- Still placeholders: Lantern Ghost's secret effect, every card's cost and
  power, and the cards in `catalogue.rs` generally (the designer edits that
  file directly).

## Layout
- `src/game/` holds the rules, as plain Rust with no Bevy. All state changes go
  through `GameState::apply(player, Action)`; that `Action` is what will be sent
  over the network. Unit tests live next to the code.
- `src/net/` is LAN play with std TCP/UDP + threads (no network crates).
  The host is authoritative: it runs `GameState` and sends the whole state
  (including the replay log) as one JSON line whenever it changes; the
  joiner sends `Action`s. UDP broadcast on 7778 announces games; TCP 7777
  carries the game. Bump `PROTOCOL_VERSION` when messages or rules change.
  The full state is sent to the joiner, so a modified client could see the
  host's hand: fine for LAN, must be filtered before online play.
- `src/ui/session.rs`: `Seat` says which player this screen is (host and
  vs-AI = One, joiner = Two); never hard-code a player in UI code.
  `HostLink`/`ClientLink` exist only during LAN games. `ui/lobby.rs` is the
  host/join screen; `ui/board/network.rs` is the in-game sync, and all local
  actions go through `network::submit`.
- `src/assets.rs` finds the `assets/` folder (cwd, else next to the binary);
  Bevy's `AssetPlugin` is pointed at the same folder.
- `src/presets.rs` loads and saves deck presets to `saves/decks.json` (cards
  stored by name). With no save file, the default decks from `deck.rs` are used.
- `src/ui/` holds the Bevy client, one screen per `AppState`: `menu.rs` (main
  menu and deck select), `deck_editor.rs`, and `board/` (the game). Screens
  redraw everything when their resource changes, and input on the board only
  ever submits `Action`s. `widgets.rs` has the shared card/button drawing.
- End-of-turn replay: `GameState` records a `Step` (event + full snapshot) for
  everything that happens in `finish_turn`. `ui/board/playback.rs` queues the
  steps and the board draws each snapshot in turn with a caption and
  animations (`animate.rs`), then returns to the live state.
- `assets/cards/` holds card art, named after the card (`stone_golem.png`).
  The current art is generated from game-icons.net icons (CC BY 3.0, credit
  required: keep `assets/cards/CREDITS.md` up to date) by
  `tools/make_card_art.sh`.
- `assets/sounds/` holds the sound effects (Kenney, CC0). `src/ui/sound.rs`
  maps each `Sound` to its file; systems play one by sending
  `PlaySound(Sound::...)`. Missing files are skipped silently.
- Text: always build `Text2d` with the `text`/`bold_text`/`wrapped_text`
  helpers in `widgets.rs`. They add `SharpText`, which re-renders text at its
  on-screen size and sets the text entity's own scale, so never animate or
  scale a text entity directly; animate its parent instead.

## Commands
- `cargo run` plays the game. `cargo test` runs the rules tests.
- Formatting: `cargo fmt` (120-column config in `rustfmt.toml`).
