# Quality Improvement Plan for `midi-keybindr`

This document catalogues the issues found in the initial GPT-5.1 Codex
implementation and proposes concrete improvements, grouped by theme.
It is intended to be reviewed by the project owner before implementation.

---

## 1. Code Documentation

### 1.1 Missing module-level doc comments

Every module (`mod.rs`, individual files) is missing a `//!` inner doc
comment explaining its purpose and invariants. For example, `midi/event.rs`
is the only file that touches raw MIDI bytes and deserves an explanation of
the byte format it handles.

**Action**: Add `//!` module-level doc comments to every file, covering:
- What the module does.
- Key invariants (e.g. "`channel` is always 1-based in this module").
- What external crates it wraps (e.g. "wraps `midir`").

### 1.2 Missing `# Errors` and `# Panics` sections

Several public functions that return `Result` or could panic under unusual
inputs lack `# Errors` and `# Panics` doc sections:

- `NoteSpec::parse` — no error documentation
- `Config::from_path` — no error documentation
- `ChannelSet::contains` — silent false return for out-of-range channels
  deserves documentation or a stronger type contract
- `MidiEvent::matches_event` — the parameter semantics (self = trigger,
  argument = incoming event) should be clearly documented

**Action**: Add standard doc sections to all public functions that can fail
or have non-obvious behaviour.

### 1.3 Missing crate-level documentation

`main.rs` has no inner `//!` comment describing the binary or directing
readers to key modules.

**Action**: Add a crate-level doc comment to `main.rs` with a brief
description and pointers to the key entry points.

### 1.4 Dead-code comment opportunity missed in `midi/event.rs`

The `_parse_with_midly` function exists with `#[allow(dead_code)]` but has no
comment explaining why it is kept or what its status is (prototype?
future use?). This creates confusion about whether it represents intended
design or a work-in-progress.

**Action**: Either remove the function entirely (preferred) or add a
`// TODO:` comment with a clear explanation of its purpose and status.

---

## 2. Testing

### 2.1 Insufficient coverage of `MappingEngine`

The single test in `mapper.rs` verifies channel matching but does not cover:
- Device glob filtering (that a non-matching device name is excluded).
- First-mapping-wins semantics when multiple mappings could match.
- `ControlChange` matching with and without a `ValueRange`.
- `ProgramChange` matching.
- `NoteOff` matching.

**Action**: Expand `mapper.rs` tests to cover all trigger types and all three
filter dimensions (device, channel, trigger).

### 2.2 `NoteSpec` edge cases not tested

`trigger.rs` tests only cover `C4`, enharmonic boundaries, and negative
octaves. Missing:
- Flat notes on the lower boundary (`Cb-1` should be an error — below MIDI 0).
- Sharp notes on the upper boundary (`G#9` = MIDI 128 — should be rejected).
- Integer input at bounds (0, 127) and out of bounds (128, 255).
- Case-insensitivity of note letter (`c4` should equal `C4`).
- Invalid inputs: just a letter (`C`), empty string, unknown letter (`H4`).

**Action**: Add parametrised or table-driven tests for the note parser edge
cases.

### 2.3 `ChannelSet` deserialisation not fully tested

Only one test covers `["2-3", "9"]`. Missing:
- Integer scalar input (`1`).
- Wildcard string (`"*"`).
- Empty sequence (should select all channels).
- Descending range (`"5-3"` should be an error).
- Out-of-range channel (17, 0).

**Action**: Add tests for each deserialisation path.

### 2.4 `KeyCombo` parsing gaps

The existing tests are good but miss:
- All named navigation keys (`Up`, `Down`, `Left`, `Right`, `Tab`, `Esc`).
- All function keys (`F1`–`F12`).
- Empty combo string (should be an error).
- Combo with only modifiers and no primary key (e.g. `"Ctrl+"`).
- Case-insensitivity of modifier tokens (`"ctrl+a"` should work).

**Action**: Add comprehensive tests for the key combo parser, including a
table of all named keys.

### 2.5 Unsafe environment variable mutation in `main.rs` tests

