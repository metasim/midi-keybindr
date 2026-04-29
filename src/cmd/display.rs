// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)

//! Implementation of the `display` subcommand: a live ratatui TUI that shows
//! incoming MIDI events (right pane) and keyboard events (left pane) in
//! scrollable tables.  Each row includes a one-line JSON snippet that a user
//! can paste directly into their config file.

use std::io;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState};

use crate::cli::DisplayArgs;
use crate::midi::event::{IncomingMidiEvent, SysRtKind};
use crate::midi::{event as midi_event, port};

// ─── Records ────────────────────────────────────────────────────────────────

/// A single captured MIDI event, stored for display.
pub struct MidiEventRecord {
    /// Monotonically increasing sequence number (1-based).
    pub seq: u64,
    /// Name of the MIDI port the event arrived on.
    pub port: String,
    /// MIDI channel (1–16), or `None` for System Real-Time messages.
    pub channel: Option<u8>,
    /// Normalized MIDI event.
    pub event: IncomingMidiEvent,
}

/// A single captured keyboard event, stored for display.
pub struct KeyboardEventRecord {
    /// Monotonically increasing sequence number (1-based).
    pub seq: u64,
    /// The raw crossterm key event.
    pub key_event: KeyEvent,
}

// ─── Pane selection ─────────────────────────────────────────────────────────

/// Which of the two display panes currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Keyboard,
    Midi,
}

// ─── App state ──────────────────────────────────────────────────────────────

/// All mutable state owned by the display TUI.
pub struct AppState {
    /// Captured MIDI events, oldest first.
    pub midi_events: Vec<MidiEventRecord>,
    /// Captured keyboard events, oldest first.
    pub keyboard_events: Vec<KeyboardEventRecord>,
    /// Which pane currently has keyboard focus.
    pub focused_pane: Pane,
    /// `TableState` for the MIDI events pane.
    pub midi_table: TableState,
    /// `TableState` for the keyboard events pane.
    pub keyboard_table: TableState,
    /// Set to `false` to exit the event loop.
    pub running: bool,
    /// When `true` the keyboard pane scrolls to the newest event automatically.
    pub keyboard_auto_scroll: bool,
    /// When `true` the MIDI pane scrolls to the newest event automatically.
    pub midi_auto_scroll: bool,
    midi_seq: u64,
    keyboard_seq: u64,
}

impl AppState {
    /// Creates an empty initial state.
    pub fn new() -> Self {
        Self {
            midi_events: Vec::new(),
            keyboard_events: Vec::new(),
            focused_pane: Pane::Keyboard,
            midi_table: TableState::new(),
            keyboard_table: TableState::new(),
            running: true,
            keyboard_auto_scroll: true,
            midi_auto_scroll: true,
            midi_seq: 0,
            keyboard_seq: 0,
        }
    }

    /// Appends a MIDI event and (when in auto-scroll mode) advances the
    /// selection to the new last row.
    pub fn add_midi_event(&mut self, port: String, channel: Option<u8>, event: IncomingMidiEvent) {
        self.midi_seq += 1;
        self.midi_events.push(MidiEventRecord {
            seq: self.midi_seq,
            port,
            channel,
            event,
        });
        if self.midi_auto_scroll {
            self.midi_table
                .select(Some(self.midi_events.len().saturating_sub(1)));
        }
    }

    /// Appends a keyboard event and (when in auto-scroll mode) advances the
    /// selection to the new last row.
    pub fn add_keyboard_event(&mut self, key_event: KeyEvent) {
        self.keyboard_seq += 1;
        self.keyboard_events.push(KeyboardEventRecord {
            seq: self.keyboard_seq,
            key_event,
        });
        if self.keyboard_auto_scroll {
            self.keyboard_table
                .select(Some(self.keyboard_events.len().saturating_sub(1)));
        }
    }

