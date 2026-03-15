/// Rice BASIC runtime library.
///
/// Provides extern "C" functions called by compiled BASIC programs.
/// Reuses the interpreter's Value type and builtin implementations.

pub mod value_ffi;
pub mod io_ffi;
