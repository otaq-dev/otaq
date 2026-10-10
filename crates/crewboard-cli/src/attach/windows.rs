//! Windows: the console in raw virtual-terminal mode (keys arrive as the same bytes a Unix terminal sends), read
//! as key events so that Ctrl+] (detach) arrives as a key even where the console doesn't spell it as a character;
//! the window size from the console, and resizes noticed by looking at the size four times a second (a console
//! has no resize signal).

use std::io;
use std::time::Duration;

use crewboard_core::session::TerminalSize;
use tokio::sync::mpsc;
use windows_sys::Win32::System::Console::{
    CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT,
    ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_PROCESSED_OUTPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode,
    GetConsoleScreenBufferInfo, GetStdHandle, INPUT_RECORD, KEY_EVENT, LEFT_CTRL_PRESSED,
    RIGHT_CTRL_PRESSED, ReadConsoleInputW, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetConsoleMode,
};

/// The console in raw mode for as long as this lives; restored on drop (also on errors).
pub struct RawTerminal {
    input: CONSOLE_MODE,
    output: Option<CONSOLE_MODE>,
}

fn mode(handle: u32) -> Option<CONSOLE_MODE> {
    let mut mode = 0;
    // SAFETY: a standard handle and an out parameter; fails when it isn't a console.
    let ok = unsafe { GetConsoleMode(GetStdHandle(handle), &mut mode) };
    (ok != 0).then_some(mode)
}

fn set_mode(handle: u32, mode: CONSOLE_MODE) -> bool {
    // SAFETY: a standard handle and a mode value.
    unsafe { SetConsoleMode(GetStdHandle(handle), mode) != 0 }
}

impl RawTerminal {
    /// `None` when stdin isn't a console (keys are then relayed as they come).
    pub fn enter() -> Option<Self> {
        let input = mode(STD_INPUT_HANDLE)?;
        let raw = (input & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
            | ENABLE_VIRTUAL_TERMINAL_INPUT;
        if !set_mode(STD_INPUT_HANDLE, raw) {
            return None;
        }
        let output = mode(STD_OUTPUT_HANDLE);
        if let Some(output) = output {
            set_mode(
                STD_OUTPUT_HANDLE,
                output
                    | ENABLE_PROCESSED_OUTPUT
                    | ENABLE_VIRTUAL_TERMINAL_PROCESSING
                    | DISABLE_NEWLINE_AUTO_RETURN,
            );
        }
        Some(Self { input, output })
    }

    pub fn size(&self) -> Option<TerminalSize> {
        size()
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        set_mode(STD_INPUT_HANDLE, self.input);
        if let Some(output) = self.output {
            set_mode(STD_OUTPUT_HANDLE, output);
        }
    }
}

fn size() -> Option<TerminalSize> {
    let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: a standard handle and an out parameter.
    if unsafe { GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) } == 0 {
        return None;
    }
    let cols = u16::try_from(info.srWindow.Right - info.srWindow.Left + 1).ok()?;
    let rows = u16::try_from(info.srWindow.Bottom - info.srWindow.Top + 1).ok()?;
    (cols > 0 && rows > 0).then_some(TerminalSize { cols, rows })
}

/// Resizes of the console window.
pub struct WindowChanges {
    tick: tokio::time::Interval,
    last: Option<TerminalSize>,
}

impl WindowChanges {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            tick: tokio::time::interval(Duration::from_millis(250)),
            last: size(),
        })
    }

    /// Resolves once the size differs from the last one seen.
    pub async fn next(&mut self) {
        loop {
            self.tick.tick().await;
            let now = size();
            if now != self.last {
                self.last = now;
                return;
            }
        }
    }
}

/// The `]` key's virtual-key code (VK_OEM_6 on US layouts).
const VK_CLOSE_BRACKET: u16 = 0xDD;

/// Reads the console's key presses and passes them on as UTF-8, until the console or the receiver goes away.
/// Returns `false` (reading nothing) when standard input isn't a console.
pub fn read_console_keys(keys: &mpsc::Sender<Vec<u8>>) -> bool {
    if mode(STD_INPUT_HANDLE).is_none() {
        return false;
    }
    // SAFETY: a standard handle; checked above to be a console.
    let input = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let mut records: [INPUT_RECORD; 64] = unsafe { std::mem::zeroed() };
    loop {
        let mut read = 0u32;
        // SAFETY: the buffer holds 64 records.
        if unsafe { ReadConsoleInputW(input, records.as_mut_ptr(), 64, &mut read) } == 0 {
            return true;
        }
        let mut units: Vec<u16> = Vec::new();
        for record in &records[..read as usize] {
            if u32::from(record.EventType) != KEY_EVENT {
                continue;
            }
            // SAFETY: a KEY_EVENT record holds a KEY_EVENT_RECORD.
            let key = unsafe { record.Event.KeyEvent };
            if key.bKeyDown == 0 {
                continue;
            }
            // SAFETY: the character union is always readable as UTF-16.
            let mut unit = unsafe { key.uChar.UnicodeChar };
            let ctrl = key.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0;
            if unit == 0 && ctrl && key.wVirtualKeyCode == VK_CLOSE_BRACKET {
                unit = u16::from(crate::attach::DETACH_KEY);
            }
            if unit != 0 {
                units.extend(std::iter::repeat_n(
                    unit,
                    usize::from(key.wRepeatCount.max(1)),
                ));
            }
        }
        if units.is_empty() {
            continue;
        }
        if keys
            .blocking_send(String::from_utf16_lossy(&units).into_bytes())
            .is_err()
        {
            return true;
        }
    }
}