    /// Switches focus between the keyboard and MIDI panes.
    pub fn toggle_pane(&mut self) {
        self.focused_pane = match self.focused_pane {
            Pane::Keyboard => Pane::Midi,
            Pane::Midi => Pane::Keyboard,
        };
    }

    /// Scrolls the focused pane up by one row and disables auto-scroll.
    pub fn scroll_up(&mut self) {
        match self.focused_pane {
            Pane::Keyboard => {
                self.keyboard_auto_scroll = false;
                let current = self.keyboard_table.selected().unwrap_or(0);
                self.keyboard_table.select(Some(current.saturating_sub(1)));
            }
            Pane::Midi => {
                self.midi_auto_scroll = false;
                let current = self.midi_table.selected().unwrap_or(0);
                self.midi_table.select(Some(current.saturating_sub(1)));
            }
        }
    }

    /// Scrolls the focused pane down by one row, re-enabling auto-scroll when
    /// the last row is reached.
    pub fn scroll_down(&mut self) {
        match self.focused_pane {
            Pane::Keyboard => {
                let max = self.keyboard_events.len().saturating_sub(1);
                let next = (self.keyboard_table.selected().unwrap_or(0) + 1).min(max);
                self.keyboard_table.select(Some(next));
                if next >= max {
                    self.keyboard_auto_scroll = true;
                }
            }
            Pane::Midi => {
                let max = self.midi_events.len().saturating_sub(1);
                let next = (self.midi_table.selected().unwrap_or(0) + 1).min(max);
                self.midi_table.select(Some(next));
                if next >= max {
                    self.midi_auto_scroll = true;
                }
            }
        }
    }

    /// Ensures that auto-scroll selections are up to date just before rendering.
    ///
    /// Called at the top of every `draw` invocation so that new events are
    /// visible even when a manual scroll is not in progress.
    pub fn prepare_render(&mut self) {
        if self.keyboard_auto_scroll && !self.keyboard_events.is_empty() {
            self.keyboard_table
                .select(Some(self.keyboard_events.len() - 1));
        }
        if self.midi_auto_scroll && !self.midi_events.is_empty() {
            self.midi_table.select(Some(self.midi_events.len() - 1));
        }
    }
}

// ─── JSON config representation ─────────────────────────────────────────────

