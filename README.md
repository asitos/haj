# haj

[![AUR](https://img.shields.io/aur/version/haj?logo=archlinux)](https://aur.archlinux.org/packages/haj)
[![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust)](https://www.rust-lang.org/)

fast, quiet, beautiful package management for BlahArch. 

`haj` is a modern, memory-safe wrapper for `pacman` written in rust. It replaces arch's notoriously loud and verbose terminal output with the clean, minimalistic aesthetic of modern package managers like `cargo` and `bun`, without compromising on speed or safety.

## features

- **the cargo/bun aesthetic:** No more jagged progress bars or walls of text. `haj` parses `pacman` output streams in real-time, displaying a single, elegant progress spinner and clean transaction summaries.
- **native alpm engine:** `haj` uses `alpm.rs` (c bindings for `libalpm`) to query your local and sync databases directly in memory.
- **blazing fast orphan detection:** instead of parsing text scripts, `haj orphan` traverses the true alpm directed acyclic graph (dag) to find unused dependencies in microseconds.
- **deadlock-free concurrency:** built on `tokio`, `haj` safely drops database locks before handing state-mutation over to `pacman`, guaranteeing zero lockfile collisions.
- **safe dry run:** Includes a built-in `--dry-run` flag to preview transactions without touching your system.

## working   

`haj` is completely transparent and relies entirely on standard Arch Linux infrastructure:
1. **reads** (`search`, `show`, `orphan`) are done natively via `libalpm` bindings for maximum performance.
2. **writes** (`install`, `remove`, `update`, `clean`) are handed off securely to standard `pacman` subprocesses (`pacman -S`, `pacman -Rs`, etc.). `haj` intercepts the `stdout`/`stderr` streams via pseudo-terminals (pty), filters out the verbose noise, applies color formatting, and pipes the cleaned data to your screen in real-time.

## installation

### aur (arch user repository)

You can install `haj` using your favorite AUR helper:

```bash
yay -S haj
# or
paru -S haj
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

search for a package:
```bash
haj search neofetch
```

sync repositories and install a package:
```bash
sudo haj sync
sudo haj install htop cmatrix
```

safely preview removing a package without actually removing it:
```bash
haj -d toss firefox
```

## configuration

`haj` works perfectly out of the box, but can be customized globally by creating a config file at `/etc/haj.conf`. It uses the TOML format.

```toml
# /etc/haj.conf

[general]
parallel_downloads = 10
animations = true
color = "auto"
verbose = false
```

*Note: `parallel_downloads` here is currently reserved for future internal Rust-native downloading features. Standard installations will still respect your `/etc/pacman.conf` settings.*

##  architecture & technical details
`haj` is built with a focus on memory safety, zero-cost abstractions, and concurrent execution.

* **asynchronous pty parsing:** uses `tokio` to spawn non-blocking pseudo-terminals, capturing and formatting `pacman`'s c-level standard streams in real-time without deadlocking background workers.
* **ffi & memory safety:** utilizes `alpm.rs` to interface with arch linux's `libalpm`. database locks are explicitly managed and safely dropped from memory before handing state mutation over to external processes.
* **dag traversal:** the `orphan` detection engine completely avoids bash scripting, instead traversing the system's directed acyclic graph (dag) directly in memory via c-bindings to calculate unneeded dependencies in microseconds.

## license

MIT License. See `LICENSE` for more information.
