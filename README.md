# midi-keybindr

A Rust CLI tool that maps MIDI events to keyboard shortcuts, routing MIDI
controller input to synthesized keyboard events delivered to whatever
application is currently in focus.

---

## Features

- Maps **Note On/Off**, **Control Change**, and **Program Change** MIDI events
  to any keyboard shortcut.
- Matches on specific **devices** (by case-insensitive glob), **channels**
  (single, range, or list), and **MIDI values** (exact or range).
- Supports **scientific pitch notation** for notes (`C4`, `Bb3`, `C-1`) as
  well as raw MIDI note numbers (`60`).
- Human-readable YAML configuration.
- `list` subcommand enumerates available MIDI input ports.
- Structured logging with configurable verbosity (`-v`, `-vv`, `-vvv`).

---

## Requirements

- **macOS** (primary target). Linux and Windows are secondary targets.
- On macOS, the process (or its parent terminal) must be granted
  **Accessibility** access:

  > System Settings → Privacy & Security → Accessibility

  macOS silently drops synthesized key events when permission is absent.
  `midi-keybindr` checks for this at startup and exits with a clear diagnostic
  message if permission is missing.

---

## Installation

```bash
cargo install --path .
```

---

## Usage

```
midi-keybindr [OPTIONS] [COMMAND]

Commands:
  list    List available MIDI input ports
  help    Print this message or the help of the given subcommand(s)

Options:
  -c, --config <PATH>   Path to config file (overrides default search locations)
  -v, --verbose...      Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
  -h, --help            Print help
  -V, --version         Print version
```

### List available MIDI ports

```bash
midi-keybindr list
midi-keybindr list --format json
```

Example output:

```
Index  Name
─────  ────────────────────────────────
0      Arturia KeyStep Pro MIDI In
1      IAC Driver Bus 1
2      USB MIDI Interface
```

### Run the mapper

```bash
midi-keybindr                          # uses default config search path
midi-keybindr --config ~/my-config.yaml
```

---

## Configuration

### Config file search order

1. `--config <path>` CLI flag
2. `$MIDI_KEYBINDR_CONFIG` environment variable
3. `~/.config/midi-keybindr/config.yaml`

### Config file format

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

  - description: "Program change 5 on any device → Ctrl+Alt+P"
    trigger:
      type: program_change
      program: 5
    action:
      keys: "Ctrl+Alt+P"
```

### Field reference

#### `devices`

Glob pattern(s) matched case-insensitively against the MIDI port name. Omit
to match all devices.

```yaml
devices: "KeyStep*"            # single pattern
devices: ["*Akai*", "*Korg*"]  # match either
devices: "*"                   # all devices (same as omitting)
```

#### `channel`

One-based MIDI channel(s) to match (1–16). Omit to match all channels.

```yaml
channel: 1            # single channel
channel: "*"          # all channels
channel: ["1-3", "9"] # channels 1, 2, 3, and 9
```

#### `trigger`

| Type | Required fields | Optional fields |
|---|---|---|
| `note_on` | `note` | — |
| `note_off` | `note` | — |
| `control_change` | `cc` | `value: { min, max }` |
| `program_change` | `program` | — |

**Note syntax** (`note` field): accepts a MIDI integer (`0`–`127`) or a
note in scientific pitch notation:

| Form | Examples |
|---|---|
| MIDI integer | `60`, `21` |
| Natural | `C4`, `A0`, `G9` |
| Sharp | `C#4`, `A#0` |
| Flat | `Bb4`, `Eb3`, `Ab2` |
| Enharmonic | `B#3` (= C4), `Cb4` (= B3) |
| Negative octave | `C-1` (MIDI 0) |

C4 = middle C = MIDI 60.

#### `action`

```yaml
action:
  keys: "Cmd+Shift+3"
```

**Modifier tokens** (case-insensitive): `Ctrl`/`Control`, `Shift`,
`Alt`/`Opt`/`Option`, `Cmd`/`Command`/`Meta`/`Super`.

**Primary key tokens**: single Unicode character (`a`, `3`, `/`), named keys
(`Space`, `Tab`, `Enter`/`Return`, `Up`, `Down`, `Left`, `Right`,
`Esc`/`Escape`, `Backspace`, `Delete`), or function keys (`F1`–`F12`).

---

## Verbosity levels

| Flag | Log level |
|---|---|
| (none) | `WARN` — only errors and warnings |
| `-v` | `INFO` — startup info and matched events |
| `-vv` | `DEBUG` — all MIDI events received |
| `-vvv` | `TRACE` — full internal detail |

The `RUST_LOG` environment variable overrides verbosity per module, e.g.:

```bash
RUST_LOG=midi_keybindr::midi=trace midi-keybindr -v
```

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option.