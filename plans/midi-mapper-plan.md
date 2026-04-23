# midi-mapper: Architecture & Implementation Plan

A Rust CLI tool that maps MIDI events to keyboard actions on macOS (with
cross-platform portability as a secondary goal).

---

## Overview

`midi-mapper` listens for MIDI events from one or more input devices and
translates them into synthesized keyboard events delivered to whatever
application is currently in focus. Mappings are defined in a YAML config file.

---

## Dependencies

```toml
[dependencies]
anyhow             = "1"
clap               = { version = "4", features = ["derive"] }
enigo              = "0.2"
glob               = "0.3"
keybinds           = { version = "0.0.7", features = ["serde"] }
midir              = "0.10"
midly              = "0.5"
serde              = { version = "1", features = ["derive"] }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
yaml_serde         = "0.10"
```

### Dependency rationale

| Crate | Role |
|---|---|
| `clap` | CLI argument parsing |
| `midir` | Cross-platform MIDI I/O; uses CoreMIDI on macOS |
| `midly` | Zero-copy MIDI message parsing (raw bytes → structured events) |
| `enigo` | Cross-platform keyboard event synthesis; uses `CGEventPost` on macOS |
| `keybinds` | Parses human-friendly key combo strings (`Cmd+Shift+F8`) at config load time only |
| `glob` | Case-insensitive glob matching for device name patterns |
| `yaml_serde` | YAML deserialisation; maintained by the YAML organisation |
| `serde` | Serialisation framework |
| `anyhow` | Application-level error handling |
| `tracing` + `tracing-subscriber` | Structured logging with verbosity levels |

### Key architectural decisions

- **`midir` not `coremidi`** — cross-platform abstraction; CoreMIDI is used
  transparently as the macOS backend.
- **`enigo` not `core-graphics`** — cross-platform abstraction; `CGEventPost`
  is used transparently on macOS.
- **`keybinds` for parsing only** — used solely at config load time to parse
  combo strings; runtime representation uses `enigo::Key` directly.
- **`midly` for MIDI parsing** — `midir` delivers raw bytes; `midly` turns
  them into structured messages.

### macOS permission requirement

`enigo` uses `CGEventPost` under the hood. The process (or its parent, e.g.
Terminal.app) must be granted **Accessibility** access in:

> System Settings → Privacy & Security → Accessibility

macOS silently drops posted events if permission is absent — no crash, no
error. The tool must detect this at startup and emit a clear diagnostic.

---

## CLI

### Behaviour

- With no subcommand: load config and run the mapper.
- `list` subcommand: enumerate available MIDI input ports.
- `--config` overrides the default config search locations.
- `-v` / `-vv` / `-vvv` increase log verbosity.

### Config file search order

The tool searches for a config file in this order, using the first match:

1. `--config <path>` CLI flag
2. `$MIDI_MAPPER_CONFIG` environment variable
3. `~/.config/midi-mapper/config.yaml`

Note: `~/.config` is used on all platforms. `~/Library/Application Support`
is intentionally avoided — it is appropriate for GUI apps, not CLI tools.

### Clap structs

```rust
#[derive(Debug, Parser)]
#[command(
    name = "midi-mapper",
    about = "Map MIDI events to keyboard actions",
    version,
    propagate_version = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to config file (overrides default search locations)
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List available MIDI input ports
    List(ListArgs),
}

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    /// Output format
    #[arg(short, long, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}
```

### Verbosity → log level mapping

| Flag | Level |
|---|---|
| (none) | `WARN` |
| `-v` | `INFO` |
| `-vv` | `DEBUG` |
| `-vvv` or more | `TRACE` |

The `env-filter` feature on `tracing-subscriber` additionally allows
`RUST_LOG` to override per-module, e.g. `RUST_LOG=midi_mapper::midi=trace`.

---

## Module Structure

```
midi-mapper/
├── Cargo.toml
└── src/
    ├── main.rs                  # CLI parse, tracing init, config resolution, dispatch
    │
    ├── cli.rs                   # Cli, Command, ListArgs, OutputFormat
    │
    ├── config/
    │   ├── mod.rs               # Config, Mapping, re-exports
    │   ├── device.rs            # DeviceGlobs, glob matching
    │   ├── channel.rs           # ChannelSet, range-list parser
    │   ├── trigger.rs           # MidiEvent, NoteSpec and its parser
    │   └── action.rs            # Action, KeyCombo, deserialize_key_combo
    │
    ├── cmd/
    │   ├── mod.rs
    │   ├── list.rs              # `list` subcommand: enumerate midir ports
    │   └── run.rs               # load config, wire midi → mapper → enigo
    │
    ├── midi/
    │   ├── mod.rs
    │   ├── port.rs              # midir port enumeration and connection
    │   └── event.rs             # raw midir bytes → MidiEvent via midly
    │
    ├── mapper.rs                # MappingEngine: match event → Action
    └── output.rs                # KeyCombo → enigo key press/release
```