/// Returns a one-line JSON snippet suitable for use as a `trigger:` block in
/// the config file.
///
/// # Examples
///
/// ```
/// use midi_keybindr::midi::event::IncomingMidiEvent;
/// use midi_keybindr::cmd::display::midi_config_json;
/// let json = midi_config_json(&IncomingMidiEvent::NoteOn { note: 60 }, Some(1));
/// assert_eq!(json, r#"{"type":"note_on","note":60,"channel":1}"#);
/// ```
pub fn midi_config_json(event: &IncomingMidiEvent, channel: Option<u8>) -> String {
    let ch_part = channel.map_or_else(String::new, |c| format!(r#","channel":{c}"#));
    match event {
        IncomingMidiEvent::NoteOn { note } => {
            format!(r#"{{"type":"note_on","note":{note}{ch_part}}}"#)
        }
        IncomingMidiEvent::NoteOff { note } => {
            format!(r#"{{"type":"note_off","note":{note}{ch_part}}}"#)
        }
        IncomingMidiEvent::ControlChange { cc, value } => {
            format!(r#"{{"type":"control_change","cc":{cc},"value":{value}{ch_part}}}"#)
        }
        IncomingMidiEvent::ProgramChange { program } => {
            format!(r#"{{"type":"program_change","program":{program}{ch_part}}}"#)
        }
        IncomingMidiEvent::SysRealTime(kind) => {
            let kind_str = match kind {
                SysRtKind::Start => "start",
                SysRtKind::Stop => "stop",
                SysRtKind::Continue => "continue",
            };
            format!(r#"{{"type":"sys_real_time","kind":"{kind_str}"{ch_part}}}"#)
        }
    }
}

/// Returns a one-line JSON snippet suitable for use as an `action:` block in
/// the config file.
///
/// # Examples
///
/// ```
/// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use midi_keybindr::cmd::display::keyboard_config_json;
/// let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
/// assert_eq!(keyboard_config_json(&key), r#"{"keys":"Ctrl+a"}"#);
/// ```
pub fn keyboard_config_json(key_event: &KeyEvent) -> String {
    let action = crossterm_key_to_action_string(key_event);
    format!(r#"{{"keys":"{action}"}}"#)
}

/// Converts a crossterm `KeyEvent` to the key-combo string format used in
/// config files (e.g. `"Ctrl+Shift+F5"`).
pub fn crossterm_key_to_action_string(key_event: &KeyEvent) -> String {
    let mut parts: Vec<String> = Vec::new();

    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_owned());
    }
    if key_event.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_owned());
    }
    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_owned());
    }
    if key_event.modifiers.contains(KeyModifiers::SUPER) {
        parts.push("Cmd".to_owned());
    }

    let key_str = match key_event.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::BackTab => "Tab".to_owned(), // crossterm's distinct code for Shift+Tab
        KeyCode::Backspace => "Backspace".to_owned(),
        KeyCode::Delete => "Delete".to_owned(),
        KeyCode::Esc => "Esc".to_owned(),
        KeyCode::Up => "Up".to_owned(),
        KeyCode::Down => "Down".to_owned(),
        KeyCode::Left => "Left".to_owned(),
        KeyCode::Right => "Right".to_owned(),
        ref other => format!("{other:?}"),
    };
    parts.push(key_str);
    parts.join("+")
}

// ─── Display helpers ─────────────────────────────────────────────────────────

/// Returns a short human-readable type label for a MIDI event.
fn midi_event_type(event: &IncomingMidiEvent) -> &'static str {
    match event {
        IncomingMidiEvent::NoteOn { .. } => "NoteOn",
        IncomingMidiEvent::NoteOff { .. } => "NoteOff",
        IncomingMidiEvent::ControlChange { .. } => "CC",
        IncomingMidiEvent::ProgramChange { .. } => "ProgChg",
        IncomingMidiEvent::SysRealTime(_) => "SysRT",
    }
}

/// Returns a compact detail string for a MIDI event.
fn midi_event_details(event: &IncomingMidiEvent) -> String {
    match event {
        IncomingMidiEvent::NoteOn { note } | IncomingMidiEvent::NoteOff { note } => {
            format!("note={note}")
        }
        IncomingMidiEvent::ControlChange { cc, value } => format!("cc={cc} val={value}"),
        IncomingMidiEvent::ProgramChange { program } => format!("prog={program}"),
        IncomingMidiEvent::SysRealTime(kind) => format!("{kind:?}"),
    }
}

/// Truncates `s` to at most `max_chars` characters, appending `…` if cut.
fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        // There were more characters beyond max_chars.
        let mut truncated: String = collected.chars().take(max_chars - 1).collect();
        truncated.push('…');
        truncated
    } else {
        collected
    }
}

// ─── UI rendering ────────────────────────────────────────────────────────────

