//! Output sink abstraction so slash commands can run in both REPL (println!
//! to stdout) and TUI (captured to a Vec<String> that becomes a ChatEntry).
//!
//! Design:
//! - `emit(line)` writes a single line of output.
//! - REPL mode uses the default sink (writes to stdout with a trailing newline).
//! - TUI mode installs a `VecSink` via `with_capture` while dispatching, so
//!   all output is collected into a string buffer, then shown as one ChatEntry.
//!
//! Commands in `slash/commands/*.rs` should use `emit!(...)` / `emitln!()`
//! instead of `println!` / `print!` so they work in both modes.

use std::cell::RefCell;

thread_local! {
    /// When Some, emit() appends to this buffer instead of printing.
    /// Installed by `with_capture`.
    static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };

    /// True when running inside TUI raw-mode. Wizard and other interactive
    /// prompts check this to bail instead of blocking on stdin.
    static TUI_MODE: RefCell<bool> = const { RefCell::new(false) };
}

/// Mark that we are (or aren't) in TUI raw-mode.
pub fn set_tui_mode(enabled: bool) {
    TUI_MODE.with(|cell| *cell.borrow_mut() = enabled);
}

/// Returns true while the TUI is active (raw-mode stdin).
pub fn is_tui_mode() -> bool {
    TUI_MODE.with(|cell| *cell.borrow())
}

/// Emit one line of command output. Adds a trailing newline.
pub fn emit(s: &str) {
    CAPTURE.with(|cell| {
        let mut guard = cell.borrow_mut();
        match guard.as_mut() {
            Some(buf) => {
                buf.push_str(s);
                buf.push('\n');
            }
            None => {
                println!("{s}");
            }
        }
    });
}

/// Emit a blank line.
pub fn blank() {
    emit("");
}

/// Run `f` with output captured into a String. Returns (captured_text, f_result).
/// Used by the TUI dispatcher to grab command output without touching stdout.
pub fn with_capture<R, F: FnOnce() -> R>(f: F) -> (String, R) {
    // Install empty buffer
    CAPTURE.with(|cell| *cell.borrow_mut() = Some(String::new()));
    let result = f();
    // Take buffer
    let captured = CAPTURE
        .with(|cell| cell.borrow_mut().take())
        .unwrap_or_default();
    (captured, result)
}

/// Macro wrapper so callers can use familiar `emitln!("foo {x}")` syntax.
/// Expands into `emit(&format!(...))`.
#[macro_export]
macro_rules! emitln {
    () => { $crate::slash::output::blank() };
    ($($arg:tt)*) => { $crate::slash::output::emit(&format!($($arg)*)) };
}
