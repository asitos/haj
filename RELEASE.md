# haj v0.3.0

**Released:** 2026-08-08 — internal refactor release. No new user-facing commands; focus is correctness, safety, and contributor ergonomics.

---

## Breaking internal changes

- `src/tui/app.rs` deleted (373 lines). State split across `tui/mod.rs`, `tui/events.rs`, and `tui/news_fetch.rs`.
- `src/core/escalate.rs`, `src/core/resolver.rs`, `src/core/pacnew.rs` deleted. Logic now lives in `src/core/mod.rs`.
- `src/commands/install.rs` + `src/commands/mod.rs` → `src/commands.rs`.
- `src/ui/progress.rs` + `src/ui/mod.rs` → `src/ui.rs`.

---

## Refactors

### TUI modularized (`src/tui/`)

`tui/mod.rs` was a 2962-line monolith. Split into:

- `tui/events.rs` — event loop and key-handling (1506 lines)
- `tui/news_fetch.rs` — Arch news XML fetch + caching (360 lines)
- `tui/mod.rs` — state structs and module declarations only (1048 lines)

Blanket `#![allow(dead_code)]` suppression removed; dead fields surfaced and cleaned up.

### Consolidated root check

`unsafe { libc::geteuid() == 0 }` was copy-pasted into 4 files:
`escalate.rs`, `pacman.rs`, `aur.rs`, `tui/events.rs`.

Replaced with one function: `crate::core::is_root()`.

### Typed AUR API responses

All three AUR RPC query sites used raw `serde_json::Value` chained with `.get("Name").and_then(|n| n.as_str())` etc.

Replaced with typed structs in `src/core/aur.rs`:

```rust
pub struct AurPackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub num_votes: u64,
    pub conflicts: Vec<String>,  // used by install conflict detection
}
pub struct AurResponse { pub results: Vec<AurPackage> }
```

Covered: `main.rs` search, upgrade check, and `commands.rs` install resolution.

### Flattened premature module structure

`commands/` and `ui/` directories each had one file and one `mod.rs`. Flattened both.

### Removed spurious `async`

`view_pkgbuilds` was `pub async fn` with no `.await` inside. Signature corrected to `pub fn`.

---

## Safety

Removed 8 `.unwrap()` / `.expect()` panics in `src/main.rs` and `src/commands.rs`:

| Site | Before | After |
|------|--------|-------|
| `pacman -Rsp` spawn | `.expect("failed to execute pacman")` | `match … { Err(e) => eprintln! + return }` |
| `pacman -F` spawn | `.expect("failed to execute pacman -F")` | same |
| `pkg_path.to_str()` (×2) | `.unwrap()` | `.unwrap_or_default()` |
| `archive_path.to_str()` | `.unwrap()` | `.unwrap_or_default()` |
| `archive_path.file_name()` | `.unwrap()` | `.map(…).unwrap_or_else(…)` |
| `status.unwrap().success()` | `.is_err() \|\| !…unwrap()…` | `.map_or(true, \|s\| !s.success())` |
| `local_ver.as_ref().unwrap()` | panics if `None` | `.as_deref().unwrap_or_default()` |

---

## CI

- Added `.github/workflows/release.yml` — automated release workflow triggered on version tags.
- `.github/workflows/ci.yml` updated.

---

## Tests

Expanded `tests/cli_tests.rs` to cover all subcommand aliases (full command names + short aliases).
Added `tests/comprehensive_features_tests.rs` (later removed; commands requiring sudo caused CI hangs — test strategy moved to unit + dry-run integration tests).

---

## Stats

```
35 files changed, 2700 insertions(+), 2983 deletions(-)
```

Net negative: removed more code than was added.