### Module responsibilities

**`main.rs`** — thin entry point. Parses CLI, initialises tracing, resolves
config path, dispatches to `cmd::list` or `cmd::run`.

**`cli.rs`** — all `clap` type definitions. No logic.

**`config/`** — pure data. Depends only on `serde`, `keybinds`, `glob`, and
`yaml_serde`. No `midir`, no `enigo`. Fully testable in isolation.

**`midi/port.rs`** — wraps `midir` for port enumeration and connection.
Provides the port name string used for device glob matching.

**`midi/event.rs`** — translates raw `&[u8]` from the `midir` callback
through `midly` into the crate's own `config::trigger::MidiEvent` type. The
rest of the codebase never sees raw bytes.

**`mapper.rs`** — pure logic. Takes a parsed `MidiEvent`, port name, and
channel number; iterates mappings; returns `Option<&Action>`. No I/O, no
platform calls. The easiest module to unit test.

**`output.rs`** — translates a `KeyCombo` into the correct sequence of
`enigo` calls: modifiers down in order, primary key click, modifiers up in
reverse order. The only file that touches `enigo`.

**`cmd/list.rs`** — calls `midi::port::list_inputs()`, formats output as
human-readable table or JSON.

**`cmd/run.rs`** — the integration point. Wires `midi`, `mapper`, and
`output` together via an `mpsc` channel (see Threading model below).

---

## Threading Model

The `midir` callback runs on a background thread. `enigo::Enigo` is not
`Send`, so key synthesis cannot happen inside the callback directly.

```
midir background thread          main thread
──────────────────────           ────────────────────────────────
midi callback fires
  → parse bytes (midly)
  → match mappings (mapper)
  → send Action over mpsc  ───►  receive Action
                                  → synthesize keys (output/enigo)
```

The `mpsc` channel is created in `cmd/run.rs`. The sender is moved into the
`midir` callback closure; the receiver is polled on the main thread.

---

## Data Flow

```
midir callback (midi/port.rs)
    │  raw &[u8] + port name + channel
    ▼
midi/event.rs       →   config::trigger::MidiEvent
    │
    ▼
mapper.rs           →   Option<&Action>
    │  (via mpsc channel)
    ▼
output.rs           →   enigo key events → macOS window server
```

Each boundary is a clean type transition with no platform leakage.

---

## Config Types

### Top-level

```rust
pub struct Config {
    pub mappings: Vec<Mapping>,
}

pub struct Mapping {
    pub description: Option<String>,
    pub devices: DeviceGlobs,      // default: match all
    pub channel: Option<ChannelSet>, // default: match all
    pub trigger: MidiEvent,
    pub action: Action,
}
```

### Device selection (`config/device.rs`)

Glob patterns matched case-insensitively against midir port names.

```rust
pub struct DeviceGlobs(pub Vec<glob::Pattern>);

impl DeviceGlobs {
    pub fn any() -> Self { ... }                    // single "*" pattern
    pub fn matches(&self, name: &str) -> bool { ... } // OR across patterns
}
```

Deserialises from either a single string or a YAML sequence:

```yaml
# single pattern
devices: "KeyStep*"

# multiple patterns (YAML list)
devices:
  - "KeyStep*"
  - "*Akai*"

# all devices
devices: "*"

# omit field entirely = all devices
```

### Channel selection (`config/channel.rs`)

```rust
/// Bitmask over channels 1–16. Bit N-1 corresponds to channel N.
#[derive(Clone, Copy)]
pub struct ChannelSet(pub u16);

impl ChannelSet {
    pub fn all() -> Self { Self(0xFFFF) }
    pub fn contains(&self, channel: u8) -> bool {
        self.0 & (1 << (channel - 1)) != 0
    }
}
```

Deserialises from an integer, `"*"`, or a YAML list of integers and
range strings:

```yaml
channel: 1          # single channel
channel: "*"        # all channels
channel:            # YAML list
  - 1
  - "2-3"
  - "9-11"
# omit field entirely = all channels
```

### MIDI triggers (`config/trigger.rs`)

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiEvent {
    NoteOn {
        note: NoteSpec,
    },
    NoteOff {
        note: NoteSpec,
    },
    ControlChange {
        cc: u8,                       // 0–127
        value: Option<ValueRange>,    // None = any value
    },
    ProgramChange {
        program: u8,                  // 0–127
    },
}