`env_var_is_used` uses `unsafe { env::set_var(...) }`. In Rust 2024 this is
explicitly unsafe, and running such tests in parallel with other tests that
read the same variable can cause non-deterministic failures.

**Action**: Wrap env-mutating tests in a global `Mutex` (or use the
`serial_test` crate) to prevent parallel execution.

### 2.6 No test for `Config::from_path`

`config/mod.rs` has no tests. The config loading path (YAML parsing end-to-end)
is tested only implicitly.

**Action**: Add at least one test that writes a minimal YAML file to a temp
file and round-trips it through `Config::from_path`, verifying the resulting
`Mapping` fields.

### 2.7 `midi/event.rs` parsing not fully covered

Only `NoteOn` is tested. Missing:
- `NoteOff` via status byte `0x8n`.
- `NoteOn` with velocity 0 (should produce `NoteOff`).
- `ControlChange` parsing.
- `ProgramChange` parsing.
- Unknown status byte (should return `Ok(None)`).
- Short/malformed messages.

**Action**: Add tests for each MIDI message type and the edge cases listed
above.

---

## 3. End-User Experience

### 3.1 No startup confirmation when the mapper begins listening

When `midi-keybindr` runs successfully, it silently blocks. The user receives
no feedback about which MIDI ports were opened or how many mappings were
loaded.

**Action**: Emit an `info!` log (visible with `-v`) when the mapper starts,
listing the number of mappings loaded and the ports being monitored. Example:

```
[INFO] Loaded 4 mappings from /Users/me/.config/midi-keybindr/config.yaml
[INFO] Listening on 2 MIDI port(s): "Arturia KeyStep Pro MIDI In", "IAC Driver Bus 1"
```

### 3.2 Matched events are never logged

There is no `info!` or `debug!` log when a MIDI event is successfully matched
and a key combo is sent. This makes troubleshooting difficult.

**Action**: Log each matched event at `INFO` level (visible with `-v`), and
log unmatched events at `DEBUG` level (visible with `-vv`). Example:

```
[INFO] note_on C4 ch3 → "Cmd+Space" (KeyStep Pro)
[DEBUG] note_on A2 ch1 — no mapping matched
```

### 3.3 `clap` help text is minimal

The current `about` string is terse. The tool has no `long_about`, no usage
examples in the help output, and the `--config` flag gives no hint about
default search paths.

**Action**:
- Add a `long_about` to `Cli` describing what the tool does, the config
  search order, and a pointer to the config file format.
- Add `#[arg(long_help = "...")]` to `--config` explaining the three-location
  search order (CLI flag → env var → default path).
- Consider using `clap`'s `#[command(after_help = "...")]` to add a usage
  example section.

### 3.4 `--dry-run` / `--check` flag missing

There is no way to validate a config file without starting the mapper and
connecting to real MIDI hardware.

**Action**: Add a `--check` flag (or `validate` subcommand) that loads and
validates the config file and exits with code 0 on success, printing the
number of mappings parsed. This is especially useful in CI or scripting.

### 3.5 Human `list` output has no indication when no ports are found

If no MIDI ports are connected, the `list` command prints only the header.
This is confusing.

**Action**: Print a user-friendly message when no ports are available, e.g.:

```
No MIDI input ports found. Connect a MIDI device and try again.
```

### 3.6 No shell completion support

Power users expect `--generate-completion` support for their shell.

**Action**: Add a `completions` subcommand (using `clap_complete`) that
prints shell completion scripts for `bash`, `zsh`, `fish`, and `powershell`.
This is low-effort and widely appreciated.

---

## 4. Removing Accidental Complexity and Idiomatic Rust

### 4.1 `midly` is imported but not actually used

`midi/event.rs` has a dead-code function `_parse_with_midly` that is the
only use of the `midly` crate. The live `parse_message` function does its own
manual byte parsing. This means:
1. A dependency is carried that provides no real value.
2. The manual parsing may diverge from `midly`'s implementation.
3. The plan's stated rationale ("midly turns raw bytes into structured messages")
   is not realized.

**Action**: Replace the manual byte parsing in `parse_message` with `midly`'s
`LiveEvent::parse`, removing the dead `_parse_with_midly` function and
eliminating the duplication. This also eliminates the custom `MidiMessageType`
enum.

