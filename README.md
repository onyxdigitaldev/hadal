# Hadal

> An audiophile-grade TUI music player for Linux

Named after the hadal zone, the deepest oceanic trenches — where only the purest signals reach.

## Features

- **High-quality audio playback** via PipeWire with bit-perfect passthrough support
- **Extensive format support**: FLAC, MP3, AAC/M4A, Ogg Vorbis, Opus, WAV, AIFF, ALAC
- **Fast library scanning** with SQLite FTS5 search
- **Beautiful TUI** built with ratatui, featuring:
  - Album art display via Kitty graphics protocol
  - Vim-like keybindings
  - Multiple themes
- **Playlist management** with M3U8 import/export
- **Audiophile mode** with format transparency

## Requirements

- Linux with PipeWire
- Rust 1.75+
- For album art: Kitty terminal (recommended) or any terminal with Sixel support

## Building

```bash
cargo build --release
```

## Usage

```bash
# Launch the TUI
hadal

# Play a file directly
hadal /path/to/song.flac

# Scan a directory
hadal --scan /path/to/music
```

## Configuration

Configuration is stored in `~/.config/hadal/config.toml`:

```toml
[library]
folders = ["~/Music"]
scan_on_startup = true

[playback]
passthrough = true
gapless = true

[ui]
theme = "dark"
vim_keys = true
```

## Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate down/up |
| `h/l` | Previous/next tab |
| `Enter` | Select/play |
| `Space` | Play/pause |
| `n/p` | Next/previous track |
| `+/-` | Volume up/down |
| `m` | Mute |
| `z` | Toggle shuffle |
| `r` | Cycle repeat |
| `/` | Search |
| `q` | Queue view |
| `?` | Help |
| `Q` | Quit |

## Architecture

Hadal is organized as a Cargo workspace with the following crates:

- `hadal` - Main binary
- `hadal-audio` - Audio decoding and playback
- `hadal-library` - Library management and indexing
- `hadal-playlist` - Playlist management
- `hadal-graphics` - Terminal graphics abstraction
- `hadal-tui` - Terminal user interface
- `hadal-common` - Shared types and utilities

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

MIT OR Apache-2.0
