# haj

[![AUR](https://img.shields.io/aur/version/haj?logo=archlinux)](https://aur.archlinux.org/packages/haj)
[![Crates.io](https://img.shields.io/crates/v/haj?logo=rust)](https://crates.io/crates/haj)
[![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust)](https://www.rust-lang.org/)

fast, quiet, beautiful package management for BlahArch. 
*(and yes, it is named after the ikea shark. all hail blahaj. 🦈)*

`haj` is a modern, memory-safe wrapper for `pacman` written in rust. It replaces arch's notoriously loud and verbose terminal output with the clean, minimalistic aesthetic of modern package managers like `cargo` and `bun`, without compromising on speed or safety. also comes with love from the supreme plushie shark.

<img alt="haj demo" src="assets/demo-output.gif" width="100%" />

## features

### what's new in v0.2.0: the tui dashboard

`haj` now features a blazing-fast, interactive tui powered by `ratatui`. run `haj tui` to access:

<img alt="haj tui demo" src="assets/tui-demo.gif" width="100%" />

- **real-time search:** press / or f to instantly filter all sync repository packages natively via libalpm.
- **live commands popups:** haj streams native pacman execution logs directly into floating, minimalist UI panels with razor-thin progress bars.
- **the orphan sweeper:** press c on the dashboard to instantly detect and vaporize unneeded dependencies (pacman -Rns) and reclaim disk space.
- **3d rotating blahaj (best):** a fully 3D, spinning ascii art shark rendered natively via [display3d](https://github.com/renpenguin/display3d).

### cli features

- **the cargo/bun aesthetic:** no more jagged progress bars or walls of text. `haj` parses `pacman` output streams in real-time, displaying a single, elegant progress spinner and clean transaction summaries.
- **native alpm engine:** `haj` uses `alpm.rs` (c bindings for `libalpm`) to query your local and sync databases directly in memory.
- **blazing fast orphan detection:** instead of parsing text scripts, `haj orphan` traverses the true alpm directed acyclic graph (dag) to find unused dependencies in microseconds.
- **deadlock-free concurrency:** built on `tokio`, `haj` safely drops database locks before handing state-mutation over to `pacman`, guaranteeing zero lockfile collisions.
- **safe dry run:** Includes a built-in `--dry-run` flag to preview commands without touching your system.

## working   

`haj` is completely transparent and relies entirely on standard arch linux infrastructure:
1. **reads** (`search`, `show`, `orphan`) are done natively via `libalpm` bindings for maximum performance.
2. **writes** (`install`, `remove`, `update`, `clean`) are handed off securely to standard `pacman` subprocesses (`pacman -S`, `pacman -Rs`, etc.). `haj` intercepts the `stdout`/`stderr` streams via pseudo-terminals (pty), filters out the verbose noise, applies color formatting, and pipes the cleaned data to your screen in real-time.

## installation

### aur 

you can install [haj](https://aur.archlinux.org/packages/haj) using your favorite aur helper:

```bash
yay -S haj
# or
paru -S haj
```

### pacman (blaharch-repo)

install `haj` from the official blahArch repo for self-updating release via `pacman`,
add this to the bottom of your `/etc/pacman.conf`:

```bash
[blaharch]
SigLevel = Optional TrustAll
Server = https://asitos.github.io/blaharch-repo/$arch
```

then just sync and install normally (or update `haj` if it is already installed):

```bash
sudo pacman -Sy haj
```

### cargo

requires [rust + cargo](https://www.rust-lang.org/tools/install) to be installed:

```bash
cargo install haj
```

### from source

requires the rust toolchain (`cargo`).

```bash
git clone https://github.com/asitos/haj.git
cd haj
cargo build --release

# install the binary to your system path
sudo install -Dm755 target/release/haj /usr/bin/haj
```

## usage & commands

`haj` provides highly aliased commands for a faster typing experience.

| command | aliases | action |
|---------|---------|--------|
| `tui` | `t` | launch the tui| 
| `install <pkg>` | `i` | install one or more packages | 
| `remove <pkg>` | `rm`, `toss` | remove packages & unneeded dependencies | 
| `update` | `up`, `sync` | sync mirror databases | 
| `search <query>` | `s` | search all remote sync databases | 
| `show <pkg>` | `info` | show local package details | 
| `clean` | `c` | scrub the package cache | 
| `orphan` | `o` | detect orphaned dependencies |

### global flags
- `-d`, `--dry-run` : preview a command without modifying the system.
- `-v`, `--verbose` : enable debug logging.
- `-h`, `--help` : print help information.
- `-V`, `--version` : print version information.

### examples

launch the interactive dashboard:
```bash
haj tui 
# or
haj t
```

search for a package:
```bash
haj search neofetch
# or
haj s neofetch
```

sync repositories and install a package:
```bash
sudo haj sync
# or 
sudo haj up

sudo haj install htop cmatrix
or 
sudo haj i htop cmatrix
```

safely preview removing a package without actually removing it:
```bash
haj -d toss firefox
```

## configuration

`haj` works perfectly out of the box, but can be customized globally by creating a config file at `~/.config/haj/config.toml`. It uses the TOML format.

```toml
# ~/.config/haj/config.toml

[general]
animations = true
color = "auto"
verbose = false
```
*more config options are currently under work*

##  for nerds
`haj` is built with a focus on memory safety, zero-cost abstractions, and concurrent execution.

* **asynchronous pty parsing:** uses `tokio` to spawn non-blocking pseudo-terminals, capturing and formatting `pacman`'s c-level standard streams in real-time without deadlocking background workers.
* **ffi & memory safety:** utilizes `alpm.rs` to interface with arch linux's `libalpm`. database locks are explicitly managed and safely dropped from memory before handing state mutation over to external processes.
* **dag traversal:** the `orphan` detection engine completely avoids bash scripting, instead traversing the system's directed acyclic graph (dag) directly in memory via c-bindings to calculate unneeded dependencies in microseconds.

## license (BORING)

MIT license. see `LICENSE` for more information.
