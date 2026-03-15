/// FFI I/O operations for the Rice runtime.
///
/// Handles PRINT, INPUT, and console operations.

use std::io::Write;

use crate::runtime::value_ffi::ffi_to_value;
use crate::value::Value;

/// Runtime state for compiled programs
pub struct RiceRuntime {
    /// Current print column position (for zone tabbing)
    print_col: usize,
    /// Output writer (stdout for real programs)
    output: Box<dyn Write>,
}

impl RiceRuntime {
    fn new() -> Self {
        Self {
            print_col: 0,
            output: Box::new(std::io::stdout()),
        }
    }

    fn print_value(&mut self, val: &Value) {
        let s = val.format_for_print();
        let _ = write!(self.output, "{s}");
        self.print_col += s.len();
    }

    fn print_comma(&mut self) {
        let next_zone = ((self.print_col / 14) + 1) * 14;
        let spaces = next_zone - self.print_col;
        let pad = " ".repeat(spaces);
        let _ = write!(self.output, "{pad}");
        self.print_col = next_zone;
    }

    fn print_newline(&mut self) {
        let _ = writeln!(self.output);
        self.print_col = 0;
    }
}

// --- extern "C" functions ---

#[unsafe(no_mangle)]
pub extern "C" fn rice_runtime_init() -> *mut RiceRuntime {
    let rt = Box::new(RiceRuntime::new());
    Box::into_raw(rt)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_runtime_shutdown(rt: *mut RiceRuntime) {
    if !rt.is_null() {
        unsafe {
            let mut rt = Box::from_raw(rt);
            let _ = rt.output.flush();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_print(rt: *mut RiceRuntime, tag: i64, data: i64, sep: i32) {
    if rt.is_null() {
        return;
    }
    let rt = unsafe { &mut *rt };
    let val = ffi_to_value(tag, data);
    rt.print_value(&val);
    // sep=1 means semicolon (no action needed after value)
    // sep=0 means no separator (just print value)
    _ = sep;
}

/// Print zone tab (comma separator) — no value needed
#[unsafe(no_mangle)]
pub extern "C" fn rice_print_comma(rt: *mut RiceRuntime) {
    if rt.is_null() {
        return;
    }
    let rt = unsafe { &mut *rt };
    rt.print_comma();
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_print_newline(rt: *mut RiceRuntime) {
    if rt.is_null() {
        return;
    }
    let rt = unsafe { &mut *rt };
    rt.print_newline();
}
