# Speedrun splits

## Prerequisites

### Keyboard privileges

Make sure to checkout the help command, otherwise, keyboard may not be detected:

```bash
speedrun_splits --help
```

## Installation

```bash
cargo build --release
# move artifact at ./target/release/speedrun_splits to $HOME/.local/bin/ or
# your preferred location
```

## Example usage

### Interactive

```bash
./target/release/speedrun_splits
```

### Non-interactive

```bash
./target/release/speedrun_splits \
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

## Remove configuration file

```bash
rm -r $HOME/.config/.speedrun_splits
rm -r $HOME/.speedrun_splits        
```