### 4.2 Hand-rolled JSON in `cmd/list.rs`

`print_json` is a hand-rolled JSON serializer that handles only `\`, `"`,
`\n`, `\r`, and `\t` escape sequences. MIDI port names could contain other
control characters (e.g. `\0`, `\x1B`).

**Action**: Use `serde_json` (add as a dev-dependency or feature-gated
dependency) for correct and complete JSON output. Even deriving `Serialize`
on a small anonymous struct and calling `serde_json::to_string_pretty` is
sufficient.

### 4.3 `ChannelSet` and `DeviceGlobs` expose their internal representation

Both types expose `pub` inner fields (`pub u16` and `pub Vec<Pattern>`),
leaking implementation details and allowing callers to bypass invariants.

**Action**: Make the inner fields private. Provide only the public methods
(`contains`, `matches`, `all`, `any`). If construction from raw values is
needed in tests, use `pub(crate)` or a constructor function.

### 4.4 `KeyCombo::raw` is unnecessary at runtime

`KeyCombo::raw` stores the original combo string for error messages, but
error messages are only generated at parse time (config load). Once the
combo is parsed successfully, the `raw` field is never read again.

**Action**: Remove `raw` from `KeyCombo`. Any error context at parse time can
include the raw string inline before constructing the `KeyCombo`. If
display/debug needs are the motivation, derive `Display` on `KeyCombo`
reconstructing the string from its components.

### 4.5 `NoteSpec::raw` has the same issue

Same as above — `NoteSpec::raw` is carried at runtime but only needed during
config validation.

**Action**: Remove `raw` from `NoteSpec` (keep it only within the parser
function scope for error messages). Derive a `Display` impl if a human-readable
form is needed post-parse.

### 4.6 `Arc<MappingEngine>` in `cmd/run.rs` is unnecessary

The `MappingEngine` is wrapped in `Arc` so it can be cloned into midir
callbacks. However, `MappingEngine` contains only `Vec<Mapping>`, which is
`Clone`. The `Arc` adds indirection; a simple clone of the `Arc` still
requires a heap allocation for the reference count.

Given that `MappingEngine` is read-only after construction and the number
of ports is small (typically 1–5), cloning the engine directly per callback
is simpler and avoids the `Arc` overhead. Alternatively, leak a
`&'static MappingEngine` (acceptable for a long-lived process).

**Action**: Either clone `MappingEngine` directly per callback (removes `Arc`
dependency), or document why `Arc` is preferred over clone.

### 4.7 `connect_all` re-enumerates ports inside its own loop

`port.rs::connect_all` creates a new `MidiInput` and calls `.ports()` for
every port in the outer loop. If the set of connected devices changes during
enumeration, the inner port index may not correspond to the correct device.

**Action**: Refactor `connect_all` to enumerate ports once and reuse the
same port references. If `midir`'s API requires a new `MidiInput` per
connection, acquire the port reference before moving it into the connect call.

### 4.8 Original `tx` sender is never dropped in `cmd/run.rs`

In `cmd/run.rs`, `mpsc::channel()` creates `(tx, rx)`. A clone of `tx` is
passed into the callbacks; the original `tx` lives until `execute` returns
(which it never does under normal operation). While this has no practical
effect in the current code, it means `rx.recv()` will never return
`Err(RecvError)` due to "all senders dropped" — the original `tx` is always
alive. This is misleading and could mask bugs.

**Action**: Drop the original `tx` immediately after cloning it for the
callback, so that `rx.recv()` returns an error if all callbacks are dropped
(e.g. all MIDI devices disconnected), allowing a clean exit or reconnect.

---

## 5. Architectural Recommendations

### 5.1 Unify `MidiEvent` as a trigger type vs. a parsed-event type

Currently `MidiEvent` serves two distinct roles:
1. **Trigger definition** in config (e.g. `ControlChange { cc: 64, value: Some(64..=127) }`).
2. **Parsed incoming event** from hardware (e.g. `ControlChange { cc: 64, value: Some(100..=100) }`).

