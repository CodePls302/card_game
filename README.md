# Card Game

A 1v1 card game inspired by Marvel Snap, written in Rust with the
[Bevy](https://bevyengine.org) game engine. Play against a simple AI, or
against a friend over your local network (LAN).

This is an early prototype: the cards and rules are placeholders and will
change.

## Install

**1. Install Rust** (one-time)

- **Windows:** download and run `rustup-init.exe` from https://rustup.rs. If
  it asks to install the Visual Studio C++ build tools, say yes.
- **Mac:** run `xcode-select --install`, then the command shown on
  https://rustup.rs.
- **Linux:** run the command shown on https://rustup.rs, then install the
  audio and input libraries:
  - Ubuntu/Debian: `sudo apt install g++ pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`
  - Fedora: `sudo dnf install gcc-c++ alsa-lib-devel systemd-devel wayland-devel libxkbcommon-devel`
  - Arch: usually nothing extra.

**2. Get the game**

- Click **Code → Download ZIP** at the top of this page and unzip it, or
- `git clone https://github.com/CodePls302/card_game`

**3. Run it**

Open a terminal in the `card_game` folder and run:

```
cargo run --release
```

The first build takes 5–15 minutes. Later runs start in seconds.

## Playing over LAN

Both players need to be on the **same home network** (same Wi-Fi or
router). Guest, school and office networks often block computers from
talking to each other.

1. One player clicks **Host LAN Game** and picks a deck.
2. The other clicks **Join LAN Game** and picks a deck ("Starter A" or
   "Starter B" is fine).
3. The host's game should appear in the list within a few seconds. Click it.
4. If it doesn't appear, type the address shown on the host's screen (e.g.
   `192.168.1.5`) and click **Connect**.
5. If your computer asks whether to allow network access (Windows usually
   does), choose **Allow** for private networks.

The game uses TCP port **7777** (the host must allow it) and UDP port
**7778** (for finding games). On Linux with ufw:

```
sudo ufw allow 7777/tcp
sudo ufw allow 7778/udp
```

## How to play

- Drag cards from your hand into a zone, then press **End Turn**. Both
  players play at the same time; cards are revealed when both have ended
  the turn.
- Each turn you get more energy (2 on turn 1, up to 9 on turn 8). A card's
  cost is the number in its top-left corner, its power in the top-right.
- Win a zone by having more power there, or by completing its mission
  when your opponent hasn't. Whoever wins more zones wins the game; if
  that's even, total power decides.
- Click any card to see what it does. Drag a card you played this turn back
  to your hand to undo it.
- Each turn has a 2-minute limit. The game ends after 8 turns.
- Build your own 15-card decks in the **Deck Editor**.

## Reporting problems

Please send a screenshot plus the last lines of text from the terminal
window the game was started from.

## Credits

- Card art made from icons by Lorc, Delapouite and Skoll on
  [game-icons.net](https://game-icons.net), under CC BY 3.0. See
  [assets/cards/CREDITS.md](assets/cards/CREDITS.md).
- Sound effects by [Kenney](https://kenney.nl) (CC0). See
  [assets/sounds/CREDITS.md](assets/sounds/CREDITS.md).
