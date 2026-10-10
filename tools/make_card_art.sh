#!/usr/bin/env bash
# Usage: make_art.sh <icons-root> <output-dir>
# Renders one 352x240 card art PNG per card from game-icons.net SVGs:
# a gradient in the card's hue with the icon in front.
set -euo pipefail
ICONS="$1"
OUT="$2"
WORK="$(mktemp -d)"
mkdir -p "$OUT"

# id|card name|icon path (author/icon)
CARDS='0|Ember Sprite|lorc/fairy
1|Stone Golem|delapouite/rock-golem
2|Tide Caller|lorc/wave-crest
3|Night Owl|lorc/owl
4|Iron Knight|delapouite/black-knight-helm
5|Storm Hawk|lorc/hawk-emblem
6|Moss Troll|skoll/troll
7|Glass Mage|delapouite/wizard-face
8|Dune Runner|lorc/sprint
9|Frost Wolf|lorc/wolf-head
10|Sun Priest|delapouite/sun-priest
11|Shadow Fox|lorc/fox-head
12|Coral Giant|delapouite/giant
13|Thorn Archer|delapouite/archer
14|Ash Drake|lorc/wyvern
15|Clockwork Ant|delapouite/ants
16|Lantern Ghost|lorc/ghost
17|River Otter|lorc/paw
18|Granite Bear|delapouite/bear-head
19|Spark Imp|lorc/imp
20|Cloud Whale|delapouite/sperm-whale
21|Rune Smith|delapouite/blacksmith
22|Velvet Moth|lorc/butterfly
23|Hollow King|lorc/crowned-skull
24|Star Seer|lorc/crystal-ball'

echo "$CARDS" | while IFS='|' read -r id name icon; do
  slug="$(echo "$name" | tr '[:upper:]' '[:lower:]' | tr ' ' '_')"
  hue=$(( id * 360 / 25 ))
  # Drop the icon's black background square so only the white shape is left.
  sed 's#<path d="M0 0h512v512H0z"/>##' "$ICONS/$icon.svg" > "$WORK/icon.svg"
  rsvg-convert -w 200 -h 200 "$WORK/icon.svg" -o "$WORK/icon.png"
  magick -size 352x240 "gradient:hsl($hue,55%,48%)-hsl($hue,55%,18%)" "$WORK/bg.png"
  # Soft dark shadow under a slightly tinted icon.
  magick "$WORK/icon.png" -fill "hsl($hue,40%,92%)" -colorize 100 "$WORK/icon_tinted.png"
  magick "$WORK/icon.png" -fill black -colorize 100 -channel A -evaluate multiply 0.5 +channel -blur 0x6 "$WORK/shadow.png"
  magick "$WORK/bg.png" \
    "$WORK/shadow.png" -gravity center -geometry +4+6 -composite \
    "$WORK/icon_tinted.png" -gravity center -geometry +0+0 -composite \
    "$OUT/$slug.png"
  echo "$name <- $icon"
done
rm -rf "$WORK"
