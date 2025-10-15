#![allow(unused)]

//! Minimal telnet-friendly line editor with history and cursor movement.
//!
//! Usage pattern (pseudo):
//!   let mut ed = LineEditor::new("> ");
//!   write!(stream, "{}", ed.repaint_line())?;
//!   loop {
//!       let b = read_one_byte(...);
//!       match ed.handle_byte(b) {
//!           EditEvent::Line(line) => { /* handle command */ write!(stream, "\n{}", ed.repaint_line())?; }
//!           EditEvent::Redraw     => { write!(stream, "{}", ed.repaint_line())?; }
//!           EditEvent::None       => {}
//!       }
//!   }
//!
//! Notes:
//! - Treats bytes as single columns (ASCII). For full Unicode widths, integrate `unicode-width` later.
//! - Designed for remote terminals (Telnet). You parse IAC/NAWS/etc. elsewhere; feed *post-negotiation* bytes here.

use std::cmp::min;

/// Events produced by the editor as it processes input.
#[derive(Debug)]
pub enum EditEvent {
    /// A full line was submitted (user pressed Enter/Return).
    Line(String),
    /// The current line changed; repaint with `repaint_line()`.
    Redraw,
    /// No visible change.
    None,
}

/// Configurable caps for the editor.
#[derive(Debug, Clone)]
pub struct EditorConfig {
    /// Max number of entries to retain in history.
    pub max_history: usize,
    /// If true, prevent pushing duplicate consecutive history items.
    pub dedup_consecutive_history: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            max_history: 200,
            dedup_consecutive_history: true,
        }
    }
}

/// A minimal readline-like line editor: history, cursor, basic ANSI keys.
#[derive(Debug)]
pub struct LineEditor {
    prompt: String,
    buf: String,
    cursor: usize, // byte index within buf (ASCII assumed)
    esc: Vec<u8>,  // accumulating an escape sequence (CSI, SS3)
    pub history: Vec<String>,
    hist_ix: Option<usize>, // index into history while navigating (None = editing new line)
    cfg: EditorConfig,
}