/// Renders the full TUI into `frame`.
///
/// This is a pure function over `state` (plus the mutable `TableState` fields
/// inside `AppState`) so it can be called with a `TestBackend` in unit tests.
pub fn draw(frame: &mut ratatui::Frame, state: &mut AppState) {
    let area = frame.area();

    // Overall layout: main content | one-line footer
    let [main_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

    // Horizontal split: keyboard events (left) | MIDI events (right)
    let [kb_area, midi_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main_area);

    state.prepare_render();

    // ── Keyboard Events pane ─────────────────────────────────────────────────
    let kb_focused = state.focused_pane == Pane::Keyboard;
    let kb_border = if kb_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let kb_header = Row::new([
        Cell::from("#").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Key").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Config JSON").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let kb_rows: Vec<Row> = state
        .keyboard_events
        .iter()
        .map(|r| {
            Row::new([
                Cell::from(r.seq.to_string()),
                Cell::from(crossterm_key_to_action_string(&r.key_event)),
                Cell::from(keyboard_config_json(&r.key_event)),
            ])
        })
        .collect();

    let kb_table = Table::new(
        kb_rows,
        [
            Constraint::Length(4),
            Constraint::Length(20),
            Constraint::Fill(1),
        ],
    )
    .header(kb_header)
    .block(
        Block::bordered()
            .title(" Keyboard Events ")
            .border_style(kb_border),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(kb_table, kb_area, &mut state.keyboard_table);

    // ── MIDI Events pane ─────────────────────────────────────────────────────
    let midi_focused = state.focused_pane == Pane::Midi;
    let midi_border = if midi_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let midi_header = Row::new([
        Cell::from("#").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Port").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Ch").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Details").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Config JSON").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let midi_rows: Vec<Row> = state
        .midi_events
        .iter()
        .map(|r| {
            Row::new([
                Cell::from(r.seq.to_string()),
                Cell::from(truncate_str(&r.port, 14)),
                Cell::from(
                    r.channel
                        .map_or_else(|| "RT".to_string(), |c| c.to_string()),
                ),
                Cell::from(midi_event_type(&r.event)),
                Cell::from(midi_event_details(&r.event)),
                Cell::from(midi_config_json(&r.event, r.channel)),
            ])
        })
        .collect();

    let midi_table = Table::new(
        midi_rows,
        [
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Length(14),
            Constraint::Fill(1),
        ],
    )
    .header(midi_header)
    .block(
        Block::bordered()
            .title(" MIDI Events ")
            .border_style(midi_border),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(midi_table, midi_area, &mut state.midi_table);

    // ── Footer ───────────────────────────────────────────────────────────────
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Ctrl+C", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": quit  "),
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": switch pane  "),
        Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": scroll  — "),
        Span::raw(format!(
            "Keyboard: {}  MIDI: {}",
            state.keyboard_events.len(),
            state.midi_events.len()
        )),
    ]));
    frame.render_widget(footer, footer_area);
}

// ─── Execute ─────────────────────────────────────────────────────────────────

