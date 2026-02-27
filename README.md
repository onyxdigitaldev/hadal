# Hadal

> An audiophile-grade TUI music player for Linux

Named after the hadal zone, the deepest oceanic trenches — where only the purest signals reach.

## Features

- **High-quality audio playback** via PipeWire with configurable resampler quality (fast/medium/best)
- **Format support**: FLAC, MP3, AAC/M4A, Ogg Vorbis, Opus, WAV, AIFF, ALAC
- **10-band parametric equalizer** with presets (Flat, Bass Boost, Treble Boost, V-Shape, Loudness) — persists across sessions
- **Real-time spectrum analyzer** and stereo VU meters
- **Album art** via Kitty graphics protocol
- **Ranger-style library browser** — three-column Artist/Album/Track navigation
- **Fast library scanning** with SQLite FTS5 full-text search
- **Playlist management** with M3U8 export support
- **Gapless playback** with lock-free ring buffer pipeline
- **Vim-style keybindings** throughout

## Requirements

- Linux with PipeWire
- Rust 1.75+
- A terminal with Kitty graphics protocol support for album art (Ghostty, Kitty, WezTerm)

## Installation

### From source

```bash
cargo build --release
# Binary at target/release/hadal
```

### Debian package

```bash
cargo install cargo-deb
cargo deb -p hadal
sudo dpkg -i target/debian/hadal_0.1.0-1_amd64.deb
```

## Usage

```bash
# Launch
hadal

# Play a specific file or directory
hadal /path/to/music

# List audio output devices
hadal --list-devices

# Enable debug logging
hadal --debug
```

## Configuration

Config lives at `~/.config/hadal/config.toml` — created automatically on first launch:

```toml
[library]
folders = ["~/Music"]
scan_on_startup = true

[playback]
gapless = true
resampler_quality = "medium"    # fast, medium, best
buffer_size = 1024
default_volume = 0.85

[ui]
show_album_art = true
fps = 60
vim_keys = true

[audio]
output_device = "default"
```

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `1`–`6` | Switch view (Library, Now Playing, Queue, EQ, Search, Playlists) |
| `j/k` | Navigate down/up |
| `h/l` | Navigate left/right |
| `g/G` | Jump to top/bottom |
| `Ctrl+u/d` | Page up/down |
| `Space` | Play/pause |
| `n/p` | Next/previous track |
| `>/<` | Seek forward/backward |
| `+/-` | Volume up/down |
| `m` | Mute |
| `z` | Toggle shuffle |
| `r` | Cycle repeat (off/all/one) |
| `a` | Add track to queue |
| `A` | Add album to queue |
| `P` | Add to playlist |
| `/` | Search |
| `q` | Quit |

### Equalizer (view 4)

| Key | Action |
|-----|--------|
| `h/l` | Select band |
| `j/k` | Adjust gain |
| `b` | Toggle bypass |
| `p` | Cycle presets |
| `0` | Reset to flat |

### Playlists (view 6)

| Key | Action |
|-----|--------|
| `h/l` | Switch panes |
| `n` | Create playlist |
| `d` | Delete playlist |
| `r` | Rename playlist |
| `x` | Remove track |

## Architecture

Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `hadal` | Binary entry point, config, CLI |
| `hadal-audio` | Lock-free decode/resample/output pipeline |
| `hadal-common` | Shared types and paths |
| `hadal-dsp` | Biquad EQ, spectrum analysis, VU metering |
| `hadal-library` | SQLite + FTS5 library, scanner, indexer |
| `hadal-playlist` | Playlist CRUD, M3U8 export, play queue |
| `hadal-tui` | ratatui frontend, views, widgets |

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

MIT — [Onyx Digital Intelligence Development LLC](https://onyxdigital.dev)
