//! Per-terminal caret positioning.
//!
//! Linux terminals differ in how they report the autocomplete caret. There are
//! two signals we can use:
//!
//! - **figterm** (`crates/figterm`): the shell wrapper runs its own alacritty `Term` and reports
//!   cursor `(col, row)` on every edit-buffer event.
//! - **IBus** `SetCursorLocation`: the terminal app pushes the absolute screen position of its
//!   caret to the IBus daemon for IME placement.
//!
//! Neither signal is universally correct, so this module owns both the math
//! and the state. The callers (`ibus.rs`, `remote_ipc/mod.rs`) just hand over
//! the raw event and receive a position (or `None`) — they contain no
//! terminal-specific logic.
//!
//! ## Approach
//!
//! - Figterm drives `caret_x`: on each `edit_buffer` we compute a base `inner_x + col * cell_w`
//!   and, when an IBus anchor is available, apply the figterm column delta against it (`anchor_x +
//!   (col - anchor_col) * cell_w`). The anchor pairs IBus's last absolute x with the figterm column
//!   reported at that moment, so figterm's deltas fill in movement between IBus events — in
//!   particular backspace, where some terminals (e.g. GNOME Terminal) never push a
//!   `SetCursorLocation`.
//! - IBus drives `caret_y` (figterm's row is unreliable on some terminals).
//!
//! ## Per-terminal tweak
//!
//! `caret_size.height` controls how far below the caret the popup placement
//! math lands the popup. Ghostty's reported IBus y sits near the cell bottom
//! (so emitting a small height places the popup just below the cell); GNOME
//! Terminal — and any unknown terminal — reports y near the cell top, so we
//! emit `cell_h` to push the popup onto the next row. See [`emit_full_cell_height`].
//!
//! ## Adding a new terminal
//!
//! Add an arm to [`emit_full_cell_height`] if the terminal's IBus y convention
//! differs from the default. If it has a different x convention (e.g. reports
//! the next-write position instead of the current cursor), add a similar
//! per-terminal helper alongside.

use std::collections::HashMap;

use fig_proto::local::TerminalCursorCoordinates;
use fig_proto::local::caret_position_hook::Origin;
use fig_util::Terminal;
use parking_lot::Mutex;
use tao::dpi::{
    LogicalPosition,
    LogicalSize,
};

use crate::event::WindowPosition;

/// True if the terminal's IBus `SetCursorLocation` reports y near the cell
/// bottom (popup math then needs only a tiny `caret_size.height` to land the
/// popup below the cell). False for terminals that report near the cell top
/// (we emit the full `cell_h` so the popup lands on the next row).
fn emit_full_cell_height(terminal: Option<&Terminal>) -> bool {
    !matches!(terminal, Some(Terminal::Ghostty))
}

/// All per-pid state required by the caret logic. Owned by `PlatformStateImpl`
/// as a single field; the maps are kept private.
#[derive(Debug, Default)]
pub struct CaretState {
    /// Last y reported by IBus per pid. Drives `caret_y` on figterm events.
    last_ibus_y: Mutex<HashMap<i32, i32>>,
    /// Most recent figterm `coords.x` per pid, snapshotted on every
    /// edit_buffer event so the IBus path can pair its absolute x with the
    /// figterm column reported at that moment.
    last_figterm_col: Mutex<HashMap<i32, i32>>,
    /// X-anchor per pid: `(ibus_x, figterm_col)` captured the last time IBus
    /// fired. On subsequent figterm events the caret x is computed as
    /// `anchor_x + (current_col - anchor_col) * cell_w`, so the popup tracks
    /// the visible cursor via IBus's absolute x while figterm's column deltas
    /// fill in movement between IBus events (e.g. on backspace in GNOME
    /// Terminal, where IBus stays silent).
    anchor: Mutex<HashMap<i32, (f64, i32)>>,
}

