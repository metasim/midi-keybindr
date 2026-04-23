# AGENTS.md — Contributor & Agent Guide

This file is the authoritative reference for AI coding agents (and human
contributors) working in the `midi-keybindr` repository.

---

## Project Overview

`midi-keybindr` is a Rust CLI tool that listens for MIDI events from one or more
input devices and translates them into synthesized keyboard events delivered to
the focused application. Mappings are defined in a YAML config file.

Primary target platform: **macOS** (cross-platform portability is a secondary
goal). The `enigo` keyboard-synthesis backend requires **Accessibility**
permission on macOS.

---

## Repository Layout

```
midi-keybindr/
├── Cargo.toml
├── plans/                     # Design documents and improvement plans
│   └── midi-mapper-plan.md    # Original architecture and implementation plan
├── src/
│   ├── main.rs                # CLI parse, tracing init, config resolution, dispatch
│   ├── cli.rs                 # clap types: Cli, Command, ListArgs, OutputFormat
│   ├── mapper.rs              # MappingEngine: pure event-to-action matching
│   ├── output.rs              # KeyCombo → enigo keyboard synthesis
│   ├── config/
│   │   ├── mod.rs             # Config, Mapping; Config::from_path
│   │   ├── action.rs          # Action, KeyCombo, key-combo parser
│   │   ├── channel.rs         # ChannelSet (bitmask over channels 1–16)
│   │   ├── device.rs          # DeviceGlobs (case-insensitive glob matching)
│   │   └── trigger.rs         # MidiEvent, NoteSpec, ValueRange
│   ├── cmd/
│   │   ├── list.rs            # `list` subcommand implementation
│   │   └── run.rs             # Runtime: wires midi → mapper → output via mpsc
│   └── midi/
│       ├── event.rs           # Raw midir bytes → ParsedEvent (via midly)
│       └── port.rs            # midir port enumeration and connection
```

---

## Build, Lint, and Test

```bash
# Build
cargo build

# Run tests
cargo test

# Check (no codegen; faster than build)
cargo check

# Lint
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check

# Format in place
cargo fmt
```

There are no additional scripts. All commands are standard Cargo.

---

## Code Conventions

### File headers

Every Rust source file begins with three SPDX comment lines:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: <contributor name or tool>
```

New files added by agents should include these lines. Use the agent/tool name
on the `SPDX-FileContributor` line.

### Rust edition and style

- **Edition**: Rust 2024 (`edition = "2024"` in `Cargo.toml`).
- Follow standard Rust naming conventions (snake_case modules/functions,
  CamelCase types).
- Prefer `anyhow` for application-level errors; avoid `unwrap` in non-test
  code except where a value is provably non-None (and the reason is clear).
- Use `tracing::{info, debug, warn, error, trace}` macros for all runtime
  logging — never `eprintln!` or `println!` for diagnostic output.
- Doc comments use `///` for public items and `//!` for module-level docs.
  Include examples in doc comments for non-trivial public functions.

### Module responsibilities (do not violate)

| Module | Allowed to touch |
|---|---|
| `config/` | `serde`, `keybinds`, `glob`, `yaml_serde` — **no** `midir`, **no** `enigo` |
| `midi/` | `midir`, `midly` — no `enigo`, no config parsing |
| `mapper.rs` | Pure logic only — no I/O, no platform APIs |
| `output.rs` | `enigo` only — the single file that synthesizes keys |
| `cmd/` | Orchestration — may use all modules |

### Error messages

User-visible error messages must be actionable. They should:
1. State what failed.
2. Explain why (if knowable).
3. Suggest what the user should do.

---

## Testing Conventions

- Unit tests live in `#[cfg(test)]` blocks at the bottom of the file they
  test.
- Integration tests (if added) live in `tests/`.
- Each test function has a `///` doc comment describing what property it
  verifies.
- Tests that mutate environment variables must be isolated from parallel
  execution (use `std::sync::Mutex` or the `serial_test` crate).
- Modules with pure logic (`config/`, `mapper.rs`) are the primary testing
  targets; modules requiring hardware (`midi/port.rs`) should provide
  testable helper functions that can be called with mock data.

---

## Dependency Policy

- Do **not** add new dependencies without reviewing the existing ones first.
- Prefer standard library solutions over micro-crates.
- Run `cargo audit` before adding or updating any dependency.
- The `midly` crate is used for MIDI byte parsing — use it; do not duplicate
  manual byte-level parsing.
- The `keybinds` crate is used **only at config-load time** for parsing key
  combo strings. Do not use it at runtime.

---

## Pull Request Checklist

- [ ] All tests pass (`cargo test`).
- [ ] No new `clippy` warnings (`cargo clippy -- -D warnings`).
- [ ] Code is formatted (`cargo fmt -- --check`).
- [ ] New public items have `///` doc comments.
- [ ] New source files include SPDX headers.
- [ ] `AGENTS.md` or `README.md` updated if behavior or conventions changed.
