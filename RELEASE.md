# haj v0.2.9

This release focuses on a faster, safer day-to-day package-management experience, a substantial internal cleanup, and a reproducible Docker distribution.

## Highlights

- Added Docker support for running haj in an isolated Arch Linux container. The image includes the required `display3d` renderer and supports normal CLI and TUI invocation.
- Added a source-release bundle that ships HAJ alongside the pinned `display3d` v0.2.3 source and its license information.
- Added an interactive PKGBUILD viewer for auditing AUR package builds before installation.
- Added offline and unavailable-AUR warnings, plus clearer handling of stale package databases and install-time repository synchronization.
- Improved CLI responsiveness and reduced dependency overhead through targeted refactors and removal of unused code.

## Reliability and safety

- Reworked transaction, conflict, cache, pacnew, history, downgrade, and escalation paths to reduce redundant work and improve error handling.
- Improved progress and package-manager output handling for cleaner, more reliable terminal feedback.
- Fixed configuration handling, shell-completion details, CLI version coverage, and several edge cases across package operations.
- `haj tui` now checks that its required `display3d` renderer is available before starting.

## Codebase and tests

- Modularized the former large `main.rs` implementation into focused command and core modules, including dedicated pacman, UI, install, and conflict handling code.
- Removed obsolete network and UI modules, unused dependencies, and redundant code paths.
- Added focused coverage for CLI behavior, configuration, package features, libalpm integration, network availability, stale databases, and transaction safety.

## Installation notes

`display3d` is required for `haj tui`:

```bash
cargo install display3d --version 0.2.3
cargo install haj
```

For Docker and source-release instructions, see the README included with this release.