/// Executes the `display` subcommand: connects to all available MIDI ports,
/// opens a full-screen TUI, and streams live events until the user quits.
///
/// **Navigation keys** (consumed for UI control, not shown in the table):
/// - `Ctrl+C` / `Ctrl+Q` — quit
/// - `Tab` — switch focused pane
/// - `↑` / `↓` — scroll selected pane
///
/// All other key events are captured and shown in the _Keyboard Events_ pane.
pub fn execute(_args: DisplayArgs) -> Result<()> {
    let (midi_tx, midi_rx) = mpsc::channel::<(String, Option<u8>, IncomingMidiEvent)>();

    // Attempt to connect to all available MIDI ports.  A failure here is not
    // fatal: the TUI will still run and show keyboard events.
    let _connections = port::connect_all(move |port_name| {
        let tx = midi_tx.clone();
        Box::new(move |_ts, message| {
            if let Ok(Some(parsed)) = midi_event::parse_message(message) {
                let _ = tx.send((port_name.clone(), parsed.channel, parsed.event));
            }
        })
    });

    // Set up the terminal.
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();
    let result = run_event_loop(&mut terminal, &mut state, &midi_rx);

    // Always restore the terminal, even on error.
    crossterm::terminal::disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Inner event loop, separated from `execute` so terminal cleanup is always
/// performed regardless of whether the loop returns an error.
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    midi_rx: &mpsc::Receiver<(String, Option<u8>, IncomingMidiEvent)>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, state))?;

        // Poll for keyboard events; 16 ms ≈ 60 fps.
        if crossterm::event::poll(Duration::from_millis(16))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            match (key.modifiers, key.code) {
                // Navigation / control keys — consumed, not recorded.
                (KeyModifiers::CONTROL, KeyCode::Char('c' | 'q')) => {
                    state.running = false;
                }
                (KeyModifiers::NONE, KeyCode::Tab) | (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                    state.toggle_pane();
                }
                (KeyModifiers::NONE, KeyCode::Up) => state.scroll_up(),
                (KeyModifiers::NONE, KeyCode::Down) => state.scroll_down(),
                // All other keys are shown in the keyboard events pane.
                _ => state.add_keyboard_event(key),
            }
        }

        if !state.running {
            break;
        }

        // Drain all buffered MIDI events.
        while let Ok((port, channel, event)) = midi_rx.try_recv() {
            state.add_midi_event(port, channel, event);
        }
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    /// Converts a ratatui `Buffer` to a plain string (one line per row).
    fn buffer_to_string(buf: &Buffer) -> String {
        let area = buf.area;
        let mut s = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    /// Renders `state` into an 80×24 `TestBackend` and returns the text.
    fn render(state: &mut AppState) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, state)).unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    // ── midi_config_json ─────────────────────────────────────────────────────

    /// NoteOn produces the correct trigger JSON including channel.
    #[test]
    fn midi_json_note_on_with_channel() {
        let event = IncomingMidiEvent::NoteOn { note: 60 };
        assert_eq!(
            midi_config_json(&event, Some(1)),
            r#"{"type":"note_on","note":60,"channel":1}"#
        );
    }

    /// NoteOff omits channel when it is `None`.
    #[test]
    fn midi_json_note_off_no_channel() {
        let event = IncomingMidiEvent::NoteOff { note: 36 };
        assert_eq!(
            midi_config_json(&event, None),
            r#"{"type":"note_off","note":36}"#
        );
    }

    /// ControlChange includes both cc and value.
    #[test]
    fn midi_json_control_change() {
        let event = IncomingMidiEvent::ControlChange { cc: 7, value: 100 };
        assert_eq!(
            midi_config_json(&event, Some(2)),
            r#"{"type":"control_change","cc":7,"value":100,"channel":2}"#
        );
    }

    /// ProgramChange JSON is correct.
    #[test]
    fn midi_json_program_change() {
        let event = IncomingMidiEvent::ProgramChange { program: 5 };
        assert_eq!(
            midi_config_json(&event, None),
            r#"{"type":"program_change","program":5}"#
        );
    }

    /// SysRealTime Start produces the correct JSON.
    #[test]
    fn midi_json_sysrt_start() {
        let event = IncomingMidiEvent::SysRealTime(SysRtKind::Start);
        assert_eq!(
            midi_config_json(&event, None),
            r#"{"type":"sys_real_time","kind":"start"}"#
        );
    }

    /// SysRealTime Stop and Continue produce correct `kind` strings.
    #[test]
    fn midi_json_sysrt_stop_and_continue() {
        assert_eq!(
            midi_config_json(&IncomingMidiEvent::SysRealTime(SysRtKind::Stop), None),
            r#"{"type":"sys_real_time","kind":"stop"}"#
        );
        assert_eq!(
            midi_config_json(&IncomingMidiEvent::SysRealTime(SysRtKind::Continue), None),
            r#"{"type":"sys_real_time","kind":"continue"}"#
        );
    }

    // ── keyboard_config_json ─────────────────────────────────────────────────

    /// Plain character produces `{"keys":"a"}`.
    #[test]
    fn kb_json_plain_char() {
        assert_eq!(
            keyboard_config_json(&key(KeyCode::Char('a'), KeyModifiers::NONE)),
            r#"{"keys":"a"}"#
        );
    }

    /// Control+character produces `{"keys":"Ctrl+c"}`.
    #[test]
    fn kb_json_ctrl_char() {
        assert_eq!(
            keyboard_config_json(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            r#"{"keys":"Ctrl+c"}"#
        );
    }

    /// Shift modifier is included.
    #[test]
    fn kb_json_shift_modifier() {
        assert_eq!(
            keyboard_config_json(&key(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            r#"{"keys":"Shift+A"}"#
        );
    }

    /// Function keys are expressed as `F1`..`F12`.
    #[test]
    fn kb_json_function_key() {
        assert_eq!(
            keyboard_config_json(&key(KeyCode::F(8), KeyModifiers::NONE)),
            r#"{"keys":"F8"}"#
        );
    }

    /// Named keys (Esc, Enter, etc.) are spelled out.
    #[test]
    fn kb_json_named_keys() {
        for (code, expected) in [
            (KeyCode::Esc, r#"{"keys":"Esc"}"#),
            (KeyCode::Enter, r#"{"keys":"Enter"}"#),
            (KeyCode::Tab, r#"{"keys":"Tab"}"#),
            (KeyCode::Backspace, r#"{"keys":"Backspace"}"#),
            (KeyCode::Delete, r#"{"keys":"Delete"}"#),
            (KeyCode::Up, r#"{"keys":"Up"}"#),
            (KeyCode::Down, r#"{"keys":"Down"}"#),
            (KeyCode::Left, r#"{"keys":"Left"}"#),
            (KeyCode::Right, r#"{"keys":"Right"}"#),
        ] {
            assert_eq!(
                keyboard_config_json(&key(code, KeyModifiers::NONE)),
                expected,
                "failed for {code:?}"
            );
        }
    }

    /// Multi-modifier combo is expressed in order Ctrl > Alt > Shift.
    #[test]
    fn kb_json_multi_modifier() {
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            keyboard_config_json(&key(KeyCode::Char('z'), modifiers)),
            r#"{"keys":"Ctrl+Shift+z"}"#
        );
    }

    // ── truncate_str ─────────────────────────────────────────────────────────

    /// Short strings are returned unchanged.
    #[test]
    fn truncate_short_str_unchanged() {
        assert_eq!(truncate_str("hi", 10), "hi");
    }

    /// Long strings are truncated and get a trailing ellipsis.
    #[test]
    fn truncate_long_str_gets_ellipsis() {
        let result = truncate_str("hello world", 7);
        assert_eq!(result.chars().count(), 7);
        assert!(result.ends_with('…'));
    }

    // ── AppState ─────────────────────────────────────────────────────────────

    /// A fresh `AppState` is empty and running.
    #[test]
    fn app_state_initial_values() {
        let s = AppState::new();
        assert!(s.midi_events.is_empty());
        assert!(s.keyboard_events.is_empty());
        assert!(s.running);
        assert_eq!(s.focused_pane, Pane::Keyboard);
        assert!(s.keyboard_auto_scroll);
        assert!(s.midi_auto_scroll);
    }

    /// `add_midi_event` increments the sequence counter.
    #[test]
    fn add_midi_event_increments_seq() {
        let mut s = AppState::new();
        s.add_midi_event(
            "port".into(),
            Some(1),
            IncomingMidiEvent::NoteOn { note: 60 },
        );
        s.add_midi_event(
            "port".into(),
            Some(1),
            IncomingMidiEvent::NoteOn { note: 62 },
        );
        assert_eq!(s.midi_events[0].seq, 1);
        assert_eq!(s.midi_events[1].seq, 2);
    }

    /// `add_keyboard_event` increments the sequence counter.
    #[test]
    fn add_keyboard_event_increments_seq() {
        let mut s = AppState::new();
        s.add_keyboard_event(key(KeyCode::Char('a'), KeyModifiers::NONE));
        s.add_keyboard_event(key(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(s.keyboard_events[0].seq, 1);
        assert_eq!(s.keyboard_events[1].seq, 2);
    }

    /// Auto-scroll keeps the selection at the last MIDI event.
    #[test]
    fn midi_auto_scroll_tracks_last_row() {
        let mut s = AppState::new();
        for note in 0..5u8 {
            s.add_midi_event("p".into(), None, IncomingMidiEvent::NoteOn { note });
        }
        assert_eq!(s.midi_table.selected(), Some(4));
    }

    /// `toggle_pane` switches focus back and forth.
    #[test]
    fn toggle_pane_alternates() {
        let mut s = AppState::new();
        assert_eq!(s.focused_pane, Pane::Keyboard);
        s.toggle_pane();
        assert_eq!(s.focused_pane, Pane::Midi);
        s.toggle_pane();
        assert_eq!(s.focused_pane, Pane::Keyboard);
    }

    /// `scroll_up` disables auto-scroll and moves the selection back.
    #[test]
    fn scroll_up_disables_auto_scroll() {
        let mut s = AppState::new();
        for note in 0..5u8 {
            s.add_keyboard_event(key(
                KeyCode::Char((b'a' + note) as char),
                KeyModifiers::NONE,
            ));
        }
        // Should be at row 4 due to auto-scroll.
        assert_eq!(s.keyboard_table.selected(), Some(4));
        s.scroll_up();
        assert!(!s.keyboard_auto_scroll);
        assert_eq!(s.keyboard_table.selected(), Some(3));
    }

    /// `scroll_down` to the last row re-enables auto-scroll.
    #[test]
    fn scroll_down_to_last_row_reenables_auto_scroll() {
        let mut s = AppState::new();
        for note in 0..3u8 {
            s.add_keyboard_event(key(
                KeyCode::Char((b'a' + note) as char),
                KeyModifiers::NONE,
            ));
        }
        s.scroll_up(); // now at row 1, auto_scroll = false
        s.scroll_down(); // row 2
        s.scroll_down(); // row 2 (max), re-enables auto_scroll
        assert!(s.keyboard_auto_scroll);
    }

    /// `scroll_up` on an empty pane does not panic.
    #[test]
    fn scroll_up_empty_pane_no_panic() {
        let mut s = AppState::new();
        s.scroll_up(); // should not panic
    }

    // ── UI smoke / integration tests ─────────────────────────────────────────

    /// The TUI renders both pane titles into the terminal buffer.
    #[test]
    fn ui_renders_both_pane_titles() {
        let mut state = AppState::new();
        let rendered = render(&mut state);
        assert!(
            rendered.contains("Keyboard Events"),
            "missing 'Keyboard Events' in:\n{rendered}"
        );
        assert!(
            rendered.contains("MIDI Events"),
            "missing 'MIDI Events' in:\n{rendered}"
        );
    }

    /// The footer help text is visible.
    #[test]
    fn ui_renders_footer_help() {
        let mut state = AppState::new();
        let rendered = render(&mut state);
        assert!(
            rendered.contains("Ctrl+C"),
            "missing footer in:\n{rendered}"
        );
    }

    /// MIDI and keyboard events appear in their respective panes after being added.
    #[test]
    fn ui_renders_added_events() {
        let mut state = AppState::new();
        state.add_midi_event(
            "Test Port".into(),
            Some(1),
            IncomingMidiEvent::NoteOn { note: 60 },
        );
        state.add_keyboard_event(key(KeyCode::Char('z'), KeyModifiers::NONE));

        let rendered = render(&mut state);
        assert!(
            rendered.contains("NoteOn"),
            "missing NoteOn in:\n{rendered}"
        );
        assert!(rendered.contains('z'), "missing 'z' key in:\n{rendered}");
    }

    /// Snapshot of the empty initial state.
    ///
    /// This test verifies that the overall layout is stable.  Run
    /// `INSTA_UPDATE=new cargo test` once to create the snapshot, then commit
    /// the generated file in `src/snapshots/`.
    #[test]
    fn ui_snapshot_empty_state() {
        let mut state = AppState::new();
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, &mut state)).unwrap();
        insta::assert_snapshot!(buffer_to_string(terminal.backend().buffer()));
    }
}