This forces the matching logic (`matches_event`) to treat a single incoming
CC value as `ValueRange { min: v, max: v }`, which is unintuitive. It also
makes the type unclear: does `value: None` mean "any value" (trigger context)
or "value not present" (event context)?

**Recommendation**: Introduce a separate `IncomingEvent` (or `RawEvent`)
type for parsed hardware events. `MidiEvent` in `config/trigger.rs` becomes
the trigger/pattern-only type. The matching method becomes
`MidiEvent::matches(&self, incoming: &IncomingEvent) -> bool`.

This separation makes both types clearer, removes the dual-purpose `ValueRange`
hack, and enables richer trigger patterns (e.g. `velocity: 64..=127`) in the
future.

### 5.2 Consider a `Validator` layer between config loading and runtime

Currently there is no validation step between YAML deserialization and
starting the mapper. Invalid combinations (e.g. a `ControlChange` trigger
with `min > max` in a value range) pass the deserializer but may produce
confusing behavior at runtime.

**Recommendation**: Add a `Config::validate(&self) -> Result<ValidatedConfig>`
step in `cmd/run.rs` after loading the config. `ValidatedConfig` is a
newtype wrapper that can only be produced by passing validation, and is what
`MappingEngine::new` accepts. This makes the contract explicit: you cannot
construct a running engine from an unvalidated config.

### 5.3 `cmd/run.rs` should handle MIDI disconnect gracefully

When all MIDI devices are unplugged, the midir callbacks are dropped, the
`tx` clones are dropped (if #4.8 above is addressed), `rx.recv()` returns
`Err`, and the process exits. However, there is no logging or user-friendly
message before the exit.

**Recommendation**: Handle `rx.recv()` returning `Err` by emitting a warning
and (optionally) attempting to reconnect rather than silently exiting.

### 5.4 Extract config search into its own testable function

`resolve_config_path` in `main.rs` is already a standalone function, but
the HOME-based path construction uses a raw `env::var("HOME")` that is
fragile on Windows and doesn't respect `dirs::config_dir()` idioms. The
function is hard to test in isolation because it mutates global state (env
vars) and may try to stat a file on disk.

**Recommendation**: Move `resolve_config_path` into a `config::location`
module. Accept the home directory as a parameter (or use the `dirs` crate)
so tests can inject a fake home without env-var mutation.

### 5.5 Phase 2: `learn` subcommand scaffolding

The original plan describes a `learn` subcommand for interactively defining
new mappings. While not yet implemented, the current architecture should not
make this harder. A few forward-compatibility notes:

- The `mpsc` channel in `cmd/run.rs` should carry either `Action` (run mode)
  or raw `ParsedEvent` (learn mode). Consider an enum or a generic channel
  payload to avoid code duplication.
- The config write path for `learn` will need YAML serialization. Ensure
  all config types implement `serde::Serialize` before that feature is
  started.

---

## Priority Summary

| Priority | Item |
|---|---|
| **High** | 4.1 — Use `midly` for parsing (eliminates dead code + dependency correctness) |
| **High** | 4.7 — Fix racey port re-enumeration in `connect_all` |
| **High** | 2.5 — Fix unsafe env-var tests (test reliability) |
| **High** | 3.1 / 3.2 — Startup and match logging (core UX gap) |
| **Medium** | 5.1 — Separate trigger type from incoming-event type |
| **Medium** | 4.2 — Replace hand-rolled JSON with `serde_json` |
| **Medium** | 4.3 — Make `ChannelSet`/`DeviceGlobs` fields private |
| **Medium** | 4.8 — Drop original `tx` after cloning |
| **Medium** | 2.1–2.7 — Expand test coverage |
| **Medium** | 3.3 — Improve `clap` help text |
| **Low** | 4.4 / 4.5 — Remove `raw` fields from `KeyCombo`/`NoteSpec` |
| **Low** | 3.4 — Add `--check` flag |
| **Low** | 3.5 — Empty port list message |
| **Low** | 3.6 — Shell completion support |
| **Low** | 5.2 — Config validation layer |
| **Low** | 5.4 — `dirs`-based config path resolution |
| **Future** | 5.5 — `learn` subcommand scaffolding |
