# haj v0.3.0

This release focuses on modularization, robust error handling, security hardening, and code quality improvements across the codebase.

## 🚀 Features & Enhancements

### 📦 Codebase Modularization & Consolidation
- **TUI Architecture Modularized**: Split `src/tui/mod.rs` into logical files (`news_fetch.rs` and `events.rs`) for cleaner navigation and easier open-source contributions.
- **Inlined Premature Modules**:
  - `commands/install.rs` moved to `src/commands.rs`.
  - `ui/progress.rs` moved to `src/ui.rs`.
  - Removed premature directory-nesting for single-file modules.
- **Core Files Consolidated**: Inlined tiny, single-function files (`escalate.rs`, `resolver.rs`, and `pacnew.rs`) into `src/core/mod.rs` to streamline the core library design.

### 🛡️ Hardening & Safety (Zero-Panic Policy)
- **Consolidated Root Check**: Replaced duplicated root verification logic (`unsafe { libc::geteuid() == 0 }`) across 4 files (`escalate.rs`, `pacman.rs`, `aur.rs`, `tui/events.rs`) with a unified, public `crate::core::is_root()` utility.
- **Removed Unsafe Panics**: Replaced 8 dangerous `.unwrap()` and `.expect()` calls inside `src/main.rs` and `src/commands.rs` with graceful error handling and user-facing diagnostics. A package manager should never panic.
- **Non-blocking Sudo / CLI Tests**: Handled terminal and interactive prompts in tests to prevent hanging test runners.

### ⚡ Typed API Design
- **Eliminated Untyped JSON**: Replaced all raw `serde_json::Value` parsing across three AUR query sites in `src/main.rs` and `src/commands.rs` with a strongly-typed `AurPackage` and `AurResponse` deserialization scheme in `src/core/aur.rs`.

### 🤖 CI & Release Automation
- Added GitHub Actions release workflow for fully automated tagging, build validation, and deployment.

## 🐛 Bug Fixes & Refactoring
- Removed blanket `#![allow(dead_code)]` lints to highlight and clean up unused code and imports.
- Removed spurious `async` keyword and `.await` from `view_pkgbuilds` since it is entirely synchronous.
- Fixed version display constants to correctly reference the current version `0.3.0`.