impl CaretState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Handle an IBus `SetCursorLocation`. Updates state; never emits a position
/// itself (the figterm path is the sole emitter — that avoids racing the two
/// paths on per-keystroke updates).
pub fn on_ibus_set_cursor_location(
    state: &CaretState,
    active_pid: Option<i32>,
    body: (i32, i32, i32, i32),
) -> Option<WindowPosition> {
    if let Some(pid) = active_pid {
        state.last_ibus_y.lock().insert(pid, body.1);
        if let Some(&col) = state.last_figterm_col.lock().get(&pid) {
            state.anchor.lock().insert(pid, (body.0 as f64, col));
        }
    }
    None
}

/// Handle an IBus `SetCursorLocationRelative` (window-relative coords). Used
/// by terminals that push relative caret positions instead of absolute ones;
/// we resolve to screen coords using the focused window's `outer_x/y` and
/// `scale`.
pub fn on_ibus_set_cursor_location_relative(
    body: (i32, i32, i32, i32),
    outer_x: i32,
    outer_y: i32,
    scale: f32,
) -> Option<WindowPosition> {
    let abs_x = (body.0 as f32 / scale).round() as i32 + outer_x;
    let abs_y = (body.1 as f32 / scale).round() as i32 + outer_y - (body.3 as f32 / scale).round() as i32;
    Some(WindowPosition::RelativeToCaret {
        caret_position: LogicalPosition::new(abs_x as f64, abs_y as f64).into(),
        caret_size: LogicalSize::new(body.2 as f64, body.3 as f64).into(),
        origin: Origin::TopLeft,
    })
}

/// Handle a figterm `edit_buffer` event. Returns the caret-relative position
/// to emit, or `None` if no position can be computed (e.g. degenerate cell
/// size).
///
/// `inner` is the focused window's content-area rect `(x, y, w, h)` in screen
/// pixels, as reported by the GNOME Shell extension.
pub fn on_figterm_edit_buffer(
    state: &CaretState,
    terminal: Option<&Terminal>,
    coords: &TerminalCursorCoordinates,
    inner: (i32, i32, i32, i32),
    pid: Option<i32>,
) -> Option<WindowPosition> {
    let (inner_x, inner_y, inner_w, inner_h) = inner;

    let cell_w = if coords.cols > 0 {
        inner_w as f64 / coords.cols as f64
    } else {
        coords.xpixel as f64
    };
    let cell_h = if coords.rows > 0 {
        inner_h as f64 / coords.rows as f64
    } else {
        coords.ypixel as f64
    };
    if cell_w <= 0.0 || cell_h <= 0.0 {
        return None;
    }

    // Snapshot figterm's column so the next IBus event can form an anchor.
    if let Some(pid) = pid {
        state.last_figterm_col.lock().insert(pid, coords.x);
    }

    // Caret x: use the IBus anchor + figterm column delta when we have one,
    // otherwise the figterm-naive `inner_x + col * cell_w`.
    let caret_x = match pid.and_then(|pid| state.anchor.lock().get(&pid).copied()) {
        Some((anchor_x, anchor_col)) => anchor_x + (coords.x - anchor_col) as f64 * cell_w,
        None => inner_x as f64 + (coords.x as f64) * cell_w,
    };

    // Caret y: IBus override when available, else figterm-naive. `emitted_cell_h` is the popup
    // placement offset — see [`emit_full_cell_height`]. Zed reports y near the cell bottom (like
    // Ghostty) but with extra descender padding, so a small fraction of cell_h lands the popup
    // just below the glyph without a visible gap.
    let (caret_y, emitted_cell_h) = match pid.and_then(|pid| state.last_ibus_y.lock().get(&pid).copied()) {
        Some(y) => {
            let height = match terminal {
                Some(Terminal::Zed) => cell_h * 0.18,
                _ if emit_full_cell_height(terminal) => cell_h,
                _ => 1.0,
            };
            (y as f64, height)
        },
        None => (inner_y as f64 + (coords.y as f64) * cell_h, cell_h),
    };

    Some(WindowPosition::RelativeToCaret {
        caret_position: LogicalPosition::new(caret_x, caret_y).into(),
        caret_size: LogicalSize::new(cell_w, emitted_cell_h).into(),
        origin: Origin::TopLeft,
    })
}
