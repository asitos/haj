# haj v0.3.0

internal refactor release. no new user-facing commands; focus is correctness, safety, and making it easier for other people to contribute.

## breaking internal changes

- `src/tui/app.rs` deleted (373 lines). state split across `tui/mod.rs`, `tui/events.rs`, and `tui/news_fetch.rs`.
- `src/core/escalate.rs`, `src/core/resolver.rs`, `src/core/pacnew.rs` deleted. logic now lives in `src/core/mod.rs`.
- `src/commands/install.rs` + `src/commands/mod.rs` → `src/commands.rs`.
- `src/ui/progress.rs` + `src/ui/mod.rs` → `src/ui.rs`.

## ci

- added `.github/workflows/release.yml` — automated release workflow triggered on version tags.
- `.github/workflows/ci.yml` updated.

### tui modularized (`src/tui/`)

`tui/mod.rs` was a 2962-line dungeon or smth lmao (sorry i didnt think much while making). split into:

- `tui/events.rs` — event loop and key-handling (1506 lines)
- `tui/news_fetch.rs` — arch news xml fetch + caching (360 lines)
- `tui/mod.rs` — state structs and module declarations only (1048 lines)

blanket `#![allow(dead_code)]` suppression removed; dead fields surfaced and cleaned up.

### consolidated root check

`unsafe { libc::geteuid() == 0 }` was copy-pasted into 4 files:
`escalate.rs`, `pacman.rs`, `aur.rs`, `tui/events.rs`.

replaced with one function: `crate::core::is_root()`.

### typed aur api responses

all three aur rpc query sites used raw `serde_json::value` chained with `.get("name").and_then(|n| n.as_str())` etc.

replaced with typed structs in `src/core/aur.rs`:

```rust
pub struct aurpackage {
    pub name: string,
    pub version: string,
    pub description: option<string>,
    pub num_votes: u64,
    pub conflicts: vec<string>,  // used by install conflict detection
}
pub struct aurresponse { pub results: vec<aurpackage> }
```

covered: `main.rs` search, upgrade check, and `commands.rs` install resolution.

### flattened premature module structure

`commands/` and `ui/` directories each had one file and one `mod.rs`. flattened both.

## safety

removed 8 `.unwrap()` / `.expect()` panics in `src/main.rs` and `src/commands.rs`:

## tests

expanded `tests/cli_tests.rs` to cover all subcommand aliases (full command names + short aliases).
added `tests/comprehensive_features_tests.rs` (later removed; commands requiring sudo caused ci hangs — test strategy moved to unit + dry-run integration tests).