pub struct ValueRange {
    pub min: u8,
    pub max: u8,
}

/// Original string preserved for error messages.
pub struct NoteSpec {
    pub raw: String,
    pub note: u8,    // resolved MIDI note number, 0–127
}
```

### NoteSpec parser

Accepts:

| Form | Examples | Notes |
|---|---|---|
| MIDI integer | `60`, `21` | 0–127 |
| Note + octave, natural | `C4`, `A0`, `G9` | |
| Note + octave, sharp | `C#4`, `A#0` | |
| Note + octave, flat | `Bb4`, `Eb3`, `Ab2` | |
| Enharmonic boundaries | `B#3`, `Cb4`, `E#2`, `Fb5` | Resolved correctly |
| Negative octave | `C-1` | MIDI notes 0–11 |

**Octave convention**: Scientific Pitch Notation. C4 = middle C = MIDI 60.
Valid range: C-1 (MIDI 0) through G9 (MIDI 127).

**Not supported**: double sharps/flats (`Ax`, `Bbb`), LilyPond tick notation
(`c'`, `bes,`), velocity suffixes.

**Parse algorithm**:
1. Try parse as integer → validate 0–127
2. Consume note letter (A–G, case-insensitive)
3. Consume optional accidental (`#` or `b`) — unambiguous since note letter
   already consumed
4. Consume signed integer octave — handles `-1` naturally
5. Compute: `midi = (octave + 1) * 12 + semitone`
6. Validate result in 0–127; error with original `raw` string if not

Enharmonic boundary arithmetic:
- `B#n` → semitone 0, octave n+1
- `Cb n` → semitone 11, octave n-1
- `E#n` → semitone 5, octave n
- `Fb n` → semitone 4, octave n

### Keyboard actions (`config/action.rs`)

```rust
pub struct KeyCombo {
    pub modifiers: Vec<enigo::Key>,  // Control, Shift, Alt, Meta
    pub key: enigo::Key,             // primary key
    pub raw: String,                 // original string for error messages
}

pub struct Action {
    #[serde(deserialize_with = "deserialize_key_combo")]
    pub keys: KeyCombo,
}
```

**Parsing**: The `keybinds` crate parses the combo string; the result is
converted to `enigo::Key` values. `keybinds` is not used at runtime.

**Supported modifier aliases** (case-insensitive, handled by `keybinds`):

| Config token | Maps to |
|---|---|
| `Ctrl`, `Control` | `enigo::Key::Control` |
| `Shift` | `enigo::Key::Shift` |
| `Alt`, `Opt`, `Option` | `enigo::Key::Alt` |
| `Cmd`, `Command`, `Meta`, `Super` | `enigo::Key::Meta` |

**Shift behaviour**: per `keybinds` grammar, `Shift` is valid with named keys
(`Shift+F8`, `Shift+Up`). For shifted character keys, use the logical
character directly (`A` rather than `Shift+a`).

**Key synthesis order** in `output.rs`:
1. Press each modifier in order
2. Click primary key (press + release)
3. Release each modifier in reverse order

---

## Example Config File

```yaml
mappings:
  - description: "A0 on channel 3 from KeyStep → F8"
    devices: "KeyStep*"
    channel: 3
    trigger:
      type: note_on
      note: "A0"
    action:
      keys: "F8"

  - description: "Middle C from any Akai device, any channel → Cmd+Space"
    devices:
      - "*Akai*"
    trigger:
      type: note_on
      note: 60
    action:
      keys: "Cmd+Space"

  - description: "Sustain pedal (half-pressed or more) → Cmd+Shift+3"
    channel:
      - "1-2"
    trigger:
      type: control_change
      cc: 64
      value:
        min: 64
        max: 127
    action:
      keys: "Cmd+Shift+3"

  - description: "Any device, any channel, note by flat name"
    trigger:
      type: note_on
      note: "Bb3"
    action:
      keys: "Ctrl+Alt+P"
```

---

## `list` Command Output

Human format shows both index and full port name, since both are valid
`DeviceSelector` values:

```
Index  Name
─────  ────────────────────────────────
0      Arturia KeyStep Pro MIDI In
1      IAC Driver Bus 1
2      USB MIDI Interface
```

JSON format emits an array of `{ "index": N, "name": "..." }` objects.

---

## Phase 2: Learning Mode

Not implemented in phase 1. The `learn` subcommand will be added later,
allowing the user to interactively define mappings by playing a MIDI event
and then pressing the desired keyboard combo. The resulting mapping will be
appended to the config file.

Design note: the config path argument to `learn` should offer to append to
an existing file rather than overwrite it.
