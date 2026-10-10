# Sound credits

All sounds are by Kenney (https://kenney.nl), released as CC0 (public
domain): "Casino Audio" and "Interface Sounds". They were renamed to say
what they're used for:

| File                  | Original                  |
|-----------------------|---------------------------|
| card_pickup.ogg       | Casino: card-slide-1      |
| card_place.ogg        | Casino: card-place-1      |
| card_return.ogg       | Casino: card-slide-5      |
| card_move.ogg         | Casino: card-shove-1      |
| card_reveal.ogg       | Casino: card-place-3      |
| end_turn.ogg          | Casino: chips-stack-3     |
| turn_start.ogg        | Casino: card-fan-1        |
| card_destroyed.ogg    | Interface: glass_002      |
| card_copied.ogg       | Interface: pluck_001      |
| secret_flipped.ogg    | Interface: question_001   |
| mission_complete.ogg  | Interface: confirmation_002 |
| invalid.ogg           | Interface: error_004      |
| button_click.ogg      | Interface: click_002      |
| timer_tick.ogg        | Interface: tick_002       |
| victory.ogg           | Interface: confirmation_004 |
| defeat.ogg            | Interface: error_006      |
| graveyard.ogg         | Interface: open_001       |

To swap a sound, replace the file with your own (keeping the name), or
change the file name in `src/ui/sound.rs` (`Sound::file`). Delete a file to
mute that sound.