impl LineEditor {
    /// Create a new editor with the given prompt and default config.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self::with_config(prompt, EditorConfig::default())
    }

    /// Create a new editor with custom config.
    pub fn with_config(prompt: impl Into<String>, cfg: EditorConfig) -> Self {
        Self {
            prompt: prompt.into(),
            buf: String::new(),
            cursor: 0,
            esc: Vec::new(),
            history: Vec::new(),
            hist_ix: None,
            cfg,
        }
    }

    /// Change the prompt string.
    pub fn set_prompt(&mut self, p: impl Into<String>) {
        self.prompt = p.into();
    }

    /// Replace the entire history (e.g., after loading from disk).
    pub fn set_history(&mut self, items: Vec<String>) {
        self.history = items;
        self.trim_history();
    }

    /// Push a line into history manually (dedup + cap).
    pub fn push_history(&mut self, line: String) {
        if line.trim().is_empty() {
            return;
        }
        if self.cfg.dedup_consecutive_history && self.history.last() == Some(&line) {
            return;
        }
        self.history.push(line);
        self.trim_history();
    }

    fn trim_history(&mut self) {
        if self.history.len() > self.cfg.max_history {
            let overflow = self.history.len() - self.cfg.max_history;
            self.history.drain(0..overflow);
        }
    }

    /// Handle a single input byte. Call this for each byte arriving from the client.
    pub fn handle_byte(&mut self, b: u8) -> EditEvent {
        // Escape sequence collection
        if !self.esc.is_empty() || b == 0x1B {
            self.esc.push(b);
            return self.handle_esc();
        }

        match b {
            b'\r' | b'\n' => {
                // Submit line
                let line = std::mem::take(&mut self.buf);
                self.cursor = 0;
                self.hist_ix = None;
                if !line.trim().is_empty() {
                    self.push_history(line.clone());
                }
                EditEvent::Line(line)
            }

            0x7F | 0x08 => {
                // Backspace / Ctrl-H
                if self.cursor > 0 {
                    let prev = self.cursor - 1;
                    self.buf.remove(prev);
                    self.cursor = prev;
                }
                EditEvent::Redraw
            }

            0x01 => {
                // Ctrl-A (Home)
                self.cursor = 0;
                EditEvent::Redraw
            }
            0x05 => {
                // Ctrl-E (End)
                self.cursor = self.buf.len();
                EditEvent::Redraw
            }
            0x15 => {
                // Ctrl-U (kill line)
                self.buf.clear();
                self.cursor = 0;
                self.hist_ix = None;
                EditEvent::Redraw
            }
            0x17 => {
                // Ctrl-W (kill previous word)
                if self.cursor > 0 {
                    // Trim trailing spaces before the cursor
                    while self.cursor > 0 && self.buf.as_bytes()[self.cursor - 1].is_ascii_whitespace() {
                        self.buf.remove(self.cursor - 1);
                        self.cursor -= 1;
                    }
                    // Remove word chars
                    while self.cursor > 0 && !self.buf.as_bytes()[self.cursor - 1].is_ascii_whitespace() {
                        self.buf.remove(self.cursor - 1);
                        self.cursor -= 1;
                    }
                }
                EditEvent::Redraw
            }

            // Printable ASCII
            b if (0x20..=0x7E).contains(&b) => {
                self.buf.insert(self.cursor, b as char);
                self.cursor += 1;
                self.hist_ix = None; // stop history browsing once editing resumes
                EditEvent::Redraw
            }

            _ => EditEvent::None,
        }
    }

    fn handle_esc(&mut self) -> EditEvent {
        // Common sequences we support:
        // CSI (ESC [ ...):
        //   A=Up, B=Down, C=Right, D=Left, H=Home, F=End
        //   3~=Delete
        // SS3 (ESC O ...):
        //   H=Home, F=End  (some terminals)
        let s = &self.esc.as_slice();

        // Arrow keys: ESC [ A/B/C/D
        if s == b"\x1B[A" {
            self.esc.clear();
            return self.hist_prev();
        }
        if s == b"\x1B[B" {
            self.esc.clear();
            return self.hist_next();
        }
        if s == b"\x1B[C" {
            self.esc.clear();
            return self.move_right();
        }
        if s == b"\x1B[D" {
            self.esc.clear();
            return self.move_left();
        }

        // Home/End via CSI
        if s == b"\x1B[H" {
            self.esc.clear();
            self.cursor = 0;
            return EditEvent::Redraw;
        }
        if s == b"\x1B[F" {
            self.esc.clear();
            self.cursor = self.buf.len();
            return EditEvent::Redraw;
        }

        // Delete: ESC [ 3 ~
        if s == b"\x1B[3~" {
            self.esc.clear();
            if self.cursor < self.buf.len() {
                self.buf.remove(self.cursor);
            }
            return EditEvent::Redraw;
        }

        // SS3 Home/End: ESC O H/F
        if s == b"\x1B[OH" {
            self.esc.clear();
            self.cursor = 0;
            return EditEvent::Redraw;
        }
        if s == b"\x1B[OF" {
            self.esc.clear();
            self.cursor = self.buf.len();
            return EditEvent::Redraw;
        }

        if s.starts_with(b"\x1b[") {
            if let Some(&last) = s.last() {
                let is_final = (last.is_ascii_alphabetic()) || last == b'~';
                return if is_final {
                    self.esc.clear();
                    EditEvent::None
                } else {
                    EditEvent::None // keep accumulating
                };
            }
            return EditEvent::None;
        }

        // Otherwise ignore.
        EditEvent::None
    }

    fn move_left(&mut self) -> EditEvent {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        EditEvent::Redraw
    }
    fn move_right(&mut self) -> EditEvent {
        if self.cursor < self.buf.len() {
            self.cursor += 1;
        }
        EditEvent::Redraw
    }

    fn hist_prev(&mut self) -> EditEvent {
        if self.history.is_empty() {
            return EditEvent::None;
        }
        match self.hist_ix {
            None => {
                self.hist_ix = Some(self.history.len().saturating_sub(1));
            }
            Some(ix) => {
                if ix > 0 {
                    self.hist_ix = Some(ix - 1);
                }
            }
        }
        if let Some(ix) = self.hist_ix {
            self.buf = self.history[ix].clone();
            self.cursor = self.buf.len();
            EditEvent::Redraw
        } else {
            EditEvent::None
        }
    }

    fn hist_next(&mut self) -> EditEvent {
        if self.history.is_empty() {
            return EditEvent::None;
        }
        match self.hist_ix {
            None => EditEvent::None,
            Some(ix) => {
                if ix + 1 >= self.history.len() {
                    // Past newest: clear to blank line
                    self.hist_ix = None;
                    self.buf.clear();
                    self.cursor = 0;
                    EditEvent::Redraw
                } else {
                    let next = min(ix + 1, self.history.len() - 1);
                    self.hist_ix = Some(next);
                    self.buf = self.history[next].clone();
                    self.cursor = self.buf.len();
                    EditEvent::Redraw
                }
            }
        }
    }

    /// Compose the minimal ANSI repaint string for the current state:
    /// - CR to line start
    /// - prompt + buffer
    /// - clear-to-EOL
    /// - move cursor left if needed to position within buffer
    pub fn repaint_line(&self) -> String {
        let mut s = String::new();
        s.push('\r');
        s.push_str(&self.prompt);
        s.push_str(&self.buf);
        s.push_str("\x1b[K"); // clear to end of line

        // Move cursor back from end to desired position
        let target = self.prompt.len() + self.cursor;
        let current = self.prompt.len() + self.buf.len();
        if current > target {
            let back = current - target;
            s.push_str(&format!("\x1b[{}D", back));
        }
        s
    }

    /// Access current buffer (e.g., for preview or external validation).
    pub fn buffer(&self) -> &str {
        &self.buf
    }

    /// Replace current buffer (e.g., programmatic completion).
    pub fn set_buffer(&mut self, new_buf: impl Into<String>) {
        self.buf = new_buf.into();
        self.cursor = self.buf.len();
        self.hist_ix = None;
    }

    /// Move cursor to absolute position within the buffer (clamped).
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.buf.len());
    }

    /// Clear current line (buffer + cursor).
    pub fn clear_line(&mut self) {
        self.buf.clear();
        self.cursor = 0;
        self.hist_ix = None;
    }
}
