# Poor man's livesplit

pmls is an application that lets you time your speedruns.

**IMPORTANT NOTICE**: This crate is is not meant to replace the official
livesplit client that might come to linux once finished. Official desktop MVP
state is detailed
[here](https://github.com/LiveSplit/livesplit-core/projects/2).

pmls allows you to be in game and use your keyboard to time your speedruns
using the livesplit_core library.

## Prerequisites

### Keyboard privileges

Make sure to checkout the help command, otherwise, keyboard may not be detected:

```bash
pmls --help
```

## Installation

```bash
cargo build --release
```

Move artifact at `./target/release/pmls` to `$HOME/.local/bin/` or
your preferred location.

## Example usage

### Interactive

Create the app configuration, then fill speedrun settings (keybinding, split names...) as you go:

```bash
pmls
```

### Use different speedrun

```bash
pmls --game Hades --category "clean file"
```

**Note**: add `--force-speedrun-settings-creation` if settings file is missing.

### Non-interactive quickstart

If you have not created any configuration files, you can skip all dialogs with:

```bash
pmls \
--accept-automatically-configuration-creation \
--game Hades \
--category "clean file" \
-s Numpad1 \
-r Numpad3 \
-p Numpad5 \
-u Numpad7 \
-c Numpad9 \
--force-speedrun-settings-creation \
-n "Tartarus|Asphodel|Elysium|Styx|Hades" \
-i $HOME/Pictures/icons/hades/tartarus.png $HOME/Pictures/icons/hades/asphodel.png $HOME/Pictures/icons/hades/elysium.png $HOME/Pictures/icons/hades/styx.png $HOME/Pictures/icons/hades/hades.png \
--make-speedrun-default
```

## Remove configuration files

```bash
rm $HOME/.config/.pmls
rm -r $HOME/.pmls
```
