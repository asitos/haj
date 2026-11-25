# haj v0.3.3

focuses on optional dependencies, natural verb-action cli syntax, source installation scripting, and nix flake support under GPL-3.0-or-later licensing.

## dependency decoupling

- **optional `libalpm` (`pacman` / `alpm` cargo features):** compiled with `libalpm` + `pacmanconf` by default, or built cleanly with `--no-default-features` (zero `libalpm.so` linkage) using pure `pacman` cli fallback and pure-rust `vercmp`.
- **optional `display3d`:** `haj tui` launches without error when `display3d` is absent, falling back to static shark art.
- **scoped lock cleanup:** `PackageManager::lock_file_path()` scopes `Ctrl+C` `/var/lib/pacman/db.lck` cleanup strictly to active pacman backends when the lock file exists.

## install script

- added `install.sh` to build `haj` from source (`--release --locked`), auto-detect `libalpm` via `pkg-config` (falling back to `--no-default-features`), install the binary and shell completions, and support `--uninstall`, `--prefix <dir>`, `--no-alpm`, and `--with-display3d`.

## natural cli syntax

- added typing-friendly verbs and aliases across commands (`install`/`add` `i`/`a`, `remove`/`uninstall`/`drop`, `search`/`find`, `show`/`about`/`describe`, `fetch`/`download`/`get`, `downgrade`/`rollback`, `owns`/`owner`/`which`, `locate`/`where`/`find-file`, `orphan`/`autoremove`/`prune`, `diff`/`pacnew`/`merge`, `--from aur|repo`, `upgrade --no-sync`).
- added natural english phrase normalization (`haj add <pkg> from aur`, `haj who owns <path>`, `haj list files of <pkg>`, `haj downgrade <pkg> to <ver>`, `haj mark <pkg> as explicit|dep`, `haj list explicit|deps|aur|orphans|updates`, `haj clean cache|orphans`) while leaving bare interactive search (`haj <query>`) untouched.

## nix flake

- added `flake.nix` targeting `x86_64-linux` and `aarch64-linux` with default `--no-default-features` package, optional `#haj-alpm` variant, shell completions, and github actions nix flake CI checks.

## license

- kept `GPL-3.0-or-later` licensing for compatibility with `alpm`, `pacmanconf`, and Apache-2.0 dependencies.
