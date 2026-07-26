# haj

[![AUR](https://img.shields.io/aur/version/haj?logo=archlinux)](https://aur.archlinux.org/packages/haj)
[![Crates.io](https://img.shields.io/crates/v/haj?logo=rust)](https://crates.io/crates/haj)
[![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust)](https://www.rust-lang.org/)

fast, quiet, beautiful package management for blahArch. 
*(and yes, it is named after the ikea shark. all hail blahaj. 🦈)*

`haj` is a modern, memory-safe wrapper for `pacman` written in rust. It replaces arch's notoriously loud and verbose terminal output with the clean, minimalistic aesthetic of modern package managers like `cargo` and `bun`, without compromising on speed or safety. also comes with love from the supreme plushie shark.

<img alt="haj demo" src="assets/demo-output.gif" width="100%" />

## features

### the tui dashboard

`haj` now features a blazing-fast, interactive tui powered by `ratatui`. run `haj tui` to access:

<img alt="haj tui demo" src="assets/tui-demo.gif" width="100%" />

- **real-time search:** press / or f to instantly filter all sync repository packages natively via libalpm.
- **live commands popups:** haj streams native pacman execution logs directly into floating, minimalist UI panels with razor-thin progress bars.
- **the orphan sweeper:** press c on the dashboard to instantly detect and vaporize unneeded dependencies (pacman -Rns) and reclaim disk space.
- **3d rotating blahaj (best):** a fully 3D, spinning ascii art shark rendered natively via [display3d](https://github.com/renpenguin/display3d).

### cli & v0.2.4 highlights

- **interactive search-install:** type a naked query (e.g., `haj discord`) to display a numbered table of matching native and AUR packages. Select numbers (e.g., `1 3`) to queue them instantly.
- **AUR PKGBUILD auditing:** press `v` during the AUR pre-transaction prompt to inspect the live `PKGBUILD` via a fast shallow clone, piped directly into `bat` or `less` right in your terminal.
- **Arch News safety guard:** before running `haj jump`, the system pings the official Arch RSS feed. If a post from the last 7 days requires manual intervention, `haj` displays an unmissable red warning banner with the headline.
- **system overview (`stats`):** a pristine status dashboard tracking package counts, explicit vs. dependency splits, cache sizes, update counts, and system health at a glance.
- **the cargo/bun aesthetic:** no more jagged progress bars or walls of text. `haj` parses `pacman` output streams in real-time, displaying a single, elegant progress spinner and clean transaction summaries.
- **native alpm engine:** `haj` uses `alpm.rs` (C bindings for `libalpm`) to query your local and sync databases directly in memory.

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

install `haj` from the [official blahArch repo](https://asitos.github.io/blaharch-repo/) for self-updating release via `pacman`,
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

### options

| command | aliases | action |
|---------|---------|--------|
| `tui` | `t` | launch the interactive package manager dashboard |
| `update` | `up`, `sync` | synchronize remote repositories |
| `jump` | `upgrade` | full system upgrade |
| `install <pkg>` | `i` | install one or more packages |
| `remove <pkg>` | `rm`, `toss` | remove packages & unneeded dependencies |
| `search <query>` | `s` | search remote repositories |
| `show <pkg>` | `info` | show detailed package information |
| `group <name>` | `g` | browse and install package groups |
| `list` | `ls` | list installed packages |
| `stats` | `st` | show system health and package statistics |
| `load <file>` | `l` | install a local package archive (.pkg.tar.zst) |
| `fetch <pkg>` | `f` | download a package without installing |
| `downgrade <pkg>` | `sink` | downgrade an installed package |
| `owns <file>` | `ow` | find which installed package owns a file |
| `files <pkg>` | `lf` | list files installed by a package |
| `locate <query>` | `loc` | search repositories for a file (pacman -F) |
| `history` | `h` | show recent package changes |
| `orphan` | `o` | detect orphaned dependencies |
| `clean` | `c` | clean the package cache |
| `mark <pkg>` | `m` | change a package's install reason |
| `diff` | `pn` | interactively manage and merge .pacnew config files |

- `-a`, `--aur` : restrict operations to the aur.
- `-r`, `--repo` : restrict operations to arch repositories.
- `-y`, `--noconfirm` : bypass all confirmation prompts.
- `-n`, `--needed` : do not reinstall up-to-date packages.
- `-i`, `--ignore <pkg>` : ignore a package upgrade (comma-separated: pkg1,pkg2).
- `-c`, `--config <path>` : specify an alternate pacman config file.
- `--root <path>` : specify an alternate installation root.
- `-v`, `--verbose` : enable verbose debug logging.
- `-d`, `--dry-run` : preview a command without modifying the system.
- `-h`, `--help` : display help information.
- `-V`, `--version` : show version info.

### examples

launch the interactive dashboard:
```bash
haj tui 
# or
haj t
```

search for a package:
```bash
# for interactive search and install
haj discord

# show all searches
haj search discord
# or
haj s discord
```

keeping up to date:
```bash
# full system upgrade
haj jump

# sync repos
haj sync
# or 
haj up

haj install htop cmatrix
or 
haj i htop cmatrix
```

safely preview removing a package:
```bash
haj -d toss firefox
```

## configuration

`haj` works perfectly out of the box, but can be customized globally by creating a config file at `~/.config/haj/config.toml`. It uses the TOML format.

```toml
# ~/.config/haj/config.toml
# default options

[general]
animations = true
theme = "catppuccin"
aur_only = false
repo_only = false
build_dir = "~/.cache/haj/aur"
diff_prog = "vimdiff"
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
